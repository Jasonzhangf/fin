use super::tool_dispatch::{ToolDispatchInput, ToolDispatchOutcome};
use super::tool_dispatch as tool_dispatch;
use super::tool_dispatch_extended_query_control;
use super::tool_dispatch_extended_query_history;
use super::tool_dispatch_extended_query_image;
use super::tool_dispatch_extended_query_task;
use serde_json::Value;

pub(super) fn handle_view_image(
    outcome: &mut ToolDispatchOutcome,
    input: &ToolDispatchInput<'_>,
    tool_call_id: &str,
    arguments: &Value,
) -> bool {
    tool_dispatch_extended_query_image::handle_view_image(outcome, input, tool_call_id, arguments)
}

pub(super) fn handle_context_history_rebuild(
    outcome: &mut ToolDispatchOutcome,
    input: &ToolDispatchInput<'_>,
    tool_call_id: &str,
    arguments: &Value,
) -> bool {
    tool_dispatch_extended_query_history::handle_context_history_rebuild(
        outcome,
        input,
        tool_call_id,
        arguments,
    )
}

pub(super) fn handle_project_task_status(
    outcome: &mut ToolDispatchOutcome,
    input: &ToolDispatchInput<'_>,
    tool_call_id: &str,
    arguments: &Value,
) -> bool {
    tool_dispatch_extended_query_task::handle_project_task_status(
        outcome,
        input,
        tool_call_id,
        arguments,
    )
}

pub(super) fn handle_project_task_list(
    outcome: &mut ToolDispatchOutcome,
    input: &ToolDispatchInput<'_>,
    tool_call_id: &str,
    arguments: &Value,
) -> bool {
    tool_dispatch_extended_query_task::handle_project_task_list(
        outcome,
        input,
        tool_call_id,
        arguments,
    )
}

pub(super) fn handle_agent_presence_list(
    outcome: &mut ToolDispatchOutcome,
    input: &ToolDispatchInput<'_>,
    tool_call_id: &str,
    arguments: &Value,
) -> bool {
    tool_dispatch_extended_query_control::handle_agent_presence_list(
        outcome,
        input,
        tool_call_id,
        arguments,
    )
}

pub(super) fn handle_project_supervision_list(
    outcome: &mut ToolDispatchOutcome,
    input: &ToolDispatchInput<'_>,
    tool_call_id: &str,
    arguments: &Value,
) -> bool {
    tool_dispatch_extended_query_control::handle_project_supervision_list(
        outcome,
        input,
        tool_call_id,
        arguments,
    )
}
