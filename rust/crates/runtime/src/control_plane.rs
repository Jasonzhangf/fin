use crate::ClosureRun;
use fin_contracts::{
    EntityRefs, ExecutionStateRecord, InputAttachmentSummary, PauseCheckpointRecord,
    PendingInputRecord,
};

#[path = "control_plane_segments.rs"]
mod control_plane_segments;
pub use control_plane_segments::{apply_segment_merge, interrupted_segment, segment_merge};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingInputDequeue {
    pub dequeued: PendingInputRecord,
    pub remaining: Vec<PendingInputRecord>,
}

pub fn running_state(
    refs: &EntityRefs,
    operation_id: &str,
    submitted_at: &str,
    pending_input_count: usize,
) -> ExecutionStateRecord {
    ExecutionStateRecord {
        state_id: format!("exec-state-{operation_id}"),
        refs: refs.clone(),
        status: "running".into(),
        active_turn_id: Some(format!("turn-{operation_id}")),
        active_step_id: Some(format!("step-{operation_id}-01-context_build")),
        pending_input_count,
        accepts_user_input: false,
        reason: Some("active closure running".into()),
        updated_at: submitted_at.into(),
    }
}

pub fn state_after_run(run: &ClosureRun, pending_input_count: usize) -> ExecutionStateRecord {
    let last_step_id = run.step_records.last().map(|step| step.step_id.clone());
    let is_waiting = run
        .tool_records
        .iter()
        .any(|record| record.tool_name == "wait.remind" && record.status == "completed");
    ExecutionStateRecord {
        state_id: format!("exec-state-{}", run.operation.operation_id),
        refs: run.operation.refs.clone(),
        status: if is_waiting {
            "waiting_external".into()
        } else {
            "idle".into()
        },
        active_turn_id: Some(run.turn_record.turn_id.clone()),
        active_step_id: last_step_id.clone(),
        pending_input_count,
        accepts_user_input: true,
        reason: Some(if is_waiting {
            "waiting for reminder or external result".into()
        } else {
            "closure completed".into()
        }),
        updated_at: run.note.created_at.clone(),
    }
}

pub fn failed_state(
    refs: &EntityRefs,
    operation_id: &str,
    now: &str,
    reason: &str,
    pending_input_count: usize,
) -> ExecutionStateRecord {
    ExecutionStateRecord {
        state_id: format!("exec-state-{operation_id}-failed"),
        refs: refs.clone(),
        status: "idle".into(),
        active_turn_id: Some(format!("turn-{operation_id}")),
        active_step_id: None,
        pending_input_count,
        accepts_user_input: true,
        reason: Some(reason.into()),
        updated_at: now.into(),
    }
}

pub fn paused_state(
    refs: &EntityRefs,
    session_id: Option<&str>,
    current_state: Option<&ExecutionStateRecord>,
    now: &str,
    reason: Option<String>,
    pending_input_count: usize,
) -> (ExecutionStateRecord, PauseCheckpointRecord) {
    let (turn_id, step_id) = current_state
        .map(|state| (state.active_turn_id.clone(), state.active_step_id.clone()))
        .unwrap_or((None, None));
    let checkpoint = PauseCheckpointRecord {
        checkpoint_id: format!("pause-{}", session_id.unwrap_or("tentative")),
        refs: refs.clone(),
        turn_id: turn_id.clone(),
        active_step_id: step_id.clone(),
        resume_from_step_id: step_id.clone(),
        resume_checkpoint_id: step_id.clone(),
        reason: reason.clone(),
        paused_at: now.into(),
    };
    let state = ExecutionStateRecord {
        state_id: checkpoint.checkpoint_id.clone(),
        refs: refs.clone(),
        status: "paused".into(),
        active_turn_id: turn_id,
        active_step_id: step_id.clone(),
        pending_input_count,
        accepts_user_input: false,
        reason,
        updated_at: now.into(),
    };
    (state, checkpoint)
}

pub fn resumed_state(
    refs: &EntityRefs,
    session_id: Option<&str>,
    checkpoint: Option<&PauseCheckpointRecord>,
    now: &str,
    pending_input_count: usize,
) -> ExecutionStateRecord {
    ExecutionStateRecord {
        state_id: format!("resume-{}", session_id.unwrap_or("tentative")),
        refs: refs.clone(),
        status: "idle".into(),
        active_turn_id: checkpoint.and_then(|value| value.turn_id.clone()),
        active_step_id: checkpoint.and_then(|value| value.active_step_id.clone()),
        pending_input_count,
        accepts_user_input: true,
        reason: Some("manual resume".into()),
        updated_at: now.into(),
    }
}

pub fn new_pending_input(
    refs: &EntityRefs,
    session_id: Option<&str>,
    next_index: usize,
    input_kind: &str,
    source: &str,
    message: &str,
    attachments: &[InputAttachmentSummary],
    enqueue_reason: &str,
    now: &str,
) -> PendingInputRecord {
    PendingInputRecord {
        pending_input_id: format!(
            "pending-{}-{next_index:02}",
            session_id.unwrap_or("tentative")
        ),
        refs: refs.clone(),
        input_kind: input_kind.into(),
        source: source.into(),
        message: message.trim().to_string(),
        attachments: attachments.to_vec(),
        status: "pending".into(),
        enqueue_reason: enqueue_reason.into(),
        enqueued_at: now.into(),
    }
}

pub fn dequeue_pending_input(pending: &[PendingInputRecord]) -> Option<PendingInputDequeue> {
    let (first, rest) = pending.split_first()?;
    let mut dequeued = first.clone();
    dequeued.status = "dequeued".into();
    Some(PendingInputDequeue {
        dequeued,
        remaining: rest.to_vec(),
    })
}

pub fn state_with_pending_count(
    state: &ExecutionStateRecord,
    pending_input_count: usize,
    now: &str,
) -> ExecutionStateRecord {
    let mut updated = state.clone();
    updated.pending_input_count = pending_input_count;
    updated.updated_at = now.into();
    updated
}

pub fn clear_waiting_state_if_due(
    state: &ExecutionStateRecord,
    now: &str,
) -> Option<ExecutionStateRecord> {
    if state.status != "waiting_external" {
        return None;
    }
    let mut updated = state.clone();
    updated.status = "idle".into();
    updated.accepts_user_input = true;
    updated.reason = Some("external reminder fired".into());
    updated.updated_at = now.into();
    Some(updated)
}

#[cfg(test)]
#[path = "control_plane_tests.rs"]
mod tests;
