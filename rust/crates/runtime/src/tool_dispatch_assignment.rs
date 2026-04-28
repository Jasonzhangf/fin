use crate::{
    assignment_queue::{read_assignment_queue, update_assignment_record},
    tool_dispatch::{
        ToolDispatchInput, ToolDispatchOutcome, failed_record, persist_authoritative_tool_receipt,
        read_string, read_u64, runtime_home_from_context, short_text,
    },
};
use fin_contracts::ToolExecutionRecord;
use serde_json::{Value, json};

pub(super) fn handle_assignment_list(
    outcome: &mut ToolDispatchOutcome,
    input: &ToolDispatchInput<'_>,
    tool_call_id: &str,
    arguments: &Value,
) -> bool {
    let Some(runtime_home) = runtime_home_from_context(input.context) else {
        outcome.tool_records.push(failed_record(
            input,
            tool_call_id.into(),
            "assignment.list",
            "missing context.project.runtime_home, cannot inspect assignments",
        ));
        return true;
    };
    let limit = read_u64(arguments, "limit").unwrap_or(20).clamp(1, 100) as usize;
    let status_filter = read_string(arguments, "status")
        .unwrap_or_else(|| "pending".into());
    let assignments = match read_assignment_queue(&runtime_home) {
        Ok(items) => items,
        Err(err) => {
            outcome.tool_records.push(failed_record(
                input,
                tool_call_id.into(),
                "assignment.list",
                format!("failed to read assignment queue: {err}").as_str(),
            ));
            return true;
        }
    };
    let filtered: Vec<_> = assignments
        .iter()
        .filter(|a| status_filter == "all" || a.status == status_filter)
        .take(limit)
        .collect();
    let preview = if filtered.is_empty() {
        format!("no {status_filter} assignments found")
    } else {
        filtered
            .iter()
            .map(|a| {
                format!(
                    "{{id={}, task_id={}, peer_id={}, worker_id={}, summary={}}}",
                    short_text(&a.assignment_id, 60),
                    a.task_id.as_deref().unwrap_or("-"),
                    short_text(&a.peer_id, 30),
                    a.target_worker_id.as_deref().unwrap_or("-"),
                    short_text(&a.task_summary, 80),
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    };
    let pending_total = assignments.iter().filter(|a| a.status == "pending").count();
    let output_summary = format!(
        "status_filter={status_filter} returned={} pending_total={pending_total}",
        filtered.len(),
    );
    let next_action_hint: String = if filtered.is_empty() {
        "no assignments match this filter; check if all work has been completed or if a different status filter is needed".into()
    } else {
        "inspect returned assignments and call assignment.resume with a specific assignment_id to trigger execution".into()
    };
    let items: Vec<_> = filtered
        .iter()
        .map(|a| {
            json!({
                "assignment_id": a.assignment_id,
                "peer_id": a.peer_id,
                "project_id": a.project_id,
                "session_id": a.session_id,
                "task_id": a.task_id,
                "target_worker_id": a.target_worker_id,
                "target_agent_name": a.target_agent_name,
                "task_summary": a.task_summary,
                "status": a.status,
                "created_at": a.created_at,
                "updated_at": a.updated_at,
                "result_summary": a.result_summary,
            })
        })
        .collect();
    let mut artifact_refs = vec!["assignments".into()];
    let receipt = json!({
        "tool_call_id": tool_call_id,
        "tool_name": "assignment.list",
        "status": "completed",
        "status_filter": status_filter,
        "limit": limit,
        "count": filtered.len(),
        "pending_total": pending_total,
        "items": items,
        "output_summary": output_summary,
        "next_action_hint": next_action_hint,
    });
    if let Ok(Some(receipt_ref)) = persist_authoritative_tool_receipt(input, tool_call_id, &receipt)
    {
        artifact_refs.push(receipt_ref);
    }
    outcome.tool_records.push(ToolExecutionRecord {
        tool_call_id: tool_call_id.into(),
        operation_id: input.operation_id.into(),
        trace_id: input.trace_id.into(),
        refs: input.refs.clone(),
        tool_name: "assignment.list".into(),
        tool_kind: "agent_tool".into(),
        title: "Assignment List".into(),
        purpose: "list pending and recent assignments".into(),
        target_kind: Some("assignment_queue".into()),
        target_ref: Some("runtime/assignments/pending.json".into()),
        input_summary: Some(format!("status_filter={status_filter} limit={limit}")),
        output_summary: Some(output_summary.clone()),
        status: "completed".into(),
        started_at: input.occurred_at.into(),
        ended_at: Some(input.occurred_at.into()),
        duration_ms: Some(0),
        side_effects: vec!["read_assignment_queue".into()],
        artifact_refs,
        error_summary: None,
    });
    outcome.events.push((
        "assignment.list_completed".into(),
        json!({
            "tool_call_id": tool_call_id,
            "count": filtered.len(),
            "pending_total": pending_total,
            "status_filter": status_filter,
        }),
    ));
    outcome.note_hints.push(format!(
        "assignment.list returned {} item(s) (filter={status_filter}, pending_total={pending_total})",
        filtered.len(),
    ));
    true
}

pub(super) fn handle_assignment_resume(
    outcome: &mut ToolDispatchOutcome,
    input: &ToolDispatchInput<'_>,
    tool_call_id: &str,
    arguments: &Value,
) -> bool {
    let Some(runtime_home) = runtime_home_from_context(input.context) else {
        outcome.tool_records.push(failed_record(
            input,
            tool_call_id.into(),
            "assignment.resume",
            "missing context.project.runtime_home, cannot resume assignment",
        ));
        return true;
    };
    let Some(assignment_id) = read_string(arguments, "assignment_id") else {
        outcome.tool_records.push(failed_record(
            input,
            tool_call_id.into(),
            "assignment.resume",
            "missing required argument: assignment_id",
        ));
        return true;
    };
    let assignments = match read_assignment_queue(&runtime_home) {
        Ok(items) => items,
        Err(err) => {
            outcome.tool_records.push(failed_record(
                input,
                tool_call_id.into(),
                "assignment.resume",
                format!("failed to read assignment queue: {err}").as_str(),
            ));
            return true;
        }
    };
    let target = assignments.iter().find(|a| a.assignment_id == assignment_id);
    let Some(target) = target else {
        outcome.tool_records.push(failed_record(
            input,
            tool_call_id.into(),
            "assignment.resume",
            format!("assignment not found: {assignment_id}").as_str(),
        ));
        return true;
    };
    if target.status != "pending" && target.status != "resume_requested" {
        outcome.tool_records.push(failed_record(
            input,
            tool_call_id.into(),
            "assignment.resume",
            format!(
                "assignment {} has status '{}', cannot resume (must be pending or resume_requested)",
                assignment_id, target.status
            )
            .as_str(),
        ));
        return true;
    }
    let now = input.occurred_at;
    if let Err(err) = update_assignment_record(
        &runtime_home,
        &assignment_id,
        |record| {
            record.status = "resume_requested".into();
            record.updated_at = Some(now.to_string());
            record.result_summary = Some(
                "resume requested by system agent via assignment.resume tool".into(),
            );
        },
    ) {
        outcome.tool_records.push(failed_record(
            input,
            tool_call_id.into(),
            "assignment.resume",
            format!("failed to mark assignment as resume_requested: {err}").as_str(),
        ));
        return true;
    }
    let output_summary = format!(
        "assignment_id={} marked resume_requested; daemon will drive worker session on next cycle",
        assignment_id,
    );
    let mut artifact_refs = vec!["assignments".into()];
    let receipt = json!({
        "tool_call_id": tool_call_id,
        "tool_name": "assignment.resume",
        "status": "resume_requested",
        "assignment_id": assignment_id,
        "peer_id": target.peer_id,
        "worker_id": target.target_worker_id,
        "task_summary": short_text(&target.task_summary, 120),
        "output_summary": output_summary,
        "next_action_hint": "daemon will observe resume_requested status and drive the bound worker session on the next cycle",
    });
    if let Ok(Some(receipt_ref)) = persist_authoritative_tool_receipt(input, tool_call_id, &receipt)
    {
        artifact_refs.push(receipt_ref);
    }
    outcome.tool_records.push(ToolExecutionRecord {
        tool_call_id: tool_call_id.into(),
        operation_id: input.operation_id.into(),
        trace_id: input.trace_id.into(),
        refs: input.refs.clone(),
        tool_name: "assignment.resume".into(),
        tool_kind: "agent_tool".into(),
        title: "Assignment Resume".into(),
        purpose: "mark a pending assignment for resume so daemon drives the worker".into(),
        target_kind: Some("assignment_queue".into()),
        target_ref: Some(format!("runtime/assignments/pending.json#{}", assignment_id)),
        input_summary: Some(format!("assignment_id={}", assignment_id)),
        output_summary: Some(output_summary.clone()),
        status: "resume_requested".into(),
        started_at: input.occurred_at.into(),
        ended_at: Some(input.occurred_at.into()),
        duration_ms: Some(0),
        side_effects: vec!["update_assignment_status_resume_requested".into()],
        artifact_refs,
        error_summary: None,
    });
    outcome.events.push((
        "assignment.resume_requested".into(),
        json!({
            "tool_call_id": tool_call_id,
            "assignment_id": assignment_id,
            "peer_id": target.peer_id,
            "worker_id": target.target_worker_id,
        }),
    ));
    outcome.note_hints.push(format!(
        "assignment.resume marked {} as resume_requested; daemon will drive worker on next cycle",
        short_text(&assignment_id, 60),
    ));
    true
}
