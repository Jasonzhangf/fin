use super::dispatch::{
    ToolDispatchInput, ToolDispatchOutcome, failed_record, read_string, read_u64,
    runtime_home_from_context, short_text,
};
use fin_contracts::ToolExecutionRecord;
use serde_json::{Value, json};
use std::{
    fs,
    path::{Path, PathBuf},
};

pub(super) fn handle_wait_remind(
    outcome: &mut ToolDispatchOutcome,
    input: &ToolDispatchInput<'_>,
    tool_call_id: &str,
    arguments: &Value,
) -> bool {
    let Some(wait_minutes) = read_u64(arguments, "wait_minutes") else {
        outcome.tool_records.push(failed_record(
            input,
            tool_call_id.into(),
            "wait.remind",
            "missing required argument: wait_minutes",
        ));
        return true;
    };
    if wait_minutes == 0 {
        outcome.tool_records.push(failed_record(
            input,
            tool_call_id.into(),
            "wait.remind",
            "wait_minutes must be greater than 0",
        ));
        return true;
    }
    let Some(reminder) = read_string(arguments, "reminder") else {
        outcome.tool_records.push(failed_record(
            input,
            tool_call_id.into(),
            "wait.remind",
            "missing required argument: reminder",
        ));
        return true;
    };

    let reminder_id = format!("reminder-{}-{tool_call_id}", input.operation_id);
    outcome.tool_records.push(ToolExecutionRecord {
        tool_call_id: tool_call_id.into(),
        operation_id: input.operation_id.into(),
        trace_id: input.trace_id.into(),
        refs: input.refs.clone(),
        tool_name: "wait.remind".into(),
        tool_kind: "agent_tool".into(),
        title: "Wait + Self Reminder".into(),
        purpose:
            "schedule an async self reminder so system role can continue later without busy waiting"
                .into(),
        target_kind: Some("system_self_wakeup".into()),
        target_ref: Some("role=system".into()),
        input_summary: Some(format!("wait_minutes={wait_minutes}, reminder={reminder}")),
        output_summary: Some(format!(
            "scheduled system self reminder in {wait_minutes} minute(s)"
        )),
        status: "completed".into(),
        started_at: input.occurred_at.into(),
        ended_at: Some(input.occurred_at.into()),
        duration_ms: Some(0),
        side_effects: vec![
            "schedule_async_reminder".into(),
            "inject_future_system_message".into(),
        ],
        artifact_refs: vec![
            "runtime/reminders/pending.json".into(),
            "conversation/messages.json".into(),
        ],
        error_summary: None,
    });
    outcome.events.push((
        "system.reminder_scheduled".into(),
        json!({
            "reminder_id": reminder_id,
            "tool_call_id": tool_call_id,
            "wait_minutes": wait_minutes,
            "reminder": reminder,
            "wake_role": "system",
            "scheduled_at": input.occurred_at,
            "operation_id": input.operation_id,
            "trace_id": input.trace_id,
            "session_id": input.refs.session_id,
            "task_id": input.refs.task_id,
        }),
    ));
    outcome.note_hints.push(format!(
        "scheduled system self reminder in {wait_minutes} minute(s): {reminder}"
    ));
    outcome.reminder_scheduled = true;
    outcome.yield_requested = true;
    true
}

pub(super) fn handle_reasoning_stop(
    outcome: &mut ToolDispatchOutcome,
    input: &ToolDispatchInput<'_>,
    tool_call_id: &str,
    arguments: &Value,
) -> bool {
    let summary = read_string(arguments, "summary")
        .or_else(|| read_string(arguments, "reason"))
        .unwrap_or_else(|| "model declared reasoning stop".into());
    outcome.tool_records.push(ToolExecutionRecord {
        tool_call_id: tool_call_id.into(),
        operation_id: input.operation_id.into(),
        trace_id: input.trace_id.into(),
        refs: input.refs.clone(),
        tool_name: "reasoning.stop".into(),
        tool_kind: "agent_tool".into(),
        title: "Reasoning Stop".into(),
        purpose: "explicitly close the current reasoning cycle".into(),
        target_kind: Some("reasoning_closure".into()),
        target_ref: Some("current_turn".into()),
        input_summary: Some(summary.clone()),
        output_summary: Some("reasoning stop requested; runtime closure gate pending".into()),
        status: "completed".into(),
        started_at: input.occurred_at.into(),
        ended_at: Some(input.occurred_at.into()),
        duration_ms: Some(0),
        side_effects: vec!["mark_reasoning_closure_stop".into()],
        artifact_refs: vec!["events/stream.jsonl".into()],
        error_summary: None,
    });
    outcome.events.push((
        "reasoning.stopped".into(),
        json!({
            "tool_call_id": tool_call_id,
            "summary": summary,
            "source": "model_tool_call",
        }),
    ));
    outcome
        .note_hints
        .push("reasoning.stop requested; runtime will decide whether closure is valid".into());
    outcome.stop_requested = true;
    true
}

pub(super) fn handle_update_plan(
    outcome: &mut ToolDispatchOutcome,
    input: &ToolDispatchInput<'_>,
    tool_call_id: &str,
    arguments: &Value,
) -> bool {
    let Some(runtime_home) = runtime_home_from_context(input.context) else {
        outcome.tool_records.push(failed_record(
            input,
            tool_call_id.into(),
            "update_plan",
            "missing context.project.runtime_home, cannot persist plan artifact",
        ));
        return true;
    };
    let Some(steps) = extract_plan_steps(arguments) else {
        outcome.tool_records.push(failed_record(
            input,
            tool_call_id.into(),
            "update_plan",
            "missing required argument: steps[{step,status}]",
        ));
        return true;
    };
    let explanation = read_string(arguments, "explanation");
    let plan = json!({
        "plan_id": format!("plan-{}-{tool_call_id}", input.operation_id),
        "operation_id": input.operation_id,
        "trace_id": input.trace_id,
        "session_id": input.refs.session_id.clone(),
        "task_id": input.refs.task_id.clone(),
        "updated_at": input.occurred_at,
        "explanation": explanation,
        "steps": steps,
    });
    let mut artifacts = Vec::new();
    let runtime_path = runtime_home.join("runtime/current/current_plan_update.json");
    if let Err(err) = write_json(&runtime_path, &plan) {
        outcome.tool_records.push(failed_record(
            input,
            tool_call_id.into(),
            "update_plan",
            format!("failed to persist runtime plan artifact: {err}").as_str(),
        ));
        return true;
    }
    artifacts.push(relative_artifact(&runtime_home, &runtime_path));
    if let Some(session_dir) = session_dir_for_refs(&runtime_home, input.refs.session_id.as_deref())
    {
        let session_path = session_dir.join("tasks/plan/latest.json");
        if let Err(err) = write_json(&session_path, &plan) {
            outcome.tool_records.push(failed_record(
                input,
                tool_call_id.into(),
                "update_plan",
                format!("failed to persist session plan artifact: {err}").as_str(),
            ));
            return true;
        }
        artifacts.push(relative_artifact(&runtime_home, &session_path));
    }
    let step_count = plan["steps"]
        .as_array()
        .map(|items| items.len())
        .unwrap_or(0);
    outcome.tool_records.push(ToolExecutionRecord {
        tool_call_id: tool_call_id.into(),
        operation_id: input.operation_id.into(),
        trace_id: input.trace_id.into(),
        refs: input.refs.clone(),
        tool_name: "update_plan".into(),
        tool_kind: "agent_tool".into(),
        title: "Update Plan".into(),
        purpose: "persist a structured execution plan update".into(),
        target_kind: Some("plan".into()),
        target_ref: input.refs.task_id.clone().or(input.refs.session_id.clone()),
        input_summary: Some(format!(
            "steps={step_count}{}",
            explanation
                .as_deref()
                .map(|v| format!(", explanation={}", short_text(v, 80)))
                .unwrap_or_default()
        )),
        output_summary: Some(format!("plan updated with {step_count} step(s)")),
        status: "completed".into(),
        started_at: input.occurred_at.into(),
        ended_at: Some(input.occurred_at.into()),
        duration_ms: Some(0),
        side_effects: vec!["persist_plan_artifact".into()],
        artifact_refs: artifacts,
        error_summary: None,
    });
    outcome.events.push((
        "plan.updated".into(),
        json!({
            "tool_call_id": tool_call_id,
            "step_count": step_count,
            "session_id": input.refs.session_id.clone(),
            "task_id": input.refs.task_id.clone(),
        }),
    ));
    outcome
        .note_hints
        .push(format!("plan updated: {step_count} step(s)"));
    true
}

pub(super) fn handle_session_list(
    outcome: &mut ToolDispatchOutcome,
    input: &ToolDispatchInput<'_>,
    tool_call_id: &str,
    arguments: &Value,
) -> bool {
    let Some(runtime_home) = runtime_home_from_context(input.context) else {
        outcome.tool_records.push(failed_record(
            input,
            tool_call_id.into(),
            "session.list",
            "missing context.project.runtime_home, cannot inspect sessions",
        ));
        return true;
    };
    let limit = read_u64(arguments, "limit").unwrap_or(10).clamp(1, 50) as usize;
    let sessions = match list_sessions(&runtime_home, limit) {
        Ok(items) => items,
        Err(err) => {
            outcome.tool_records.push(failed_record(
                input,
                tool_call_id.into(),
                "session.list",
                format!("failed to list sessions: {err}").as_str(),
            ));
            return true;
        }
    };
    let preview = if sessions.is_empty() {
        "no sessions found".into()
    } else {
        sessions.join(", ")
    };
    outcome.tool_records.push(ToolExecutionRecord {
        tool_call_id: tool_call_id.into(),
        operation_id: input.operation_id.into(),
        trace_id: input.trace_id.into(),
        refs: input.refs.clone(),
        tool_name: "session.list".into(),
        tool_kind: "agent_tool".into(),
        title: "Session List".into(),
        purpose: "list recently known sessions under the current runtime home".into(),
        target_kind: Some("session_registry".into()),
        target_ref: Some("runtime_home/sessions".into()),
        input_summary: Some(format!("limit={limit}")),
        output_summary: Some(format!(
            "sessions={} [{}]",
            sessions.len(),
            short_text(&preview, 160)
        )),
        status: "completed".into(),
        started_at: input.occurred_at.into(),
        ended_at: Some(input.occurred_at.into()),
        duration_ms: Some(0),
        side_effects: vec!["read_session_registry".into()],
        artifact_refs: vec!["sessions".into()],
        error_summary: None,
    });
    outcome.events.push((
        "session.list_completed".into(),
        json!({
            "tool_call_id": tool_call_id,
            "count": sessions.len(),
            "sessions": sessions,
        }),
    ));
    outcome.note_hints.push(format!(
        "session.list returned {} session(s)",
        sessions.len()
    ));
    true
}

fn extract_plan_steps(arguments: &Value) -> Option<Vec<Value>> {
    let steps = arguments.as_object()?.get("steps")?.as_array()?;
    let normalized = steps
        .iter()
        .filter_map(|item| {
            let object = item.as_object()?;
            let step = object.get("step").and_then(Value::as_str)?.trim();
            let status = object.get("status").and_then(Value::as_str)?.trim();
            if step.is_empty() || status.is_empty() {
                return None;
            }
            Some(json!({ "step": step, "status": status }))
        })
        .collect::<Vec<_>>();
    (!normalized.is_empty()).then_some(normalized)
}

fn write_json(path: &Path, value: &Value) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|err| err.to_string())?;
    }
    fs::write(
        path,
        serde_json::to_vec_pretty(value).map_err(|err| err.to_string())?,
    )
    .map_err(|err| err.to_string())
}

fn relative_artifact(runtime_home: &Path, path: &Path) -> String {
    path.strip_prefix(runtime_home)
        .map(|relative| relative.display().to_string())
        .unwrap_or_else(|_| path.display().to_string())
}

fn session_dir_for_refs(runtime_home: &Path, session_id: Option<&str>) -> Option<PathBuf> {
    let session_id = session_id?.trim();
    if session_id.is_empty() {
        return None;
    }
    let sessions_root = runtime_home.join("sessions");
    let years = fs::read_dir(&sessions_root).ok()?;
    for year in years.flatten() {
        let year_path = year.path();
        if !year_path.is_dir() {
            continue;
        }
        for month in fs::read_dir(&year_path).ok()?.flatten() {
            let month_path = month.path();
            if !month_path.is_dir() {
                continue;
            }
            let candidate = month_path.join(session_id);
            if candidate.is_dir() {
                return Some(candidate);
            }
        }
    }
    None
}

fn list_sessions(runtime_home: &Path, limit: usize) -> Result<Vec<String>, String> {
    let sessions_root = runtime_home.join("sessions");
    let mut sessions = Vec::new();
    if !sessions_root.exists() {
        return Ok(sessions);
    }
    for year in fs::read_dir(&sessions_root).map_err(|err| err.to_string())? {
        let year = year.map_err(|err| err.to_string())?;
        let year_path = year.path();
        if !year_path.is_dir() {
            continue;
        }
        for month in fs::read_dir(&year_path).map_err(|err| err.to_string())? {
            let month = month.map_err(|err| err.to_string())?;
            let month_path = month.path();
            if !month_path.is_dir() {
                continue;
            }
            for session in fs::read_dir(&month_path).map_err(|err| err.to_string())? {
                let session = session.map_err(|err| err.to_string())?;
                if session.path().is_dir() {
                    sessions.push(session.file_name().to_string_lossy().to_string());
                }
            }
        }
    }
    sessions.sort();
    sessions.reverse();
    sessions.truncate(limit);
    Ok(sessions)
}
