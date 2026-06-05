use crate::ClosureRun;
use fin_contracts::{
    EntityRefs, InterruptedSegmentRecord, PauseCheckpointRecord, SegmentMergeRecord,
};

pub fn interrupted_segment(
    refs: &EntityRefs,
    session_id: Option<&str>,
    checkpoint: &PauseCheckpointRecord,
) -> InterruptedSegmentRecord {
    InterruptedSegmentRecord {
        segment_id: build_segment_id(session_id, checkpoint),
        refs: refs.clone(),
        interrupted_turn_id: checkpoint.turn_id.clone(),
        interrupted_step_id: checkpoint.active_step_id.clone(),
        resume_from_step_id: checkpoint.resume_from_step_id.clone(),
        status: "open".into(),
        reason: checkpoint.reason.clone(),
        created_at: checkpoint.paused_at.clone(),
        merged_into_turn_id: None,
        merged_into_operation_id: None,
        merged_at: None,
    }
}

pub fn segment_merge(segment: &InterruptedSegmentRecord, run: &ClosureRun) -> SegmentMergeRecord {
    SegmentMergeRecord {
        merge_id: format!("merge-{}", run.operation.operation_id),
        segment_id: segment.segment_id.clone(),
        refs: run.operation.refs.clone(),
        interrupted_turn_id: segment.interrupted_turn_id.clone(),
        resumed_turn_id: run.turn_record.turn_id.clone(),
        resumed_operation_id: run.operation.operation_id.clone(),
        strategy: "resume_as_new_closure".into(),
        created_at: run.note.created_at.clone(),
    }
}

pub fn apply_segment_merge(
    segments: &mut [InterruptedSegmentRecord],
    segment: &InterruptedSegmentRecord,
    resumed_turn_id: &str,
    resumed_operation_id: &str,
    merged_at: &str,
) -> Option<InterruptedSegmentRecord> {
    let target_index = segments.iter().rposition(|item| {
        item.segment_id == segment.segment_id
            && item.created_at == segment.created_at
            && item.interrupted_turn_id == segment.interrupted_turn_id
            && item.interrupted_step_id == segment.interrupted_step_id
            && item.status == "open"
    })?;
    let item = &mut segments[target_index];
    item.status = "merged".into();
    item.merged_into_turn_id = Some(resumed_turn_id.into());
    item.merged_into_operation_id = Some(resumed_operation_id.into());
    item.merged_at = Some(merged_at.into());
    Some(item.clone())
}

fn build_segment_id(session_id: Option<&str>, checkpoint: &PauseCheckpointRecord) -> String {
    let session = sanitize_id_fragment(session_id.unwrap_or("tentative"));
    let turn = sanitize_id_fragment(checkpoint.turn_id.as_deref().unwrap_or("turn"));
    let paused_at = sanitize_id_fragment(&checkpoint.paused_at);
    format!("segment-{session}-{turn}-{paused_at}")
}

fn sanitize_id_fragment(value: &str) -> String {
    let sanitized = value
        .chars()
        .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '-' })
        .collect::<String>();
    sanitized
        .trim_matches('-')
        .split('-')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}
