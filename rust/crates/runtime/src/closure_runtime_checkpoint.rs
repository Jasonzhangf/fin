use super::*;
use closure_runtime_rounds::build_followup_input;

pub(super) fn build_resume_checkpoint(
    operation: &OperationEnvelope<InferenceOperationPayload>,
    refs: &EntityRefs,
    turn_id: &str,
    last_round: Option<&RoundRecord>,
    parsed_output: &ParsedModelOutput,
    dispatched_tools: &tool_dispatch::ToolDispatchOutcome,
    assistant_response_text: &str,
    latest_round_tool_records: &[ToolExecutionRecord],
    created_at: &str,
) -> Option<ExecutionCheckpointRecord> {
    let last_round = last_round?;
    let checkpoint_kind = if dispatched_tools.yield_requested {
        "wait_reminder_resume"
    } else if !dispatched_tools.stop_requested && !parsed_output.tool_calls.is_empty() {
        "tool_followup_resume"
    } else {
        return None;
    };
    let next_round_index = last_round.round_index.saturating_add(1);
    let resume_input = build_followup_input(
        operation.payload.input.as_str(),
        assistant_response_text,
        latest_round_tool_records,
    );
    Some(ExecutionCheckpointRecord {
        checkpoint_id: format!(
            "checkpoint-{}-r{next_round_index:02}",
            operation.operation_id
        ),
        refs: refs.clone(),
        trace_id: operation.trace_id.clone(),
        source_operation_id: operation.operation_id.clone(),
        source_turn_id: turn_id.to_string(),
        source_step_id: last_round.tool_dispatch_step_id.clone(),
        checkpoint_kind: checkpoint_kind.into(),
        status: "open".into(),
        source_round_index: last_round.round_index,
        next_round_index,
        resume_input,
        summary: Some(match checkpoint_kind {
            "wait_reminder_resume" => format!(
                "resume round {next_round_index} after wait.remind from round {}",
                last_round.round_index
            ),
            _ => format!(
                "resume follow-up round {next_round_index} after tool dispatch in round {}",
                last_round.round_index
            ),
        }),
        created_at: created_at.into(),
        consumed_at: None,
        consumed_by_operation_id: None,
    })
}
