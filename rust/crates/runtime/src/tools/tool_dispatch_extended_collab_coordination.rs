use super::tool_dispatch::{
    ToolDispatchInput, ToolDispatchOutcome, failed_record, read_string, runtime_home_from_context,
    short_text,
};
use crate::{AssignmentRecord, append_assignment_record, target_agent_name_from_worker_id};
use fin_contracts::ToolExecutionRecord;
use serde_json::{Value, json};
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
};

pub(super) fn handle_agent_assign(
    outcome: &mut ToolDispatchOutcome,
    input: &ToolDispatchInput<'_>,
    tool_call_id: &str,
    arguments: &Value,
) -> bool {
    let target_worker_id = read_string(arguments, "target_worker_id");
    let Some(peer_id) = read_string(arguments, "peer_id")
        .or_else(|| read_string(arguments, "target_peer_id"))
        .or_else(|| {
            target_worker_id
                .as_ref()
                .map(|value| format!("local-{value}"))
        })
    else {
        outcome.tool_records.push(failed_record(
            input,
            tool_call_id.into(),
            "agent.assign",
            "missing required argument: peer_id or target_worker_id",
        ));
        return true;
    };
    let Some(task_summary) = read_string(arguments, "task_summary")
        .or_else(|| read_string(arguments, "subtask"))
        .or_else(|| read_string(arguments, "instruction"))
    else {
        outcome.tool_records.push(failed_record(
            input,
            tool_call_id.into(),
            "agent.assign",
            "missing required argument: task_summary",
        ));
        return true;
    };
    let Some(runtime_home) = runtime_home_from_context(input.context) else {
        outcome.tool_records.push(failed_record(
            input,
            tool_call_id.into(),
            "agent.assign",
            "missing context.project.runtime_home, cannot persist assignment queue",
        ));
        return true;
    };

    let assignment_id = format!("assign-{}-{tool_call_id}", input.operation_id);
    let project_id = input
        .context
        .project
        .as_ref()
        .and_then(|project| project.primary_project.as_ref())
        .map(|project| project.project_id.clone());
    let assignment = AssignmentRecord {
        assignment_id: assignment_id.clone(),
        peer_id: peer_id.clone(),
        project_id,
        session_id: input.refs.session_id.clone(),
        task_id: read_string(arguments, "task_id").or_else(|| input.refs.task_id.clone()),
        target_worker_id: target_worker_id.clone(),
        target_agent_name: target_worker_id
            .as_deref()
            .and_then(target_agent_name_from_worker_id),
        requested_role_id: "project".into(),
        owner_worker_id: input.refs.worker_id.clone(),
        task_summary: task_summary.clone(),
        created_at: input.occurred_at.into(),
        status: "pending".into(),
        ..AssignmentRecord::default()
    };
    if let Err(err) = append_assignment_record(&runtime_home, assignment.clone()) {
        outcome.tool_records.push(failed_record(
            input,
            tool_call_id.into(),
            "agent.assign",
            format!("failed to persist assignment queue: {err}").as_str(),
        ));
        return true;
    }
    let pending_path = runtime_home.join("runtime/assignments/pending.json");

    outcome.tool_records.push(ToolExecutionRecord {
        tool_call_id: tool_call_id.into(),
        operation_id: input.operation_id.into(),
        trace_id: input.trace_id.into(),
        refs: input.refs.clone(),
        tool_name: "agent.assign".into(),
        tool_kind: "agent_tool".into(),
        title: "Agent Assign".into(),
        purpose: "request bounded subtask delegation to a target agent peer".into(),
        target_kind: Some("agent_peer".into()),
        target_ref: Some(peer_id.clone()),
        input_summary: Some(short_text(
            format!(
                "peer_id={peer_id}{}; task={task_summary}",
                target_worker_id
                    .as_ref()
                    .map(|value| format!(", target_worker_id={value}"))
                    .unwrap_or_default()
            )
            .as_str(),
            160,
        )),
        output_summary: Some(format!("assignment queued: {assignment_id}")),
        status: "completed".into(),
        started_at: input.occurred_at.into(),
        ended_at: Some(input.occurred_at.into()),
        duration_ms: Some(0),
        side_effects: vec!["append_assignment_queue".into()],
        artifact_refs: vec![relative_artifact(input.context, &pending_path)],
        error_summary: None,
    });
    outcome.events.push((
        "agent.assignment_requested".into(),
        json!({
            "tool_call_id": tool_call_id,
            "assignment_id": assignment_id,
            "peer_id": peer_id,
            "project_id": assignment.project_id,
            "session_id": assignment.session_id,
            "task_id": assignment.task_id,
            "target_worker_id": target_worker_id,
            "requested_role_id": "project",
            "owner_worker_id": input.refs.worker_id,
            "task_summary": short_text(task_summary.as_str(), 240),
        }),
    ));
    outcome
        .note_hints
        .push(format!("assignment queued for {peer_id}"));
    true
}

pub(super) fn handle_capability_invoke(
    outcome: &mut ToolDispatchOutcome,
    input: &ToolDispatchInput<'_>,
    tool_call_id: &str,
    arguments: &Value,
) -> bool {
    let Some(capability_id) = read_string(arguments, "capability_id") else {
        outcome.tool_records.push(failed_record(
            input,
            tool_call_id.into(),
            "capability.invoke",
            "missing required argument: capability_id",
        ));
        return true;
    };
    let Some(runtime_home) = runtime_home_from_context(input.context) else {
        outcome.tool_records.push(failed_record(
            input,
            tool_call_id.into(),
            "capability.invoke",
            "missing context.project.runtime_home, cannot append capability invocation logs",
        ));
        return true;
    };

    let known = input.context.peer.as_ref().map(|block| {
        block
            .peers
            .iter()
            .any(|peer| peer.capability_ids.iter().any(|id| id == &capability_id))
    });
    if matches!(known, Some(false)) {
        outcome.tool_records.push(failed_record(
            input,
            tool_call_id.into(),
            "capability.invoke",
            format!(
                "capability_id '{}' not found in current peer context descriptors",
                capability_id
            )
            .as_str(),
        ));
        return true;
    }

    let invoke_payload = arguments
        .as_object()
        .and_then(|value| value.get("input"))
        .cloned()
        .unwrap_or(Value::Null);
    let invocation = json!({
        "invocation_id": format!("cap-{}-{tool_call_id}", input.operation_id),
        "operation_id": input.operation_id,
        "trace_id": input.trace_id,
        "tool_call_id": tool_call_id,
        "capability_id": capability_id,
        "input": invoke_payload,
        "placeholder_result": {
            "status": "accepted",
            "message": "M1 capability invoke bridge recorded; remote execution plane not connected yet"
        },
        "occurred_at": input.occurred_at,
    });
    let log_path = runtime_home.join("runtime/capabilities/invocations.jsonl");
    if let Err(err) = append_jsonl(&log_path, &invocation) {
        outcome.tool_records.push(failed_record(
            input,
            tool_call_id.into(),
            "capability.invoke",
            format!("failed to append invocation log: {err}").as_str(),
        ));
        return true;
    }

    outcome.tool_records.push(ToolExecutionRecord {
        tool_call_id: tool_call_id.into(),
        operation_id: input.operation_id.into(),
        trace_id: input.trace_id.into(),
        refs: input.refs.clone(),
        tool_name: "capability.invoke".into(),
        tool_kind: "agent_tool".into(),
        title: "Capability Invoke".into(),
        purpose: "invoke one capability endpoint via peer plane bridge".into(),
        target_kind: Some("capability".into()),
        target_ref: Some(capability_id.clone()),
        input_summary: Some(short_text(invocation["input"].to_string().as_str(), 140)),
        output_summary: Some(
            "invocation accepted (placeholder bridge, remote execution not wired in M1)".into(),
        ),
        status: "completed".into(),
        started_at: input.occurred_at.into(),
        ended_at: Some(input.occurred_at.into()),
        duration_ms: Some(0),
        side_effects: vec!["append_capability_invocation_log".into()],
        artifact_refs: vec![relative_artifact(input.context, &log_path)],
        error_summary: None,
    });
    outcome.events.push((
        "capability.invoke_requested".into(),
        json!({
            "tool_call_id": tool_call_id,
            "capability_id": capability_id,
        }),
    ));
    outcome.events.push((
        "capability.invoke_completed".into(),
        json!({
            "tool_call_id": tool_call_id,
            "capability_id": capability_id,
            "placeholder": true,
            "status": "accepted",
        }),
    ));
    outcome
        .note_hints
        .push(format!("capability.invoke accepted for {capability_id}"));
    true
}

fn append_jsonl(path: &Path, value: &Value) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|err| err.to_string())?;
    }
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|err| err.to_string())?;
    file.write_all(
        format!(
            "{}\n",
            serde_json::to_string(value).map_err(|err| err.to_string())?
        )
        .as_bytes(),
    )
    .map_err(|err| err.to_string())
}

fn relative_artifact(context: &fin_contracts::MinimalContextView, absolute: &Path) -> String {
    runtime_home_from_context(context)
        .and_then(|home| {
            absolute
                .strip_prefix(home)
                .ok()
                .map(|value| value.to_string_lossy().to_string())
        })
        .unwrap_or_else(|| absolute.display().to_string())
}

#[allow(dead_code)]
fn _path_join(base: &Path, suffix: &str) -> PathBuf {
    base.join(suffix)
}
