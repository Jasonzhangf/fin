use super::retry::ContractRetrySummary;
use crate::tools::tool_dispatch::ToolDispatchOutcome;
use fin_contracts::ControlFeedback;

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

pub(super) fn stop_source(
    waiting_external: bool,
    stopped: bool,
    feedback: &ControlFeedback,
) -> &'static str {
    if waiting_external {
        "wait.remind"
    } else if stopped {
        exit_channel_label(feedback)
    } else {
        "not_emitted"
    }
}

pub(super) fn operation_status(
    waiting_external: bool,
    stopped: bool,
    feedback: &ControlFeedback,
) -> &'static str {
    if waiting_external {
        "waiting_external"
    } else if stopped {
        exit_channel_label(feedback)
    } else {
        "continued"
    }
}

fn exit_channel_label(feedback: &ControlFeedback) -> &'static str {
    if feedback.task_completed
        && !feedback.completion_evidence.is_empty()
        && !feedback.final_conclusions.is_empty()
    {
        "completed_with_evidence"
    } else if feedback.is_simple_chat {
        "simple_chat_done"
    } else if feedback.blocked
        && feedback.needs_user_involve
        && feedback
            .blocked_reason
            .as_ref()
            .map_or(false, |s| !s.trim().is_empty())
        && feedback
            .what_needs_to_be_done_by_user
            .as_ref()
            .map_or(false, |s| !s.trim().is_empty())
    {
        "blocked_requires_user_action"
    } else {
        "stopped"
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

pub(super) fn record_auto_tool_round_limit(
    dispatched_tools: &mut ToolDispatchOutcome,
    parsed_output: &crate::ParsedModelOutput,
    round_count: usize,
    max_auto_tool_rounds: usize,
) {
    if dispatched_tools.stop_requested
        || parsed_output.tool_calls.is_empty()
        || round_count < max_auto_tool_rounds
    {
        return;
    }
    dispatched_tools.note_hints.push(format!(
        "auto tool loop stopped at round limit ({max_auto_tool_rounds})"
    ));
    dispatched_tools.events.push((
        "reasoning.auto_tool_roundtrip_limit_reached".into(),
        serde_json::json!({
            "round_count": round_count,
            "max_rounds": max_auto_tool_rounds,
            "remaining_tool_calls": parsed_output.tool_calls.len(),
        }),
    ));
}

pub(super) fn record_output_contract_retry_limit(
    dispatched_tools: &mut ToolDispatchOutcome,
    summaries: &[ContractRetrySummary],
    max_retries: usize,
) {
    let total_retries = summaries.iter().map(|item| item.retry_count).sum::<usize>();
    if total_retries == 0 {
        return;
    }
    let limit_hits = summaries.iter().filter(|item| item.limit_reached).count();
    if limit_hits == 0 {
        return;
    }
    dispatched_tools.note_hints.push(format!(
        "output contract retry hit limit on {} round(s) with max {} retry attempt(s)",
        limit_hits, max_retries
    ));
}
