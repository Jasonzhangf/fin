use crate::{
    tool_dispatch::{ToolDispatchInput, ToolDispatchOutcome},
    tool_dispatch_extended_collab_coordination, tool_dispatch_extended_collab_mailbox,
};
use serde_json::Value;

pub(super) fn handle_mailbox_send(
    outcome: &mut ToolDispatchOutcome,
    input: &ToolDispatchInput<'_>,
    tool_call_id: &str,
    arguments: &Value,
) -> bool {
    tool_dispatch_extended_collab_mailbox::handle_mailbox_send(
        outcome,
        input,
        tool_call_id,
        arguments,
    )
}

pub(super) fn handle_mailbox_poll(
    outcome: &mut ToolDispatchOutcome,
    input: &ToolDispatchInput<'_>,
    tool_call_id: &str,
    arguments: &Value,
) -> bool {
    tool_dispatch_extended_collab_mailbox::handle_mailbox_poll(
        outcome,
        input,
        tool_call_id,
        arguments,
    )
}

pub(super) fn handle_agent_assign(
    outcome: &mut ToolDispatchOutcome,
    input: &ToolDispatchInput<'_>,
    tool_call_id: &str,
    arguments: &Value,
) -> bool {
    tool_dispatch_extended_collab_coordination::handle_agent_assign(
        outcome,
        input,
        tool_call_id,
        arguments,
    )
}

pub(super) fn handle_capability_invoke(
    outcome: &mut ToolDispatchOutcome,
    input: &ToolDispatchInput<'_>,
    tool_call_id: &str,
    arguments: &Value,
) -> bool {
    tool_dispatch_extended_collab_coordination::handle_capability_invoke(
        outcome,
        input,
        tool_call_id,
        arguments,
    )
}
