use super::*;
use closure_runtime_accumulator::ClosureAccumulator;
use closure_runtime_checkpoint::build_resume_checkpoint;
use closure_runtime_contract_retry::{
    MAX_OUTPUT_CONTRACT_RETRIES, execute_round_with_contract_retries,
};
use closure_runtime_events::{EventEmissionInput, emit_runtime_events};
use closure_runtime_finalize::{
    append_checkpoint_recorded_event, append_finalize_step, build_final_run, build_partial_run,
};
use closure_runtime_records::build_closure_records;
use closure_runtime_rounds::{build_context_snapshot, build_followup_input};
use closure_runtime_state::{
    merge_dispatch_outcome, operation_status, record_auto_tool_round_limit,
    record_output_contract_retry_limit, stop_source,
};
use round_context::{DynamicRoundContextInput, build_round_context};

#[path = "closure_runtime_accumulator.rs"]
mod closure_runtime_accumulator;
#[path = "closure_runtime_checkpoint.rs"]
mod closure_runtime_checkpoint;
#[path = "closure_runtime_contract_retry.rs"]
mod closure_runtime_contract_retry;
#[path = "closure_runtime_events.rs"]
mod closure_runtime_events;
#[path = "closure_runtime_finalize.rs"]
mod closure_runtime_finalize;
#[path = "closure_runtime_records.rs"]
mod closure_runtime_records;
#[path = "closure_runtime_rounds.rs"]
mod closure_runtime_rounds;
#[path = "closure_runtime_state.rs"]
mod closure_runtime_state;

#[derive(Debug, Clone)]
pub struct M1Runtime {
    source: String,
    sequence: u64,
}

impl Default for M1Runtime {
    fn default() -> Self {
        Self::new("runtime")
    }
}

impl M1Runtime {
    pub fn new(source: impl Into<String>) -> Self {
        Self {
            source: source.into(),
            sequence: 0,
        }
    }

    pub fn run_closure(
        &mut self,
        operation: OperationEnvelope<InferenceOperationPayload>,
        provider: &impl InferenceProvider,
    ) -> Result<ClosureRun, RuntimeError> {
        operation.validate()?;
        operation.payload.validate()?;
        let refs = operation.refs.clone();
        let turn_id = format!("turn-{}", operation.operation_id);
        let closure_id = format!("closure-{}", operation.operation_id);
        let digest_id = format!("digest-{}", operation.operation_id);
        let initial_round_context = build_round_context(DynamicRoundContextInput {
            role_id: operation.payload.role.role_id.as_str(),
            base_context: &operation.payload.context,
            operation_id: &operation.operation_id,
            trace_id: &operation.trace_id,
            round_index: 1,
            current_input: &operation.payload.input,
            previous_assistant_response: None,
            recent_tool_records: &[],
        });
        let initial_retry_bundle = execute_round_with_contract_retries(
            &operation,
            provider,
            &refs,
            &initial_round_context,
            1,
            operation.payload.input.clone(),
        )?;
        let initial_round = initial_retry_bundle.final_round.clone();
        let mut prepared_request = initial_round.prepared_request.clone();
        let mut provider_response = initial_round.provider_response.clone();
        let mut provider_debug = initial_round.provider_debug.clone();
        let mut parsed_output = initial_round.parsed_output.clone();
        let mut dispatched_tools = initial_round.dispatched_tools.clone();
        let mut assistant_response_text = initial_round.assistant_response_text.clone();
        let mut control_feedback = initial_round.control_feedback.clone();
        let mut round_count = 1usize;
        let max_auto_tool_rounds = 6usize;
        let context_snapshot = build_context_snapshot(&operation, &refs);
        let mut accumulator = ClosureAccumulator::new(&operation, &refs, &turn_id);
        accumulator.absorb_retry_bundle(
            &operation,
            &refs,
            &turn_id,
            1,
            &operation.submitted_at,
            &initial_retry_bundle,
        );

        while !dispatched_tools.stop_requested
            && !dispatched_tools.yield_requested
            && !parsed_output.tool_calls.is_empty()
            && round_count < max_auto_tool_rounds
        {
            let next_round_index = round_count as u32 + 1;
            let followup_input = build_followup_input(
                &operation.payload.context,
                operation.payload.input.as_str(),
                assistant_response_text.as_str(),
                &accumulator.latest_round_tool_records,
            );
            let followup_round_context = build_round_context(DynamicRoundContextInput {
                role_id: operation.payload.role.role_id.as_str(),
                base_context: &operation.payload.context,
                operation_id: &operation.operation_id,
                trace_id: &operation.trace_id,
                round_index: next_round_index,
                current_input: &followup_input,
                previous_assistant_response: Some(assistant_response_text.as_str()),
                recent_tool_records: &accumulator.tool_records,
            });
            let followup_retry_bundle = execute_round_with_contract_retries(
                &operation,
                provider,
                &refs,
                &followup_round_context,
                next_round_index,
                followup_input,
            )?;
            let followup_round = followup_retry_bundle.final_round.clone();
            accumulator.absorb_retry_bundle(
                &operation,
                &refs,
                &turn_id,
                next_round_index,
                &operation.submitted_at,
                &followup_retry_bundle,
            );
            let current_round_tools = followup_round.dispatched_tools.clone();
            merge_dispatch_outcome(&mut dispatched_tools, &current_round_tools);
            prepared_request = followup_round.prepared_request.clone();
            provider_response = followup_round.provider_response.clone();
            provider_debug = followup_round.provider_debug.clone();
            parsed_output = followup_round.parsed_output.clone();
            assistant_response_text = followup_round.assistant_response_text.clone();
            control_feedback = followup_round.control_feedback.clone();
            round_count += 1;
        }
        record_auto_tool_round_limit(
            &mut dispatched_tools,
            &parsed_output,
            round_count,
            max_auto_tool_rounds,
        );
        record_output_contract_retry_limit(
            &mut dispatched_tools,
            &accumulator.contract_retry_summaries,
            MAX_OUTPUT_CONTRACT_RETRIES,
        );

        let closure_stopped = dispatched_tools.stop_requested;
        let closure_waiting_external = dispatched_tools.yield_requested;
        let resume_checkpoint = build_resume_checkpoint(
            &operation,
            &refs,
            &turn_id,
            accumulator.round_records.last(),
            &parsed_output,
            &dispatched_tools,
            assistant_response_text.as_str(),
            &accumulator.latest_round_tool_records,
            &operation.submitted_at,
        );
        let stop_source = stop_source(closure_waiting_external, closure_stopped);
        let operation_status = operation_status(closure_waiting_external, closure_stopped);
        let records = build_closure_records(
            &operation,
            &refs,
            &prepared_request,
            assistant_response_text.as_str(),
            &dispatched_tools,
            closure_waiting_external,
            closure_stopped,
            Some(stop_source),
            &control_feedback,
            &accumulator.tool_records,
            &digest_id,
            &closure_id,
        );
        append_finalize_step(
            &mut accumulator.step_index,
            &operation,
            &refs,
            &turn_id,
            &digest_id,
            stop_source,
            assistant_response_text.as_str(),
            &records.progress,
            &records.note,
            &mut accumulator.step_records,
        );
        let turn_record = turn_records::turn_record(
            &operation.operation_id,
            &operation.trace_id,
            &refs,
            &turn_id,
            &closure_id,
            &operation.submitted_at,
            &operation.payload.input,
            assistant_response_text.as_str(),
            &records.progress,
            &records.note,
            &accumulator.tool_records,
            &accumulator.provider_request_records,
            &accumulator.provider_response_records,
            &accumulator.step_records,
            "completed",
        );
        let mut events = emit_runtime_events(
            self,
            EventEmissionInput {
                operation: &operation,
                refs: &refs,
                prepared_request: &prepared_request,
                provider_response: &provider_response,
                provider_debug: &provider_debug,
                parsed_output: &parsed_output,
                control_feedback: &control_feedback,
                assistant_response_text: assistant_response_text.as_str(),
                round_records: &accumulator.round_records,
                step_records: &mut accumulator.step_records,
                tool_records: &accumulator.tool_records,
                dispatched_tools: &dispatched_tools,
                round_count,
                closure_stopped,
                progress: &records.progress,
                note: &records.note,
                reasoning_view: &records.reasoning_view,
                routing_decision: &records.routing_decision,
                routing_action: &records.routing_action,
                digest: &records.digest,
                turn_record: &turn_record,
            },
        )?;

        let partial_run = build_partial_run(
            &operation,
            &prepared_request,
            &provider_response,
            assistant_response_text.as_str(),
            &control_feedback,
            &context_snapshot,
            &accumulator.tool_records,
            &accumulator.provider_request_records,
            &accumulator.provider_response_records,
            &accumulator.round_records,
            &accumulator.step_records,
            &records.progress,
            &records.note,
            &records.reasoning_view,
            &records.digest,
            &turn_record,
            &records.routing_decision,
            &records.routing_action,
            resume_checkpoint.as_ref(),
            &accumulator.compacted_history_records,
            &events,
        );
        let closure_trace = trace_records::closure_trace_record(&partial_run);
        events.push(self.event(
            "closure.trace_recorded",
            &operation.trace_id,
            &operation.submitted_at,
            &refs,
            Some(operation.operation_id.clone()),
            serde_json::to_value(&closure_trace)?,
        )?);
        events.push(self.event(
            "operation.completed",
            &operation.trace_id,
            &operation.submitted_at,
            &refs,
            Some(operation.operation_id.clone()),
            serde_json::json!({
                "status": operation_status,
                "stop_source": stop_source,
                "answer": assistant_response_text.clone(),
            }),
        )?);
        append_checkpoint_recorded_event(
            self,
            &mut events,
            &operation,
            &refs,
            resume_checkpoint.as_ref(),
        )?;

        Ok(build_final_run(
            operation,
            prepared_request,
            provider_response,
            assistant_response_text,
            control_feedback,
            context_snapshot,
            accumulator.tool_records,
            accumulator.provider_request_records,
            accumulator.provider_response_records,
            accumulator.round_records,
            accumulator.step_records,
            records.progress,
            records.note,
            records.reasoning_view,
            records.digest,
            turn_record,
            records.routing_decision,
            records.routing_action,
            resume_checkpoint,
            accumulator.compacted_history_records,
            closure_trace,
            events,
        ))
    }

    pub(super) fn event(
        &mut self,
        event_type: &str,
        trace_id: &str,
        occurred_at: &str,
        refs: &EntityRefs,
        operation_id: Option<String>,
        payload: Value,
    ) -> Result<EventEnvelope<Value>, RuntimeError> {
        self.sequence += 1;
        let mut event = EventEnvelope::new(
            format!("evt-{}", self.sequence),
            event_type,
            occurred_at.to_string(),
            self.source.clone(),
            trace_id.to_string(),
            self.sequence,
            payload,
        );
        event.refs = refs.clone();
        event.operation_id = operation_id;
        event.validate()?;
        Ok(event)
    }
}

pub fn map_runtime_error_through_error_pipeline(
    operation: &OperationEnvelope<InferenceOperationPayload>,
    err: &RuntimeError,
) -> crate::error_pipeline::ErrorErr05UserVisible {
    use crate::error_pipeline::{
        ErrorErr01DetectedBuilder, ErrorErr02SourceClassifiedBuilder,
        ErrorErr03RuntimeClassifiedBuilder, ErrorErr04SessionRecordedBuilder,
        ErrorErr05UserVisibleBuilder,
        classify_runtime_error,
    };
    let detected = ErrorErr01DetectedBuilder
        .build(
            "M1Runtime::run_closure",
            err.to_string(),
            operation.submitted_at.clone(),
        )
        .expect("error fact non-empty");
    let (source_class, decision) = classify_runtime_error(err);
    let classified = ErrorErr02SourceClassifiedBuilder.build(detected, source_class);
    let runtime_classified = ErrorErr03RuntimeClassifiedBuilder.build(classified, decision);
    let recorded = ErrorErr04SessionRecordedBuilder
        .build(
            runtime_classified,
            format!("err-{}", operation.operation_id),
            format!("sessions/ledger/{}.jsonl", operation.operation_id),
        )
        .expect("error record ids non-empty");
    ErrorErr05UserVisibleBuilder
        .build(recorded, "runtime failure: see session ledger for trace")
        .expect("user message non-empty")
}
