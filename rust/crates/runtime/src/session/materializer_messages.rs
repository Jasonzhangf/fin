use crate::{ClosureRun, RuntimeError};
use fin_config::RuntimeRetentionConfig;
use fin_contracts::EventEnvelope;
use serde_json::Value;
use std::path::Path;

use super::materializer::SessionMessageRecord;
use super::materializer_support::{
    PendingReminderRecord, create_dir_all, parse_scheduled_reminder_payload, read_json_or_empty,
    trim_head, write_json_file,
};

pub(super) fn persist_session_messages(
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

pub(super) fn assistant_display_name(role_id: &str, agent_name: Option<&str>) -> String {
    match (role_id, agent_name.filter(|value| !value.trim().is_empty())) {
        ("system", Some(name)) if name != "system" => format!("System Agent · {name}"),
        ("system", _) => "System Agent".into(),
        ("project", Some(name)) => format!("Project Agent · {name}"),
        ("project", _) => "Project Agent".into(),
        (other, Some(name)) => format!("{other} · {name}"),
        (other, None) => format!("{other} agent"),
    }
}

pub(super) fn persist_scheduled_reminders(
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
