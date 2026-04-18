use crate::{ClosureRun, RuntimeError};
use fin_contracts::{ClosureTraceRecord, ContextSnapshotRecord, DigestRecord, ReasoningViewRecord, ToolExecutionRecord};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
};

const RECENT_CONTEXT_LIMIT: usize = 8;
const RECENT_DIGEST_LIMIT: usize = 8;
const RECENT_REASONING_LIMIT: usize = 16;
const RECENT_TOOL_RECORD_LIMIT: usize = 32;
const RECENT_CLOSURE_LIMIT: usize = 16;
const SESSION_MESSAGE_LIMIT: usize = 128;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionMessageRecord {
    pub message_id: String,
    pub role: String,
    pub content: String,
    pub created_at: String,
    pub session_id: String,
    pub task_id: Option<String>,
    #[serde(default)]
    pub operation_id: Option<String>,
    #[serde(default)]
    pub trace_id: Option<String>,
    #[serde(default)]
    pub closure_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionMaterializationReceipt {
    pub runtime_home: PathBuf,
    pub session_dir: PathBuf,
    pub session_id: String,
    pub session_recent_contexts_path: String,
    pub session_recent_digests_path: String,
    pub session_messages_path: String,
}

#[derive(Debug, Clone, Default)]
pub struct SessionMaterializer;

impl SessionMaterializer {
    pub fn persist(
        &self,
        runtime_home: &Path,
        run: &ClosureRun,
    ) -> Result<SessionMaterializationReceipt, RuntimeError> {
        let session_id = run
            .progress
            .refs
            .session_id
            .clone()
            .unwrap_or_else(|| "session-m1".into());
        let created_at = &run.note.created_at;
        let year = created_at.get(0..4).unwrap_or("unknown");
        let month = created_at.get(5..7).unwrap_or("00");
        let session_dir = runtime_home
            .join("sessions")
            .join(year)
            .join(month)
            .join(&session_id);

        for relative in [
            "events",
            "progress",
            "control",
            "notes",
            "digests",
            "context",
            "conversation",
            "reasoning",
            "tools",
            "closures",
            "collab",
            "tasks",
            "topics",
            "artifacts/candidates",
        ] {
            create_dir_all(&session_dir.join(relative))?;
        }

        write_json_lines(&session_dir.join("events/stream.jsonl"), &run.events)?;
        write_json_file(&session_dir.join("progress/latest.json"), &run.progress)?;
        write_json_file(&session_dir.join("control/latest.json"), &run.control_feedback)?;
        write_json_file(&session_dir.join("notes/latest.json"), &run.note)?;
        write_json_file(&session_dir.join("digests/latest.json"), &run.digest)?;
        persist_recent_digests(&session_dir, &run.digest)?;
        persist_context_snapshots(runtime_home, &session_dir, &run.context_snapshot)?;
        persist_reasoning_views(runtime_home, &session_dir, &run.reasoning_view)?;
        persist_tool_records(runtime_home, &session_dir, &run.tool_records)?;
        persist_closure_traces(runtime_home, &session_dir, &run.closure_trace)?;
        write_json_file(
            &runtime_home.join("runtime/current/current_control_feedback.json"),
            &run.control_feedback,
        )?;
        persist_session_messages(&session_dir, run)?;

        let session_recent_contexts_path =
            format!("sessions/{year}/{month}/{session_id}/context/recent_contexts.json");
        let session_recent_digests_path =
            format!("sessions/{year}/{month}/{session_id}/digests/recent_digests.json");
        let session_messages_path =
            format!("sessions/{year}/{month}/{session_id}/conversation/messages.json");
        let session_recent_reasoning_path =
            format!("sessions/{year}/{month}/{session_id}/reasoning/recent_reasoning_views.json");
        let session_recent_tool_records_path =
            format!("sessions/{year}/{month}/{session_id}/tools/recent_tool_records.json");
        let session_recent_closures_path =
            format!("sessions/{year}/{month}/{session_id}/closures/recent_closures.json");
        write_bytes(
            &runtime_home.join("runtime/current/last_run.json"),
            serde_json::to_vec_pretty(&json!({
                "operation_id": run.operation.operation_id,
                "trace_id": run.operation.trace_id,
                "turn_index": parse_turn_index(&run.operation.operation_id),
                "session_id": session_id,
                "task_id": run.progress.refs.task_id,
                "digest_id": run.digest.digest_id,
                "provider": run.prepared_request.provider_name,
                "model": run.prepared_request.model,
                "provider_user_agent": run.prepared_request.user_agent,
                "provider_header_names": run.prepared_request.sanitized_headers.keys().collect::<Vec<_>>(),
                "answer": run.assistant_response_text,
                "provider_raw_output": run.provider_response.output_text,
                "current_context_path": "runtime/current/current_context.json",
                "current_control_feedback_path": "runtime/current/current_control_feedback.json",
                "current_reasoning_view_path": "runtime/current/current_reasoning_view.json",
                "current_closure_trace_path": "runtime/current/current_closure_trace.json",
                "session_control_feedback_path": format!("sessions/{year}/{month}/{session_id}/control/latest.json"),
                "session_recent_contexts_path": session_recent_contexts_path,
                "session_recent_digests_path": session_recent_digests_path,
                "session_recent_reasoning_path": session_recent_reasoning_path,
                "session_recent_tool_records_path": session_recent_tool_records_path,
                "session_recent_closures_path": session_recent_closures_path,
                "session_messages_path": session_messages_path,
            }))?
            .as_slice(),
        )?;

        Ok(SessionMaterializationReceipt {
            runtime_home: runtime_home.to_path_buf(),
            session_dir,
            session_id: session_id.clone(),
            session_recent_contexts_path: format!(
                "sessions/{year}/{month}/{session_id}/context/recent_contexts.json"
            ),
            session_recent_digests_path: format!(
                "sessions/{year}/{month}/{session_id}/digests/recent_digests.json"
            ),
            session_messages_path: format!(
                "sessions/{year}/{month}/{session_id}/conversation/messages.json"
            ),
        })
    }
}

fn persist_context_snapshots(
    runtime_home: &Path,
    session_dir: &Path,
    snapshot: &ContextSnapshotRecord,
) -> Result<(), RuntimeError> {
    write_json_file(
        &runtime_home.join("runtime/current/current_context.json"),
        snapshot,
    )?;

    let recent_path = session_dir.join("context/recent_contexts.json");
    let mut recent = read_json_or_empty::<ContextSnapshotRecord>(&recent_path)?;
    recent.push(snapshot.clone());
    trim_head(&mut recent, RECENT_CONTEXT_LIMIT);
    write_json_file(&recent_path, &recent)
}

fn persist_recent_digests(session_dir: &Path, digest: &DigestRecord) -> Result<(), RuntimeError> {
    let recent_path = session_dir.join("digests/recent_digests.json");
    let mut recent = read_json_or_empty::<DigestRecord>(&recent_path)?;
    recent.push(digest.clone());
    trim_head(&mut recent, RECENT_DIGEST_LIMIT);
    write_json_file(&recent_path, &recent)
}

fn persist_reasoning_views(
    runtime_home: &Path,
    session_dir: &Path,
    reasoning_view: &ReasoningViewRecord,
) -> Result<(), RuntimeError> {
    write_json_file(
        &runtime_home.join("runtime/current/current_reasoning_view.json"),
        reasoning_view,
    )?;
    let recent_path = session_dir.join("reasoning/recent_reasoning_views.json");
    let mut recent = read_json_or_empty::<ReasoningViewRecord>(&recent_path)?;
    recent.push(reasoning_view.clone());
    trim_head(&mut recent, RECENT_REASONING_LIMIT);
    write_json_file(&recent_path, &recent)?;
    write_json_file(&session_dir.join("reasoning/latest.json"), reasoning_view)
}

fn persist_tool_records(
    runtime_home: &Path,
    session_dir: &Path,
    tool_records: &[ToolExecutionRecord],
) -> Result<(), RuntimeError> {
    write_json_file(
        &runtime_home.join("runtime/current/current_tool_records.json"),
        tool_records,
    )?;
    let recent_path = session_dir.join("tools/recent_tool_records.json");
    let mut recent = read_json_or_empty::<ToolExecutionRecord>(&recent_path)?;
    recent.extend(tool_records.iter().cloned());
    trim_head(&mut recent, RECENT_TOOL_RECORD_LIMIT);
    write_json_file(&recent_path, &recent)?;
    write_json_file(&session_dir.join("tools/latest.json"), tool_records)
}

fn persist_closure_traces(
    runtime_home: &Path,
    session_dir: &Path,
    closure_trace: &ClosureTraceRecord,
) -> Result<(), RuntimeError> {
    write_json_file(
        &runtime_home.join("runtime/current/current_closure_trace.json"),
        closure_trace,
    )?;
    let recent_path = session_dir.join("closures/recent_closures.json");
    let mut recent = read_json_or_empty::<ClosureTraceRecord>(&recent_path)?;
    recent.push(closure_trace.clone());
    trim_head(&mut recent, RECENT_CLOSURE_LIMIT);
    write_json_file(&recent_path, &recent)?;
    write_json_file(&session_dir.join("closures/latest.json"), closure_trace)
}

fn persist_session_messages(session_dir: &Path, run: &ClosureRun) -> Result<(), RuntimeError> {
    let path = session_dir.join("conversation/messages.json");
    let mut messages = read_json_or_empty::<SessionMessageRecord>(&path)?;
    let session_id = run
        .context_snapshot
        .refs
        .session_id
        .clone()
        .unwrap_or_else(|| "session-m1".into());
    let task_id = run.context_snapshot.refs.task_id.clone();
    messages.push(SessionMessageRecord {
        message_id: format!("user-{}", run.context_snapshot.operation_id),
        role: "user".into(),
        content: run.context_snapshot.input.clone(),
        created_at: run.context_snapshot.captured_at.clone(),
        session_id: session_id.clone(),
        task_id: task_id.clone(),
        operation_id: Some(run.context_snapshot.operation_id.clone()),
        trace_id: Some(run.context_snapshot.trace_id.clone()),
        closure_id: Some(run.digest.closure_id.clone()),
    });
    messages.push(SessionMessageRecord {
        message_id: format!("assistant-{}", run.digest.closure_id),
        role: "assistant".into(),
        content: run.assistant_response_text.clone(),
        created_at: run.note.created_at.clone(),
        session_id,
        task_id,
        operation_id: Some(run.context_snapshot.operation_id.clone()),
        trace_id: Some(run.context_snapshot.trace_id.clone()),
        closure_id: Some(run.digest.closure_id.clone()),
    });
    trim_head(&mut messages, SESSION_MESSAGE_LIMIT);
    write_json_file(&path, &messages)
}

fn trim_head<T>(items: &mut Vec<T>, limit: usize) {
    if items.len() > limit {
        let drain_count = items.len() - limit;
        items.drain(0..drain_count);
    }
}

fn read_json_or_empty<T: for<'de> Deserialize<'de>>(path: &Path) -> Result<Vec<T>, RuntimeError> {
    match fs::read_to_string(path) {
        Ok(content) => Ok(serde_json::from_str(&content)?),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(source) => Err(RuntimeError::Io {
            path: path.display().to_string(),
            source,
        }),
    }
}

fn write_json_file<T: Serialize + ?Sized>(path: &Path, value: &T) -> Result<(), RuntimeError> {
    write_bytes(path, serde_json::to_vec_pretty(value)?.as_slice())
}

fn write_json_lines<T: Serialize>(path: &Path, values: &[T]) -> Result<(), RuntimeError> {
    let mut file = fs::File::create(path).map_err(|source| RuntimeError::Io {
        path: path.display().to_string(),
        source,
    })?;
    for value in values {
        let line = serde_json::to_string(value)?;
        writeln!(file, "{line}").map_err(|source| RuntimeError::Io {
            path: path.display().to_string(),
            source,
        })?;
    }
    file.flush().map_err(|source| RuntimeError::Io {
        path: path.display().to_string(),
        source,
    })
}

fn write_bytes(path: &Path, bytes: &[u8]) -> Result<(), RuntimeError> {
    fs::write(path, bytes).map_err(|source| RuntimeError::Io {
        path: path.display().to_string(),
        source,
    })
}

fn create_dir_all(path: &Path) -> Result<(), RuntimeError> {
    fs::create_dir_all(path).map_err(|source| RuntimeError::Io {
        path: path.display().to_string(),
        source,
    })
}

fn parse_turn_index(operation_id: &str) -> Option<u64> {
    operation_id
        .rsplit_once('-')
        .and_then(|(_, tail)| tail.parse::<u64>().ok())
}
