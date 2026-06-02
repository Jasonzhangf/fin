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
    Some(
        project
            .idle_agent_count
            .saturating_sub(project.pending_assignment_count),
    )
}
