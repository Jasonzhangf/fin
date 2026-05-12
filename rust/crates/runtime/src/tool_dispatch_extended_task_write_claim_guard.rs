use crate::{task_store::StoredTaskRecord, tool_dispatch::ToolDispatchInput};

pub(super) fn reject_owner_self_claim_when_dispatchable(
    task: &StoredTaskRecord,
    input: &ToolDispatchInput<'_>,
    worker_id: &str,
) -> Result<(), String> {
    if task.status != "ready" || task.claimed_by_worker_id.is_some() {
        return Ok(());
    }
    if task.review_owner_worker_id.as_deref() != Some(worker_id) {
        return Ok(());
    }
    let Some(project) = input.context.project.as_ref() else {
        return Ok(());
    };
    let Some(dispatchable_slots) = parse_dispatchable_slots(project) else {
        return Ok(());
    };
    if dispatchable_slots == 0 {
        return Ok(());
    }
    Err(format!(
        "owner self-claim rejected: visible idle worker capacity exists ({dispatchable_slots}); dispatch this ready task with agent.assign and let the target worker claim it"
    ))
}

fn parse_dispatchable_slots(project: &fin_contracts::ProjectContextBlock) -> Option<usize> {
    let visible_idle_agents =
        count_presence_status(project.agent_presence_summary.as_deref()?, "idle");
    let pending_assignments = project
        .assignment_queue_summary
        .as_deref()
        .and_then(parse_pending_assignments)
        .unwrap_or(0);
    Some(visible_idle_agents.saturating_sub(pending_assignments))
}

fn count_presence_status(summary: &str, status: &str) -> usize {
    summary.match_indices(format!(":{status}").as_str()).count()
}

fn parse_pending_assignments(summary: &str) -> Option<usize> {
    let prefix = "pending_assignments=";
    let start = summary.find(prefix)? + prefix.len();
    let digits = summary[start..]
        .chars()
        .take_while(|char| char.is_ascii_digit())
        .collect::<String>();
    if digits.is_empty() {
        None
    } else {
        digits.parse::<usize>().ok()
    }
}
