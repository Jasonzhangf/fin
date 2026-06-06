use crate::task::board_snapshot::{collect_tasks, read_task_status};
use super::tool_dispatch::{
    ToolDispatchInput, ToolDispatchOutcome, failed_record, read_string, read_u64,
    runtime_home_from_context, short_text,
};
use fin_contracts::ToolExecutionRecord;
use serde_json::{Value, json};

pub(super) fn handle_project_task_status(
    outcome: &mut ToolDispatchOutcome,
    input: &ToolDispatchInput<'_>,
    tool_call_id: &str,
    arguments: &Value,
) -> bool {
    let Some(runtime_home) = runtime_home_from_context(input.context) else {
        outcome.tool_records.push(failed_record(
            input,
            tool_call_id.into(),
            "project.task.status",
            "missing context.project.runtime_home, cannot inspect task status",
        ));
        return true;
    };
    let Some(task_id) = read_string(arguments, "task_id").or_else(|| input.refs.task_id.clone())
    else {
        outcome.tool_records.push(failed_record(
            input,
            tool_call_id.into(),
            "project.task.status",
            "missing task_id and current refs.task_id is unavailable",
        ));
        return true;
    };
    let tasks = match collect_tasks(&runtime_home) {
        Ok(value) => value,
        Err(error) => {
            outcome.tool_records.push(failed_record(
                input,
                tool_call_id.into(),
                "project.task.status",
                format!("failed to scan task registry: {error}").as_str(),
            ));
            return true;
        }
    };
    let Some(summary) = tasks.get(task_id.as_str()) else {
        outcome.tool_records.push(failed_record(
            input,
            tool_call_id.into(),
            "project.task.status",
            format!("task not found in runtime truth: {task_id}").as_str(),
        ));
        return true;
    };
    let detail = match read_task_status(summary) {
        Ok(value) => value,
        Err(error) => {
            outcome.tool_records.push(failed_record(
                input,
                tool_call_id.into(),
                "project.task.status",
                error.as_str(),
            ));
            return true;
        }
    };
    outcome.tool_records.push(ToolExecutionRecord {
        tool_call_id: tool_call_id.into(),
        operation_id: input.operation_id.into(),
        trace_id: input.trace_id.into(),
        refs: input.refs.clone(),
        tool_name: "project.task.status".into(),
        tool_kind: "agent_tool".into(),
        title: "Project Task Status".into(),
        purpose: "inspect task status from execution, routing, and plan artifacts".into(),
        target_kind: Some("task".into()),
        target_ref: Some(task_id.clone()),
        input_summary: Some(format!("task_id={task_id}")),
        output_summary: Some(detail.output_summary.clone()),
        status: "completed".into(),
        started_at: input.occurred_at.into(),
        ended_at: Some(input.occurred_at.into()),
        duration_ms: Some(0),
        side_effects: vec!["read_task_artifacts".into()],
        artifact_refs: detail.artifact_refs,
        error_summary: None,
    });
    outcome.events.push((
        "project.task.status_completed".into(),
        json!({
            "tool_call_id": tool_call_id,
            "task_id": task_id,
            "session_id": summary.session_id,
            "status": detail.status,
            "pending_input_count": detail.pending_input_count,
            "plan_step_count": detail.plan_step_count,
        }),
    ));
    outcome
        .note_hints
        .push(format!("project.task.status {}", detail.output_summary));
    true
}

pub(super) fn handle_project_task_list(
    outcome: &mut ToolDispatchOutcome,
    input: &ToolDispatchInput<'_>,
    tool_call_id: &str,
    arguments: &Value,
) -> bool {
    let Some(runtime_home) = runtime_home_from_context(input.context) else {
        outcome.tool_records.push(failed_record(
            input,
            tool_call_id.into(),
            "project.task.list",
            "missing context.project.runtime_home, cannot list tasks",
        ));
        return true;
    };
    let limit = read_u64(arguments, "limit").unwrap_or(10).clamp(1, 50) as usize;
    let tasks = match collect_tasks(&runtime_home) {
        Ok(value) => value,
        Err(error) => {
            outcome.tool_records.push(failed_record(
                input,
                tool_call_id.into(),
                "project.task.list",
                format!("failed to scan task registry: {error}").as_str(),
            ));
            return true;
        }
    };
    let mut ids = tasks.keys().cloned().collect::<Vec<_>>();
    ids.sort();
    ids.reverse();
    ids.truncate(limit);
    let preview = ids
        .iter()
        .filter_map(|task_id| {
            tasks
                .get(task_id)
                .map(|item| format!("{task_id}@{}", item.session_id))
        })
        .collect::<Vec<_>>()
        .join(", ");
    outcome.tool_records.push(ToolExecutionRecord {
        tool_call_id: tool_call_id.into(),
        operation_id: input.operation_id.into(),
        trace_id: input.trace_id.into(),
        refs: input.refs.clone(),
        tool_name: "project.task.list".into(),
        tool_kind: "agent_tool".into(),
        title: "Project Task List".into(),
        purpose: "list known task ids from runtime/session truth".into(),
        target_kind: Some("task_registry".into()),
        target_ref: Some("runtime_home/sessions".into()),
        input_summary: Some(format!("limit={limit}")),
        output_summary: Some(format!(
            "tasks={} [{}]",
            ids.len(),
            short_text(&preview, 160)
        )),
        status: "completed".into(),
        started_at: input.occurred_at.into(),
        ended_at: Some(input.occurred_at.into()),
        duration_ms: Some(0),
        side_effects: vec!["read_task_registry".into()],
        artifact_refs: vec!["sessions".into()],
        error_summary: None,
    });
    outcome.events.push((
        "project.task.list_completed".into(),
        json!({
            "tool_call_id": tool_call_id,
            "count": ids.len(),
            "task_ids": ids,
        }),
    ));
    outcome
        .note_hints
        .push(format!("project.task.list returned {} task(s)", ids.len()));
    true
}
