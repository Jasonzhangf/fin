use crate::tool_dispatch::ToolDispatchOutcome;

pub(super) fn merge_dispatch_outcome(
    aggregate: &mut ToolDispatchOutcome,
    current_round: &ToolDispatchOutcome,
) {
    aggregate.events.extend(current_round.events.clone());
    aggregate
        .note_hints
        .extend(current_round.note_hints.clone());
    aggregate.reminder_scheduled |= current_round.reminder_scheduled;
    aggregate.stop_requested |= current_round.stop_requested;
    aggregate.yield_requested |= current_round.yield_requested;
}

pub(super) fn stop_source(waiting_external: bool, stopped: bool) -> &'static str {
    if waiting_external {
        "wait.remind"
    } else if stopped {
        "reasoning.stop"
    } else {
        "not_emitted"
    }
}

pub(super) fn operation_status(waiting_external: bool, stopped: bool) -> &'static str {
    if waiting_external {
        "waiting_external"
    } else if stopped {
        "stopped"
    } else {
        "continued"
    }
}

pub(super) fn next_step(reminder_scheduled: bool, closure_stopped: bool) -> &'static str {
    if reminder_scheduled {
        "wait_for_scheduled_reminder"
    } else if !closure_stopped {
        "continue_reasoning"
    } else {
        "render_projection"
    }
}
