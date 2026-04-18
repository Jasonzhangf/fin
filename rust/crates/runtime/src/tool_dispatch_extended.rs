use crate::{
    tool_dispatch::{ToolDispatchInput, ToolDispatchOutcome},
    tool_dispatch_extended_collab, tool_dispatch_extended_exec,
};
use serde_json::Value;

pub(super) fn handle_exec_command(
    outcome: &mut ToolDispatchOutcome,
    input: &ToolDispatchInput<'_>,
    tool_call_id: &str,
    arguments: &Value,
) -> bool {
    tool_dispatch_extended_exec::handle_exec_command(outcome, input, tool_call_id, arguments)
}

pub(super) fn handle_write_stdin(
    outcome: &mut ToolDispatchOutcome,
    input: &ToolDispatchInput<'_>,
    tool_call_id: &str,
    arguments: &Value,
) -> bool {
    tool_dispatch_extended_exec::handle_write_stdin(outcome, input, tool_call_id, arguments)
}

pub(super) fn handle_mailbox_send(
    outcome: &mut ToolDispatchOutcome,
    input: &ToolDispatchInput<'_>,
    tool_call_id: &str,
    arguments: &Value,
) -> bool {
    tool_dispatch_extended_collab::handle_mailbox_send(outcome, input, tool_call_id, arguments)
}

pub(super) fn handle_mailbox_poll(
    outcome: &mut ToolDispatchOutcome,
    input: &ToolDispatchInput<'_>,
    tool_call_id: &str,
    arguments: &Value,
) -> bool {
    tool_dispatch_extended_collab::handle_mailbox_poll(outcome, input, tool_call_id, arguments)
}

pub(super) fn handle_agent_assign(
    outcome: &mut ToolDispatchOutcome,
    input: &ToolDispatchInput<'_>,
    tool_call_id: &str,
    arguments: &Value,
) -> bool {
    tool_dispatch_extended_collab::handle_agent_assign(outcome, input, tool_call_id, arguments)
}

pub(super) fn handle_capability_invoke(
    outcome: &mut ToolDispatchOutcome,
    input: &ToolDispatchInput<'_>,
    tool_call_id: &str,
    arguments: &Value,
) -> bool {
    tool_dispatch_extended_collab::handle_capability_invoke(outcome, input, tool_call_id, arguments)
}
