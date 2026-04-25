use crate::{ClosureRun, RuntimeError, session_record_journal, uses_ephemeral_session_persistence};
use fin_config::RuntimeRetentionConfig;
use fin_contracts::EventEnvelope;
use fin_contracts::{
    ClosureTraceRecord, ContextSnapshotRecord, DigestRecord, ExecutionCheckpointRecord,
    ReasoningViewRecord, ToolExecutionRecord,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use serde_json::json;
use std::{
    fs,
    path::{Path, PathBuf},
};

#[path = "session_materializer_events.rs"]
mod session_materializer_events;
#[path = "session_materializer_support.rs"]
mod session_materializer_support;
use session_materializer_events::persist_event_stream;
use session_materializer_support::{
    PendingReminderRecord, parse_scheduled_reminder_payload, parse_turn_index,
};
pub(crate) use session_materializer_support::{
    create_dir_all, read_json_or_empty, trim_head, write_bytes, write_json_file,
};

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
    #[serde(default)]
    pub sender_kind: Option<String>,
    #[serde(default)]
    pub role_id: Option<String>,
    #[serde(default)]
    pub agent_name: Option<String>,
    #[serde(default)]
    pub display_name: Option<String>,
    #[serde(default)]
    pub source_kind: Option<String>,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SessionPersistenceMode {
    Normal,
    EphemeralControlPlane,
}

impl SessionPersistenceMode {
    fn persist_session_history(self) -> bool {
        matches!(self, Self::Normal)
    }

    fn persist_last_run(self) -> bool {
        matches!(self, Self::Normal)
    }
}

impl SessionMaterializer {
    pub fn persist(
        &self,
        runtime_home: &Path,
        run: &ClosureRun,
        retention: &RuntimeRetentionConfig,
    ) -> Result<SessionMaterializationReceipt, RuntimeError> {
        let persistence_mode = if (run.conversation_user_input.is_none()
            && !run.context_snapshot.input.trim().is_empty())
            || uses_ephemeral_session_persistence(run.operation.source.as_str())
        {
            SessionPersistenceMode::EphemeralControlPlane
        } else {
            SessionPersistenceMode::Normal
        };
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
            "provider",
            "rounds",
            "steps",
            "turns",
            "closures",
            "collab",
            "tasks",
            "tasks/routing",
            "topics",
            "artifacts/candidates",
        ] {
            create_dir_all(&session_dir.join(relative))?;
        }

        if persistence_mode.persist_session_history() {
            persist_event_stream(
                runtime_home,
                &session_dir,
                year,
                month,
                &session_id,
                &run.events,
                retention,
            )?;
        }
        write_json_file(&session_dir.join("progress/latest.json"), &run.progress)?;
        write_json_file(
            &session_dir.join("control/latest.json"),
            &run.control_feedback,
        )?;
        write_json_file(&session_dir.join("notes/latest.json"), &run.note)?;
        if persistence_mode.persist_session_history() {
            write_json_file(&session_dir.join("digests/latest.json"), &run.digest)?;
        }
        persist_recent_digests(
            &session_dir,
            &run.digest,
            retention,
            persistence_mode.persist_session_history(),
        )?;
        persist_context_snapshots(
            runtime_home,
            &session_dir,
            &run.context_snapshot,
            retention,
            persistence_mode.persist_session_history(),
        )?;
        persist_reasoning_views(
            runtime_home,
            &session_dir,
            &run.reasoning_view,
            retention,
            persistence_mode.persist_session_history(),
        )?;
        persist_tool_records(
            runtime_home,
            &session_dir,
            &run.tool_records,
            retention,
            persistence_mode.persist_session_history(),
        )?;
        persist_closure_traces(
            runtime_home,
            &session_dir,
            &run.closure_trace,
            retention,
            persistence_mode.persist_session_history(),
        )?;
        persist_execution_checkpoint(runtime_home, &session_dir, run.resume_checkpoint.as_ref())?;
        let journal_paths = session_record_journal::persist_extended_records(
            runtime_home,
            &session_dir,
            year,
            month,
            &session_id,
            run,
            retention,
            persistence_mode.persist_session_history(),
        )?;
        persist_scheduled_reminders(runtime_home, &run.events, retention)?;
        write_json_file(
            &runtime_home.join("runtime/current/current_control_feedback.json"),
            &run.control_feedback,
        )?;
        if persistence_mode.persist_session_history() {
            persist_session_messages(runtime_home, &session_dir, run, retention)?;
        }

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
        let mut last_run = json!({
                "operation_id": run.operation.operation_id,
                "trace_id": run.operation.trace_id,
                "turn_index": parse_turn_index(&run.operation.operation_id),
                "session_id": session_id,
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
                "current_turn_record_path": "runtime/current/current_turn.json",
                "current_step_records_path": "runtime/current/current_step_records.json",
                "current_provider_requests_path": "runtime/current/current_provider_requests.json",
                "current_provider_responses_path": "runtime/current/current_provider_responses.json",
                "current_rounds_path": "runtime/current/current_rounds.json",
                "current_routing_decision_path": "runtime/current/current_routing_decision.json",
                "current_routing_action_path": "runtime/current/current_routing_action.json",
                "current_execution_checkpoint_path": run.resume_checkpoint.as_ref().map(|_| "runtime/current/current_execution_checkpoint.json"),
                "current_event_archive_index_path": "runtime/current/current_event_archive_index.json",
                "session_control_feedback_path": format!("sessions/{year}/{month}/{session_id}/control/latest.json"),
                "session_recent_contexts_path": session_recent_contexts_path,
                "session_recent_digests_path": session_recent_digests_path,
                "session_recent_reasoning_path": session_recent_reasoning_path,
                "session_recent_tool_records_path": session_recent_tool_records_path,
                "session_recent_execution_checkpoints_path": format!("sessions/{year}/{month}/{session_id}/control/recent_execution_checkpoints.json"),
                "session_recent_closures_path": session_recent_closures_path,
                "session_recent_provider_requests_path": journal_paths.session_recent_provider_requests_path,
                "session_recent_provider_responses_path": journal_paths.session_recent_provider_responses_path,
                "session_recent_rounds_path": journal_paths.session_recent_rounds_path,
                "session_recent_step_records_path": journal_paths.session_recent_step_records_path,
                "session_recent_turns_path": journal_paths.session_recent_turns_path,
                "session_recent_routing_decisions_path": journal_paths.session_recent_routing_decisions_path,
                "session_recent_routing_actions_path": journal_paths.session_recent_routing_actions_path,
                "session_event_archive_index_path": format!("sessions/{year}/{month}/{session_id}/events/archive_index.json"),
                "session_messages_path": session_messages_path,
        });
        let object = last_run.as_object_mut().expect("last_run object");
        if let Some(task_id) = run.progress.refs.task_id.as_ref() {
            object.insert("task_id".into(), Value::String(task_id.clone()));
        }
        if let Some(topic_thread_id) = run.progress.refs.topic_thread_id.as_ref() {
            object.insert(
                "topic_thread_id".into(),
                Value::String(topic_thread_id.clone()),
            );
        }
        if persistence_mode.persist_last_run() {
            write_bytes(
                &runtime_home.join("runtime/current/last_run.json"),
                serde_json::to_vec_pretty(&last_run)?.as_slice(),
            )?;
        }

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

pub fn append_framework_events(
    runtime_home: &Path,
    session_dir: &Path,
    events: &[EventEnvelope<Value>],
    retention: &RuntimeRetentionConfig,
) -> Result<(), RuntimeError> {
    let _ = (runtime_home, session_dir, events, retention);
    Ok(())
}

fn persist_context_snapshots(
    runtime_home: &Path,
    session_dir: &Path,
    snapshot: &ContextSnapshotRecord,
    retention: &RuntimeRetentionConfig,
    persist_session_history: bool,
) -> Result<(), RuntimeError> {
    write_json_file(
        &runtime_home.join("runtime/current/current_context.json"),
        snapshot,
    )?;
    if !persist_session_history {
        return Ok(());
    }

    let recent_path = session_dir.join("context/recent_contexts.json");
    let mut recent = read_json_or_empty::<ContextSnapshotRecord>(&recent_path)?;
    recent.push(snapshot.clone());
    trim_head(&mut recent, retention.recent_context_limit);
    write_json_file(&recent_path, &recent)
}

fn persist_recent_digests(
    session_dir: &Path,
    digest: &DigestRecord,
    retention: &RuntimeRetentionConfig,
    persist_session_history: bool,
) -> Result<(), RuntimeError> {
    if !persist_session_history {
        return Ok(());
    }
    let recent_path = session_dir.join("digests/recent_digests.json");
    let mut recent = read_json_or_empty::<DigestRecord>(&recent_path)?;
    recent.push(digest.clone());
    trim_head(&mut recent, retention.recent_digest_limit);
    write_json_file(&recent_path, &recent)
}

fn persist_reasoning_views(
    runtime_home: &Path,
    session_dir: &Path,
    reasoning_view: &ReasoningViewRecord,
    retention: &RuntimeRetentionConfig,
    persist_session_history: bool,
) -> Result<(), RuntimeError> {
    write_json_file(
        &runtime_home.join("runtime/current/current_reasoning_view.json"),
        reasoning_view,
    )?;
    if !persist_session_history {
        return Ok(());
    }
    let recent_path = session_dir.join("reasoning/recent_reasoning_views.json");
    let mut recent = read_json_or_empty::<ReasoningViewRecord>(&recent_path)?;
    recent.push(reasoning_view.clone());
    trim_head(&mut recent, retention.recent_reasoning_limit);
    write_json_file(&recent_path, &recent)?;
    write_json_file(&session_dir.join("reasoning/latest.json"), reasoning_view)
}

fn persist_tool_records(
    runtime_home: &Path,
    session_dir: &Path,
    tool_records: &[ToolExecutionRecord],
    retention: &RuntimeRetentionConfig,
    persist_session_history: bool,
) -> Result<(), RuntimeError> {
    write_json_file(
        &runtime_home.join("runtime/current/current_tool_records.json"),
        tool_records,
    )?;
    if !persist_session_history {
        return Ok(());
    }
    let recent_path = session_dir.join("tools/recent_tool_records.json");
    let mut recent = read_json_or_empty::<ToolExecutionRecord>(&recent_path)?;
    recent.extend(tool_records.iter().cloned());
    trim_head(&mut recent, retention.recent_tool_record_limit);
    write_json_file(&recent_path, &recent)?;
    write_json_file(&session_dir.join("tools/latest.json"), tool_records)
}

fn persist_closure_traces(
    runtime_home: &Path,
    session_dir: &Path,
    closure_trace: &ClosureTraceRecord,
    retention: &RuntimeRetentionConfig,
    persist_session_history: bool,
) -> Result<(), RuntimeError> {
    write_json_file(
        &runtime_home.join("runtime/current/current_closure_trace.json"),
        closure_trace,
    )?;
    if !persist_session_history {
        return Ok(());
    }
    let recent_path = session_dir.join("closures/recent_closures.json");
    let mut recent = read_json_or_empty::<ClosureTraceRecord>(&recent_path)?;
    recent.push(closure_trace.clone());
    trim_head(&mut recent, retention.recent_closure_limit);
    write_json_file(&recent_path, &recent)?;
    write_json_file(&session_dir.join("closures/latest.json"), closure_trace)
}

fn persist_execution_checkpoint(
    runtime_home: &Path,
    session_dir: &Path,
    checkpoint: Option<&ExecutionCheckpointRecord>,
) -> Result<(), RuntimeError> {
    let runtime_path = runtime_home.join("runtime/current/current_execution_checkpoint.json");
    let session_path = session_dir.join("control/execution_checkpoint.json");
    let recent_path = session_dir.join("control/recent_execution_checkpoints.json");
    match checkpoint {
        Some(value) => {
            write_json_file(&runtime_path, value)?;
            write_json_file(&session_path, value)?;
            let mut recent = read_json_or_empty::<ExecutionCheckpointRecord>(&recent_path)?;
            recent.retain(|item| item.checkpoint_id != value.checkpoint_id);
            recent.push(value.clone());
            trim_head(&mut recent, 16);
            write_json_file(&recent_path, &recent)
        }
        None => {
            let _ = fs::remove_file(runtime_path);
            let _ = fs::remove_file(session_path);
            Ok(())
        }
    }
}

fn persist_session_messages(
    runtime_home: &Path,
    session_dir: &Path,
    run: &ClosureRun,
    retention: &RuntimeRetentionConfig,
) -> Result<(), RuntimeError> {
    let path = session_dir.join("conversation/messages.json");
    let mut messages = read_json_or_empty::<SessionMessageRecord>(&path)?;
    let session_id = run
        .context_snapshot
        .refs
        .session_id
        .clone()
        .unwrap_or_else(|| "session-m1".into());
    let task_id = run.context_snapshot.refs.task_id.clone();
    if let Some(user_input) = run.conversation_user_input.as_ref() {
        messages.push(SessionMessageRecord {
            message_id: format!("user-{}", run.context_snapshot.operation_id),
            role: "user".into(),
            content: user_input.clone(),
            created_at: run.context_snapshot.captured_at.clone(),
            session_id: session_id.clone(),
            task_id: task_id.clone(),
            operation_id: Some(run.context_snapshot.operation_id.clone()),
            trace_id: Some(run.context_snapshot.trace_id.clone()),
            closure_id: Some(run.digest.closure_id.clone()),
            sender_kind: Some("user_message".into()),
            role_id: None,
            agent_name: None,
            display_name: Some("User".into()),
            source_kind: Some("conversation_message".into()),
        });
    }
    let worker_id = run.progress.refs.worker_id.as_deref();
    let identity = worker_id
        .map(|worker_id| crate::resolve_agent_identity_by_worker_id(runtime_home, worker_id))
        .transpose()?
        .flatten();
    let role_id = run.operation.payload.role.role_id.as_str().to_string();
    let agent_name = identity.as_ref().map(|value| value.agent_name.clone());
    if !run.assistant_response_text.trim().is_empty() {
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
            sender_kind: Some("agent_reply".into()),
            role_id: Some(role_id.clone()),
            agent_name: agent_name.clone(),
            display_name: Some(assistant_display_name(
                role_id.as_str(),
                agent_name.as_deref(),
            )),
            source_kind: Some("session_message".into()),
        });
    }
    trim_head(&mut messages, retention.session_message_limit);
    write_json_file(&path, &messages)
}

fn assistant_display_name(role_id: &str, agent_name: Option<&str>) -> String {
    match (role_id, agent_name.filter(|value| !value.trim().is_empty())) {
        ("system", Some(name)) if name != "system" => format!("System Agent · {name}"),
        ("system", _) => "System Agent".into(),
        ("project", Some(name)) => format!("Project Agent · {name}"),
        ("project", _) => "Project Agent".into(),
        (other, Some(name)) => format!("{other} · {name}"),
        (other, None) => format!("{other} agent"),
    }
}

fn persist_scheduled_reminders(
    runtime_home: &Path,
    events: &[EventEnvelope<Value>],
    retention: &RuntimeRetentionConfig,
) -> Result<(), RuntimeError> {
    let reminders = events
        .iter()
        .filter(|event| event.event_type == "system.reminder_scheduled")
        .filter_map(|event| parse_scheduled_reminder_payload(&event.payload))
        .collect::<Vec<_>>();
    if reminders.is_empty() {
        return Ok(());
    }
    let reminders_dir = runtime_home.join("runtime/reminders");
    create_dir_all(&reminders_dir)?;
    let pending_path = reminders_dir.join("pending.json");
    let mut pending = read_json_or_empty::<PendingReminderRecord>(&pending_path)?;
    let mut changed = false;
    for reminder in reminders {
        if pending
            .iter()
            .any(|item| item.reminder_id == reminder.reminder_id)
        {
            continue;
        }
        pending.push(reminder);
        changed = true;
    }
    if !changed {
        return Ok(());
    }
    trim_head(&mut pending, retention.reminder_pending_limit);
    write_json_file(&pending_path, &pending)?;
    write_json_file(
        &runtime_home.join("runtime/current/current_reminders.json"),
        &pending,
    )?;
    Ok(())
}
