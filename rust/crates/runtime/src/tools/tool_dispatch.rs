use crate::{
    model::parser::ModelToolCall,
};
use super::{tool_dispatch_control, tool_dispatch_extended, tool_dispatch_peer};
use fin_contracts::{EntityRefs, MinimalContextView, ToolExecutionRecord};
use serde_json::Value;
use std::path::PathBuf;

#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct ToolDispatchOutcome {
    pub(crate) tool_records: Vec<ToolExecutionRecord>,
    pub(crate) events: Vec<(String, Value)>,
    pub(crate) note_hints: Vec<String>,
    pub(crate) reminder_scheduled: bool,
    pub(crate) stop_requested: bool,
    pub(crate) yield_requested: bool,
}

#[derive(Debug, Clone)]
pub(crate) struct ToolDispatchInput<'a> {
    pub(crate) operation_id: &'a str,
    pub(crate) trace_id: &'a str,
    pub(crate) refs: &'a EntityRefs,
    pub(crate) occurred_at: &'a str,
    pub(crate) context: &'a MinimalContextView,
}

pub(crate) fn execute_model_tools(
    operation_id: &str,
    trace_id: &str,
    refs: &EntityRefs,
    occurred_at: &str,
    context: &MinimalContextView,
    round_index: u32,
    tool_calls: &[ModelToolCall],
) -> ToolDispatchOutcome {
    let mut outcome = ToolDispatchOutcome::default();
    let dispatch_input = ToolDispatchInput {
        operation_id,
        trace_id,
        refs,
        occurred_at,
        context,
    };
    for (index, call) in tool_calls.iter().enumerate() {
        let tool_call_id = format!("tool-model-{operation_id}-r{round_index:02}-{index:02}");
        let handled = match call.tool_name.as_str() {
            "update_plan" => tool_dispatch_control::handle_update_plan(
                &mut outcome,
                &dispatch_input,
                &tool_call_id,
                &call.arguments,
            ),
            "session.list" => tool_dispatch_control::handle_session_list(
                &mut outcome,
                &dispatch_input,
                &tool_call_id,
                &call.arguments,
            ),
            "peer.list" => tool_dispatch_peer::handle_peer_list(
                &mut outcome,
                &dispatch_input,
                &tool_call_id,
                &call.arguments,
            ),
            "peer.describe" => tool_dispatch_peer::handle_peer_describe(
                &mut outcome,
                &dispatch_input,
                &tool_call_id,
                &call.arguments,
            ),
            "daemon.ensure_peer" => tool_dispatch_peer::handle_daemon_ensure_peer(
                &mut outcome,
                &dispatch_input,
                &tool_call_id,
                &call.arguments,
            ),
            "wait.remind" => tool_dispatch_control::handle_wait_remind(
                &mut outcome,
                &dispatch_input,
                &tool_call_id,
                &call.arguments,
            ),
            "reasoning.stop" => tool_dispatch_control::handle_reasoning_stop(
                &mut outcome,
                &dispatch_input,
                &tool_call_id,
                &call.arguments,
            ),
            "apply_patch" => tool_dispatch_extended::handle_apply_patch(
                &mut outcome,
                &dispatch_input,
                &tool_call_id,
                &call.arguments,
            ),
            "view_image" => tool_dispatch_extended::handle_view_image(
                &mut outcome,
                &dispatch_input,
                &tool_call_id,
                &call.arguments,
            ),
            "context_history.rebuild" => tool_dispatch_extended::handle_context_history_rebuild(
                &mut outcome,
                &dispatch_input,
                &tool_call_id,
                &call.arguments,
            ),
            "project.task.status" => tool_dispatch_extended::handle_project_task_status(
                &mut outcome,
                &dispatch_input,
                &tool_call_id,
                &call.arguments,
            ),
            "project.task.list" => tool_dispatch_extended::handle_project_task_list(
                &mut outcome,
                &dispatch_input,
                &tool_call_id,
                &call.arguments,
            ),
            "project.task.create" => tool_dispatch_extended::handle_project_task_create(
                &mut outcome,
                &dispatch_input,
                &tool_call_id,
                &call.arguments,
            ),
            "project.task.claim" => tool_dispatch_extended::handle_project_task_claim(
                &mut outcome,
                &dispatch_input,
                &tool_call_id,
                &call.arguments,
            ),
            "project.task.submit" => tool_dispatch_extended::handle_project_task_submit(
                &mut outcome,
                &dispatch_input,
                &tool_call_id,
                &call.arguments,
            ),
            "project.task.review" => tool_dispatch_extended::handle_project_task_review(
                &mut outcome,
                &dispatch_input,
                &tool_call_id,
                &call.arguments,
            ),
            "agent.presence.list" => tool_dispatch_extended::handle_agent_presence_list(
                &mut outcome,
                &dispatch_input,
                &tool_call_id,
                &call.arguments,
            ),
            "project.supervision.list" => tool_dispatch_extended::handle_project_supervision_list(
                &mut outcome,
                &dispatch_input,
                &tool_call_id,
                &call.arguments,
            ),
            "exec_command" => tool_dispatch_extended::handle_exec_command(
                &mut outcome,
                &dispatch_input,
                &tool_call_id,
                &call.arguments,
            ),
            "write_stdin" => tool_dispatch_extended::handle_write_stdin(
                &mut outcome,
                &dispatch_input,
                &tool_call_id,
                &call.arguments,
            ),
            "mailbox.send" => tool_dispatch_extended::handle_mailbox_send(
                &mut outcome,
                &dispatch_input,
                &tool_call_id,
                &call.arguments,
            ),
            "mailbox.poll" => tool_dispatch_extended::handle_mailbox_poll(
                &mut outcome,
                &dispatch_input,
                &tool_call_id,
                &call.arguments,
            ),
            "agent.assign" => tool_dispatch_extended::handle_agent_assign(
                &mut outcome,
                &dispatch_input,
                &tool_call_id,
                &call.arguments,
            ),
            "capability.invoke" => tool_dispatch_extended::handle_capability_invoke(
                &mut outcome,
                &dispatch_input,
                &tool_call_id,
                &call.arguments,
            ),
            _ => false,
        };
        if !handled {
            outcome.tool_records.push(failed_record(
                &dispatch_input,
                tool_call_id,
                call.tool_name.as_str(),
                "tool is not registered in current runtime tool dispatcher",
            ));
        }
    }
    outcome
}

pub(crate) fn runtime_home_from_context(context: &MinimalContextView) -> Option<PathBuf> {
    context
        .project
        .as_ref()
        .and_then(|project| project.runtime_home.as_deref())
        .filter(|value| !value.trim().is_empty())
        .map(PathBuf::from)
}

pub(crate) fn read_string(arguments: &Value, key: &str) -> Option<String> {
    let object = arguments.as_object()?;
    object.get(key).and_then(|value| match value {
        Value::String(raw) => {
            let trimmed = raw.trim();
            (!trimmed.is_empty()).then(|| trimmed.to_string())
        }
        Value::Number(number) => Some(number.to_string()),
        Value::Bool(boolean) => Some(boolean.to_string()),
        _ => None,
    })
}

pub(crate) fn read_u64(arguments: &Value, key: &str) -> Option<u64> {
    let object = arguments.as_object()?;
    let value = object.get(key)?;
    match value {
        Value::Number(number) => number.as_u64(),
        Value::String(raw) => raw.trim().parse::<u64>().ok(),
        _ => None,
    }
}

pub(crate) fn read_bool(arguments: &Value, key: &str) -> Option<bool> {
    let object = arguments.as_object()?;
    match object.get(key)? {
        Value::Bool(value) => Some(*value),
        Value::String(value) => match value.trim().to_ascii_lowercase().as_str() {
            "true" | "1" | "yes" => Some(true),
            "false" | "0" | "no" => Some(false),
            _ => None,
        },
        _ => None,
    }
}

pub(crate) fn failed_record(
    input: &ToolDispatchInput<'_>,
    tool_call_id: String,
    tool_name: &str,
    reason: &str,
) -> ToolExecutionRecord {
    ToolExecutionRecord {
        tool_call_id,
        operation_id: input.operation_id.into(),
        trace_id: input.trace_id.into(),
        refs: input.refs.clone(),
        tool_name: tool_name.into(),
        tool_kind: "agent_tool".into(),
        title: "Tool Dispatch Failed".into(),
        purpose: "record deterministic tool dispatch failure for debugging and retry".into(),
        target_kind: None,
        target_ref: None,
        input_summary: None,
        output_summary: None,
        status: "failed".into(),
        started_at: input.occurred_at.into(),
        ended_at: Some(input.occurred_at.into()),
        duration_ms: Some(0),
        side_effects: Vec::new(),
        artifact_refs: vec!["events/stream.jsonl".into()],
        error_summary: Some(reason.into()),
    }
}

pub(crate) fn short_text(value: &str, limit: usize) -> String {
    let trimmed = value.trim();
    if trimmed.chars().count() <= limit {
        trimmed.into()
    } else {
        let mut out = trimmed.chars().take(limit).collect::<String>();
        out.push('…');
        out
    }
}

pub fn authoritative_receipt_ref(artifact_ref: &str) -> bool {
    artifact_ref.ends_with(".receipt.md") || artifact_ref.ends_with(".receipt.json") || artifact_ref.contains("/receipts/")
}
