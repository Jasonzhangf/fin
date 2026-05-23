use super::*;
use crate::source_visibility::is_hidden_session_source;
use closure_runtime_rounds::allocate_step;

pub(super) fn build_partial_run(
    operation: &OperationEnvelope<InferenceOperationPayload>,
    prepared_request: &PreparedRequest,
    provider_response: &ProviderResponse,
    assistant_response_text: &str,
    control_feedback: &ControlFeedback,
    context_snapshot: &ContextSnapshotRecord,
    tool_records: &[ToolExecutionRecord],
    provider_request_records: &[ProviderRequestRecord],
    provider_response_records: &[ProviderResponseRecord],
    round_records: &[RoundRecord],
    step_records: &[StepRecord],
    progress: &ProgressBlock,
    note: &ExecutionNote,
    reasoning_view: &ReasoningViewRecord,
    digest: &DigestRecord,
    turn_record: &TurnRecord,
    routing_decision: &RoutingDecisionRecord,
    routing_action: &RoutingActionRecord,
    resume_checkpoint: Option<&ExecutionCheckpointRecord>,
    compacted_history_records: &[CompactedHistoryRecord],
    events: &[EventEnvelope<Value>],
) -> ClosureRun {
    ClosureRun {
        operation: operation.clone(),
        prepared_request: prepared_request.clone(),
        provider_response: provider_response.clone(),
        assistant_response_text: assistant_response_text.to_string(),
        conversation_user_input: conversation_user_input_for_source(
            &operation.source,
            &operation.payload.input,
        ),
        control_feedback: control_feedback.clone(),
        context_snapshot: context_snapshot.clone(),
        tool_records: tool_records.to_vec(),
        provider_request_records: provider_request_records.to_vec(),
        provider_response_records: provider_response_records.to_vec(),
        round_records: round_records.to_vec(),
        step_records: step_records.to_vec(),
        progress: progress.clone(),
        note: note.clone(),
        reasoning_view: reasoning_view.clone(),
        digest: digest.clone(),
        turn_record: turn_record.clone(),
        routing_decision: routing_decision.clone(),
        routing_action: routing_action.clone(),
        resume_checkpoint: resume_checkpoint.cloned(),
        compacted_history_records: compacted_history_records.to_vec(),
        closure_trace: ClosureTraceRecord::default(),
        events: events.to_vec(),
    }
}

pub(super) fn append_checkpoint_recorded_event(
    runtime: &mut M1Runtime,
    events: &mut Vec<EventEnvelope<Value>>,
    operation: &OperationEnvelope<InferenceOperationPayload>,
    refs: &EntityRefs,
    checkpoint: Option<&ExecutionCheckpointRecord>,
) -> Result<(), RuntimeError> {
    let Some(checkpoint) = checkpoint else {
        return Ok(());
    };
    events.push(runtime.event(
        "execution.checkpoint_recorded",
        &operation.trace_id,
        &operation.submitted_at,
        refs,
        Some(operation.operation_id.clone()),
        serde_json::to_value(checkpoint)?,
    )?);
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub(super) fn append_finalize_step(
    step_index: &mut u32,
    operation: &OperationEnvelope<InferenceOperationPayload>,
    refs: &EntityRefs,
    turn_id: &str,
    digest_id: &str,
    stop_source: &str,
    assistant_response_text: &str,
    progress: &ProgressBlock,
    note: &ExecutionNote,
    step_records: &mut Vec<StepRecord>,
) {
    let finalize_step = allocate_step(step_index, &operation.operation_id, "finalize");
    step_records.push(turn_records::finalize_step_record(
        turn_records::step_record(
            finalize_step.step_id,
            turn_id,
            &operation.operation_id,
            &operation.trace_id,
            refs,
            finalize_step.step_index,
            "finalize",
            "completed",
            &operation.submitted_at,
            format!(
                "closure finalized with stop_source={} answer={assistant_response_text}",
                stop_source
            ),
            Some(format!("digests/latest.json#digest_id={digest_id}")),
            Some(format!("turns/recent_turns.json#turn_id={turn_id}")),
            Some("render_projection".into()),
        ),
        progress,
        note,
    ));
}

#[allow(clippy::too_many_arguments)]
pub(super) fn build_final_run(
    operation: OperationEnvelope<InferenceOperationPayload>,
    prepared_request: PreparedRequest,
    provider_response: ProviderResponse,
    assistant_response_text: String,
    control_feedback: ControlFeedback,
    context_snapshot: ContextSnapshotRecord,
    tool_records: Vec<ToolExecutionRecord>,
    provider_request_records: Vec<ProviderRequestRecord>,
    provider_response_records: Vec<ProviderResponseRecord>,
    round_records: Vec<RoundRecord>,
    step_records: Vec<StepRecord>,
    progress: ProgressBlock,
    note: ExecutionNote,
    reasoning_view: ReasoningViewRecord,
    digest: DigestRecord,
    turn_record: TurnRecord,
    routing_decision: RoutingDecisionRecord,
    routing_action: RoutingActionRecord,
    resume_checkpoint: Option<ExecutionCheckpointRecord>,
    compacted_history_records: Vec<CompactedHistoryRecord>,
    closure_trace: ClosureTraceRecord,
    events: Vec<EventEnvelope<Value>>,
) -> ClosureRun {
    ClosureRun {
        conversation_user_input: conversation_user_input_for_source(
            &operation.source,
            &context_snapshot.input,
        ),
        operation,
        prepared_request,
        provider_response,
        assistant_response_text,
        control_feedback,
        context_snapshot,
        tool_records,
        provider_request_records,
        provider_response_records,
        round_records,
        step_records,
        progress,
        note,
        reasoning_view,
        digest,
        turn_record,
        routing_decision,
        routing_action,
        resume_checkpoint,
        compacted_history_records,
        closure_trace,
        events,
    }
}

fn conversation_user_input_for_source(source: &str, input: &str) -> Option<String> {
    if is_hidden_session_source(source) {
        None
    } else {
        Some(input.to_string())
    }
}
