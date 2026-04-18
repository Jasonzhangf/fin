use crate::tool_dispatch::{
    ToolDispatchInput, ToolDispatchOutcome, failed_record, read_string, read_u64,
};
use fin_contracts::ToolExecutionRecord;
use serde_json::{Value, json};

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
        output_summary: Some("reasoning stop accepted".into()),
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
    outcome.note_hints.push("reasoning.stop accepted".into());
    outcome.stop_requested = true;
    true
}
