use super::tool_dispatch::{ToolDispatchInput, ToolDispatchOutcome};
use serde_json::Value;

pub(super) fn handle_apply_patch(
    outcome: &mut ToolDispatchOutcome,
    input: &ToolDispatchInput<'_>,
    tool_call_id: &str,
    arguments: &Value,
) -> bool {
    super::tool_dispatch_extended_patch::handle_apply_patch(outcome, input, tool_call_id, arguments)
}

pub(super) fn handle_exec_command(
    outcome: &mut ToolDispatchOutcome,
    input: &ToolDispatchInput<'_>,
    tool_call_id: &str,
    arguments: &Value,
) -> bool {
    super::tool_dispatch_extended_exec::handle_exec_command(outcome, input, tool_call_id, arguments)
}

pub(super) fn handle_view_image(
    outcome: &mut ToolDispatchOutcome,
    input: &ToolDispatchInput<'_>,
    tool_call_id: &str,
    arguments: &Value,
) -> bool {
    super::tool_dispatch_extended_query::handle_view_image(outcome, input, tool_call_id, arguments)
}

pub(super) fn handle_context_history_rebuild(
    outcome: &mut ToolDispatchOutcome,
    input: &ToolDispatchInput<'_>,
    tool_call_id: &str,
    arguments: &Value,
) -> bool {
    super::tool_dispatch_extended_query::handle_context_history_rebuild(
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
    super::tool_dispatch_extended_query::handle_project_task_status(
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
    super::tool_dispatch_extended_query::handle_project_task_list(outcome, input, tool_call_id, arguments)
}

pub(super) fn handle_project_task_create(
    outcome: &mut ToolDispatchOutcome,
    input: &ToolDispatchInput<'_>,
    tool_call_id: &str,
    arguments: &Value,
) -> bool {
    super::tool_dispatch_extended_task_write::handle_project_task_create(
        outcome,
        input,
        tool_call_id,
        arguments,
    )
}

pub(super) fn handle_project_task_claim(
    outcome: &mut ToolDispatchOutcome,
    input: &ToolDispatchInput<'_>,
    tool_call_id: &str,
    arguments: &Value,
) -> bool {
    super::tool_dispatch_extended_task_write::handle_project_task_claim(
        outcome,
        input,
        tool_call_id,
        arguments,
    )
}

pub(super) fn handle_project_task_submit(
    outcome: &mut ToolDispatchOutcome,
    input: &ToolDispatchInput<'_>,
    tool_call_id: &str,
    arguments: &Value,
) -> bool {
    super::tool_dispatch_extended_task_write::handle_project_task_submit(
        outcome,
        input,
        tool_call_id,
        arguments,
    )
}

pub(super) fn handle_project_task_review(
    outcome: &mut ToolDispatchOutcome,
    input: &ToolDispatchInput<'_>,
    tool_call_id: &str,
    arguments: &Value,
) -> bool {
    super::tool_dispatch_extended_task_write::handle_project_task_review(
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
    super::tool_dispatch_extended_query::handle_agent_presence_list(
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
    super::tool_dispatch_extended_query::handle_project_supervision_list(
        outcome,
        input,
        tool_call_id,
        arguments,
    )
}

pub(super) fn handle_write_stdin(
    outcome: &mut ToolDispatchOutcome,
    input: &ToolDispatchInput<'_>,
    tool_call_id: &str,
    arguments: &Value,
) -> bool {
    super::tool_dispatch_extended_exec::handle_write_stdin(outcome, input, tool_call_id, arguments)
}

pub(super) fn handle_mailbox_send(
    outcome: &mut ToolDispatchOutcome,
    input: &ToolDispatchInput<'_>,
    tool_call_id: &str,
    arguments: &Value,
) -> bool {
    super::tool_dispatch_extended_collab::handle_mailbox_send(outcome, input, tool_call_id, arguments)
}

pub(super) fn handle_mailbox_poll(
    outcome: &mut ToolDispatchOutcome,
    input: &ToolDispatchInput<'_>,
    tool_call_id: &str,
    arguments: &Value,
) -> bool {
    super::tool_dispatch_extended_collab::handle_mailbox_poll(outcome, input, tool_call_id, arguments)
}

pub(super) fn handle_agent_assign(
    outcome: &mut ToolDispatchOutcome,
    input: &ToolDispatchInput<'_>,
    tool_call_id: &str,
    arguments: &Value,
) -> bool {
    super::tool_dispatch_extended_collab::handle_agent_assign(outcome, input, tool_call_id, arguments)
}

pub(super) fn handle_capability_invoke(
    outcome: &mut ToolDispatchOutcome,
    input: &ToolDispatchInput<'_>,
    tool_call_id: &str,
    arguments: &Value,
) -> bool {
    super::tool_dispatch_extended_collab::handle_capability_invoke(outcome, input, tool_call_id, arguments)
}
