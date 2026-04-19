use fin_contracts::{
    ControlFeedback, EventEnvelope, ExecutionNote, ProgressBlock, ProjectionView,
    ProviderEventPayload,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    fs,
    io::Write,
    net::TcpListener,
    path::{Path, PathBuf},
    thread,
};
use thiserror::Error;

mod chat_api;
mod event_stream;
mod http;
mod routes;
mod session_view;
mod web_app;
mod web_assets;
mod web_styles;

pub use chat_api::{ChatSendRequest, ChatSendResponse, DebugBinding};

pub(crate) use http::{
    HttpRequest, HttpResponse, bad_request_response, css_response, file_response, html_response,
    internal_error_response, javascript_response, json_response, not_found_response,
    read_http_request, write_http_response,
};

pub(crate) const INDEX_HTML_PATH: &str = "/";
pub(crate) const STYLES_CSS_PATH: &str = "/styles.css";
pub(crate) const API_BINDING_PATH: &str = "/api/binding.json";
pub(crate) const API_PROJECTION_PATH: &str = "/api/current_projection.json";
pub(crate) const API_SNAPSHOT_PATH: &str = "/api/current_snapshot.json";
pub(crate) const API_EVENTS_PATH: &str = "/api/latest_events.jsonl";
pub(crate) const API_LAST_RUN_PATH: &str = "/api/last_run.json";
pub(crate) const API_CURRENT_CONTEXT_PATH: &str = "/api/current_context.json";
pub(crate) const API_RECENT_CONTEXTS_PATH: &str = "/api/recent_contexts.json";
pub(crate) const API_RECENT_DIGESTS_PATH: &str = "/api/recent_digests.json";
pub(crate) const API_RECENT_REASONING_VIEWS_PATH: &str = "/api/recent_reasoning_views.json";
pub(crate) const API_RECENT_TOOL_RECORDS_PATH: &str = "/api/recent_tool_records.json";
pub(crate) const API_RECENT_CLOSURES_PATH: &str = "/api/recent_closures.json";
pub(crate) const API_RECENT_TURNS_PATH: &str = "/api/recent_turns.json";
pub(crate) const API_SESSION_MESSAGES_PATH: &str = "/api/session_messages.json";
pub(crate) const API_SESSION_EVENTS_PATH: &str = "/api/session_events.json";
pub(crate) const API_SESSION_EVENT_ARCHIVE_INDEX_PATH: &str = "/api/session_event_archive_index.json";
pub(crate) const API_SESSION_EVENTS_SEGMENT_PATH: &str = "/api/session_events_segment.json";
pub(crate) const API_CURRENT_EXECUTION_STATE_PATH: &str = "/api/current_execution_state.json";
pub(crate) const API_CURRENT_PENDING_INPUTS_PATH: &str = "/api/current_pending_inputs.json";
pub(crate) const API_CURRENT_PAUSE_CHECKPOINT_PATH: &str = "/api/current_pause_checkpoint.json";
pub(crate) const API_CURRENT_INTERRUPTED_SEGMENT_PATH: &str = "/api/current_interrupted_segment.json";
pub(crate) const API_CURRENT_SEGMENT_MERGE_PATH: &str = "/api/current_segment_merge.json";
pub(crate) const API_CURRENT_ROUTING_DECISION_PATH: &str = "/api/current_routing_decision.json";
pub(crate) const API_QQBOT_STATE_PATH: &str = "/api/qqbot_state.json";
pub(crate) const API_QQBOT_EVENTS_PATH: &str = "/api/qqbot_events.jsonl";
pub(crate) const API_CHAT_SEND_PATH: &str = "/api/chat/send";
pub(crate) const API_WATCH_PATH: &str = "/api/watch";

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

pub trait DebugActionHandler {
    fn read_binding(&self, runtime_home: &Path) -> Result<DebugBinding, String>;
    fn send_chat_message(
        &self,
        runtime_home: &Path,
        request: ChatSendRequest,
    ) -> Result<ChatSendResponse, String>;
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct NoopDebugActionHandler;

impl DebugActionHandler for NoopDebugActionHandler {
    fn read_binding(&self, runtime_home: &Path) -> Result<DebugBinding, String> {
        let project_label = runtime_home
            .file_name()
            .and_then(|value| value.to_str())
            .filter(|value| !value.trim().is_empty())
            .unwrap_or("fin")
            .to_string();
        Ok(DebugBinding {
            project_id: project_label.clone(),
            project_label,
            runtime_home: runtime_home.display().to_string(),
            session_id: None,
            task_id: None,
            session_messages_path: None,
            recent_contexts_path: None,
            recent_digests_path: None,
        })
    }

    fn send_chat_message(
        &self,
        _runtime_home: &Path,
        _request: ChatSendRequest,
    ) -> Result<ChatSendResponse, String> {
        Err("chat send handler not configured".into())
    }
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

impl InMemoryProjector {
    pub fn apply(&mut self, event: &EventEnvelope<Value>) {
        self.current.session_id = event.refs.session_id.clone();
        self.current.task_id = event.refs.task_id.clone();
        self.current.topic_thread_id = event.refs.topic_thread_id.clone();

        match event.event_type.as_str() {
            "provider.operation_accepted"
            | "provider.gateway_request_sent"
            | "provider.gateway_response_received"
            | "provider.response_normalized"
            | "provider.completed" => {
                self.current.latest_provider_activity = Some(event.event_type.clone());
                if let Ok(payload) =
                    serde_json::from_value::<ProviderEventPayload>(event.payload.clone())
                {
                    if let Some(debug) = payload.debug {
                        self.current.latest_provider_user_agent = debug.user_agent;
                        self.current.latest_provider_header_names =
                            debug.request_headers.keys().cloned().collect();
                    }
                }
            }
            "progress.updated" => {
                if let Ok(progress) = serde_json::from_value::<ProgressBlock>(event.payload.clone())
                {
                    self.current.current_phase = Some(progress.phase);
                    self.current.latest_progress_id = Some(progress.progress_id);
                }
            }
            "control.feedback_recorded" => {
                if let Ok(feedback) =
                    serde_json::from_value::<ControlFeedback>(event.payload.clone())
                {
                    self.current.latest_control_origin = Some(feedback.origin);
                    self.current.latest_continuity_confidence =
                        Some(feedback.continuity_confidence);
                    self.current.latest_topic_shift_confidence =
                        Some(feedback.topic_shift_confidence);
                    self.current.latest_simple_query_confidence =
                        Some(feedback.simple_query_confidence);
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
                self.current
                    .warnings
                    .retain(|warning| warning != "runtime_busy");
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
    let handler = NoopDebugActionHandler;
    serve_debug_mvp_with_handler(runtime_home, bind_addr, &handler)
}

pub fn serve_debug_mvp_with_handler(
    runtime_home: &Path,
    bind_addr: &str,
    handler: &(impl DebugActionHandler + Sync),
) -> Result<(), DebugDataError> {
    let listener = TcpListener::bind(bind_addr).map_err(|source| DebugDataError::Io {
        path: bind_addr.to_string(),
        source,
    })?;

    thread::scope(|scope| {
        for stream in listener.incoming() {
            let mut stream = stream.map_err(|source| DebugDataError::Io {
                path: bind_addr.to_string(),
                source,
            })?;
            let runtime_home = runtime_home.to_path_buf();
            scope.spawn(move || {
                let _ = routes::handle_connection(&mut stream, &runtime_home, handler);
            });
        }

        Ok(())
    })
}
#[cfg(test)]
pub(crate) use routes::response_for_request;
#[cfg(test)]
pub(crate) use routes::response_for_path;

#[cfg(test)]
mod tests;
