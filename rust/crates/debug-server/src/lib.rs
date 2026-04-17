use fin_contracts::{EventEnvelope, ExecutionNote, ProgressBlock, ProjectionView};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    fs,
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    path::{Path, PathBuf},
};
use thiserror::Error;

mod web_assets;

const INDEX_HTML_PATH: &str = "/";
const APP_JS_PATH: &str = "/app.js";
const STYLES_CSS_PATH: &str = "/styles.css";
const API_PROJECTION_PATH: &str = "/api/current_projection.json";
const API_SNAPSHOT_PATH: &str = "/api/current_snapshot.json";
const API_EVENTS_PATH: &str = "/api/latest_events.jsonl";
const API_LAST_RUN_PATH: &str = "/api/last_run.json";

#[derive(Debug, Error)]
pub enum DebugDataError {
    #[error("io error at '{path}': {source}")]
    Io {
        path: String,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to serialize debug artifact: {0}")]
    Serialize(#[from] serde_json::Error),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DebugProjectionConfig {
    pub retain_raw_events: bool,
    pub retain_timeline_rows: usize,
}

impl Default for DebugProjectionConfig {
    fn default() -> Self {
        Self {
            retain_raw_events: true,
            retain_timeline_rows: 10_000,
        }
    }
}

#[derive(Debug, Default, Clone, PartialEq)]
pub struct InMemoryProjector {
    pub current: ProjectionView,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DebugSnapshot {
    pub projection: ProjectionView,
    pub events: Vec<EventEnvelope<Value>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DebugSnapshotPaths {
    pub projection_json: PathBuf,
    pub events_jsonl: PathBuf,
    pub snapshot_json: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct HttpResponse {
    status_code: u16,
    content_type: &'static str,
    body: Vec<u8>,
}

impl InMemoryProjector {
    pub fn apply(&mut self, event: &EventEnvelope<Value>) {
        self.current.session_id = event.refs.session_id.clone();
        self.current.task_id = event.refs.task_id.clone();
        self.current.topic_thread_id = event.refs.topic_thread_id.clone();

        match event.event_type.as_str() {
            "provider.request_started" | "provider.response_received" => {
                self.current.latest_provider_activity = Some(event.event_type.clone());
            }
            "progress.updated" => {
                if let Ok(progress) = serde_json::from_value::<ProgressBlock>(event.payload.clone())
                {
                    self.current.current_phase = Some(progress.phase);
                    self.current.latest_progress_id = Some(progress.progress_id);
                }
            }
            "execution_note.appended" => {
                if let Ok(note) = serde_json::from_value::<ExecutionNote>(event.payload.clone()) {
                    self.current.latest_note_id = Some(note.note_id);
                }
            }
            "digest.finalized" => {
                if let Some(digest_id) = event.payload.get("digest_id").and_then(Value::as_str) {
                    self.current.latest_digest_id = Some(digest_id.to_string());
                }
            }
            "operation.completed" => {
                self.current.warnings.retain(|w| w != "runtime_busy");
            }
            _ => {}
        }
    }
}

pub fn build_projection(events: &[EventEnvelope<Value>]) -> ProjectionView {
    let mut projector = InMemoryProjector::default();
    for event in events {
        projector.apply(event);
    }
    projector.current
}

pub fn persist_snapshot(
    projection_dir: &Path,
    events: &[EventEnvelope<Value>],
) -> Result<DebugSnapshotPaths, DebugDataError> {
    fs::create_dir_all(projection_dir).map_err(|source| DebugDataError::Io {
        path: projection_dir.display().to_string(),
        source,
    })?;

    let projection = build_projection(events);
    let snapshot = DebugSnapshot {
        projection,
        events: events.to_vec(),
    };

    let projection_json = projection_dir.join("current_projection.json");
    let events_jsonl = projection_dir.join("latest_events.jsonl");
    let snapshot_json = projection_dir.join("current_snapshot.json");

    fs::write(
        &projection_json,
        serde_json::to_vec_pretty(&snapshot.projection)?,
    )
    .map_err(|source| DebugDataError::Io {
        path: projection_json.display().to_string(),
        source,
    })?;

    let mut events_file = fs::File::create(&events_jsonl).map_err(|source| DebugDataError::Io {
        path: events_jsonl.display().to_string(),
        source,
    })?;
    for event in events {
        let line = serde_json::to_string(event)?;
        writeln!(events_file, "{line}").map_err(|source| DebugDataError::Io {
            path: events_jsonl.display().to_string(),
            source,
        })?;
    }

    fs::write(&snapshot_json, serde_json::to_vec_pretty(&snapshot)?).map_err(|source| {
        DebugDataError::Io {
            path: snapshot_json.display().to_string(),
            source,
        }
    })?;

    Ok(DebugSnapshotPaths {
        projection_json,
        events_jsonl,
        snapshot_json,
    })
}

pub fn serve_debug_mvp(runtime_home: &Path, bind_addr: &str) -> Result<(), DebugDataError> {
    let listener = TcpListener::bind(bind_addr).map_err(|source| DebugDataError::Io {
        path: bind_addr.to_string(),
        source,
    })?;

    for stream in listener.incoming() {
        let mut stream = stream.map_err(|source| DebugDataError::Io {
            path: bind_addr.to_string(),
            source,
        })?;
        handle_connection(&mut stream, runtime_home)?;
    }

    Ok(())
}

fn handle_connection(stream: &mut TcpStream, runtime_home: &Path) -> Result<(), DebugDataError> {
    let mut buffer = [0_u8; 8192];
    let bytes_read = stream
        .read(&mut buffer)
        .map_err(|source| DebugDataError::Io {
            path: "tcp-stream-read".into(),
            source,
        })?;
    let request = String::from_utf8_lossy(&buffer[..bytes_read]);
    let path = parse_request_path(&request);
    let response = response_for_path(path, runtime_home);
    write_http_response(stream, &response)
}

fn parse_request_path(request: &str) -> &str {
    request
        .lines()
        .next()
        .and_then(|line| {
            let mut parts = line.split_whitespace();
            let method = parts.next()?;
            let path = parts.next()?;
            if method == "GET" { Some(path) } else { None }
        })
        .unwrap_or("/")
}

fn response_for_path(path: &str, runtime_home: &Path) -> HttpResponse {
    match path {
        INDEX_HTML_PATH => html_response(web_assets::INDEX_HTML),
        APP_JS_PATH => javascript_response(web_assets::APP_JS),
        STYLES_CSS_PATH => css_response(web_assets::STYLES_CSS),
        API_PROJECTION_PATH => file_response(
            &runtime_home.join("runtime/projections/current_projection.json"),
            "application/json; charset=utf-8",
        ),
        API_SNAPSHOT_PATH => file_response(
            &runtime_home.join("runtime/projections/current_snapshot.json"),
            "application/json; charset=utf-8",
        ),
        API_EVENTS_PATH => file_response(
            &runtime_home.join("runtime/projections/latest_events.jsonl"),
            "application/x-ndjson; charset=utf-8",
        ),
        API_LAST_RUN_PATH => file_response(
            &runtime_home.join("runtime/current/last_run.json"),
            "application/json; charset=utf-8",
        ),
        _ => not_found_response(path),
    }
}

fn html_response(body: &str) -> HttpResponse {
    HttpResponse {
        status_code: 200,
        content_type: "text/html; charset=utf-8",
        body: body.as_bytes().to_vec(),
    }
}

fn javascript_response(body: &str) -> HttpResponse {
    HttpResponse {
        status_code: 200,
        content_type: "application/javascript; charset=utf-8",
        body: body.as_bytes().to_vec(),
    }
}

fn css_response(body: &str) -> HttpResponse {
    HttpResponse {
        status_code: 200,
        content_type: "text/css; charset=utf-8",
        body: body.as_bytes().to_vec(),
    }
}

fn file_response(path: &Path, content_type: &'static str) -> HttpResponse {
    match fs::read(path) {
        Ok(body) => HttpResponse {
            status_code: 200,
            content_type,
            body,
        },
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => not_found_response(
            path.file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("artifact"),
        ),
        Err(err) => internal_error_response(&format!("failed to read {}: {err}", path.display())),
    }
}

fn not_found_response(path: &str) -> HttpResponse {
    HttpResponse {
        status_code: 404,
        content_type: "text/plain; charset=utf-8",
        body: format!("not found: {path}\n").into_bytes(),
    }
}

fn internal_error_response(message: &str) -> HttpResponse {
    HttpResponse {
        status_code: 500,
        content_type: "text/plain; charset=utf-8",
        body: format!("internal error: {message}\n").into_bytes(),
    }
}

fn write_http_response(
    stream: &mut TcpStream,
    response: &HttpResponse,
) -> Result<(), DebugDataError> {
    let status_text = match response.status_code {
        200 => "OK",
        404 => "Not Found",
        500 => "Internal Server Error",
        _ => "OK",
    };
    let header = format!(
        "HTTP/1.1 {} {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n",
        response.status_code,
        status_text,
        response.content_type,
        response.body.len()
    );
    stream
        .write_all(header.as_bytes())
        .and_then(|_| stream.write_all(&response.body))
        .and_then(|_| stream.flush())
        .map_err(|source| DebugDataError::Io {
            path: "tcp-stream-write".into(),
            source,
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use fin_contracts::{
        DebugVisibility, EntityRefs, EventEnvelope, ExecutionNote, ProgressBlock, Severity,
    };
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn projector_tracks_latest_progress_note_digest_and_provider_activity() {
        let refs = EntityRefs {
            session_id: Some("session-1".into()),
            task_id: Some("task-1".into()),
            topic_thread_id: Some("topic-1".into()),
            dispatch_id: None,
            worker_id: None,
        };
        let progress = ProgressBlock {
            progress_id: "progress-1".into(),
            refs: refs.clone(),
            phase: "running".into(),
            blocker: None,
            next_step: None,
            health_hint: None,
            tool_snapshots: vec![],
        };
        let note = ExecutionNote {
            note_id: "note-1".into(),
            refs: refs.clone(),
            summary: "progress made".into(),
            decision: None,
            lesson: None,
            blocker: None,
            next_step: None,
            created_at: "2026-04-17T00:00:00Z".into(),
        };
        let mut projector = InMemoryProjector::default();

        for (idx, (kind, payload)) in [
            (
                "provider.request_started",
                serde_json::json!({"provider":"openai"}),
            ),
            ("progress.updated", serde_json::to_value(progress).unwrap()),
            (
                "execution_note.appended",
                serde_json::to_value(note).unwrap(),
            ),
            (
                "digest.finalized",
                serde_json::json!({"digest_id": "digest-1"}),
            ),
        ]
        .into_iter()
        .enumerate()
        {
            let mut event = EventEnvelope::new(
                format!("evt-{}", idx + 1),
                kind,
                "2026-04-17T00:00:00Z",
                "runtime",
                "trace-1",
                (idx + 1) as u64,
                payload,
            );
            event.refs = refs.clone();
            event.severity = Severity::Info;
            event.debug_visibility = DebugVisibility::Normal;
            projector.apply(&event);
        }

        assert_eq!(projector.current.current_phase.as_deref(), Some("running"));
        assert_eq!(
            projector.current.latest_progress_id.as_deref(),
            Some("progress-1")
        );
        assert_eq!(projector.current.latest_note_id.as_deref(), Some("note-1"));
        assert_eq!(
            projector.current.latest_digest_id.as_deref(),
            Some("digest-1")
        );
        assert_eq!(
            projector.current.latest_provider_activity.as_deref(),
            Some("provider.request_started")
        );
        assert_eq!(projector.current.task_id.as_deref(), Some("task-1"));
    }

    #[test]
    fn persist_snapshot_writes_projection_and_event_files() {
        let tmp = std::env::temp_dir().join(format!(
            "fin-debug-server-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("time should work")
                .as_nanos()
        ));
        let event = EventEnvelope::new(
            "evt-1",
            "provider.response_received",
            "2026-04-17T00:00:00Z",
            "runtime",
            "trace-1",
            1,
            serde_json::json!({"status": "ok"}),
        );
        let paths = persist_snapshot(&tmp, &[event]).expect("snapshot should persist");
        assert!(paths.projection_json.exists());
        assert!(paths.events_jsonl.exists());
        assert!(paths.snapshot_json.exists());
    }

    #[test]
    fn response_for_root_serves_html_shell() {
        let response = response_for_path("/", Path::new("/tmp/unused"));
        let body = String::from_utf8(response.body).expect("html should be utf8");
        assert_eq!(response.status_code, 200);
        assert!(body.contains("fin Debug MVP"));
        assert!(body.contains("Current Projection"));
    }

    #[test]
    fn response_for_api_projection_reads_runtime_artifact() {
        let runtime_home = std::env::temp_dir().join(format!(
            "fin-debug-runtime-home-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("time should work")
                .as_nanos()
        ));
        let projection_dir = runtime_home.join("runtime/projections");
        fs::create_dir_all(&projection_dir).expect("projection dir should exist");
        fs::write(
            projection_dir.join("current_projection.json"),
            br#"{"task_id":"task-1"}"#,
        )
        .expect("projection should write");

        let response = response_for_path(API_PROJECTION_PATH, &runtime_home);
        assert_eq!(response.status_code, 200);
        assert_eq!(response.content_type, "application/json; charset=utf-8");
        assert_eq!(
            String::from_utf8(response.body).unwrap(),
            r#"{"task_id":"task-1"}"#
        );
    }

    #[test]
    fn response_for_missing_artifact_returns_404() {
        let runtime_home = std::env::temp_dir().join(format!(
            "fin-debug-runtime-home-missing-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("time should work")
                .as_nanos()
        ));
        let response = response_for_path(API_SNAPSHOT_PATH, &runtime_home);
        assert_eq!(response.status_code, 404);
        assert!(
            String::from_utf8(response.body)
                .unwrap()
                .contains("current_snapshot.json")
        );
    }
}
