use super::*;
use closure_runtime_checkpoint::build_resume_checkpoint;
use closure_runtime_events::{EventEmissionInput, emit_runtime_events};
use closure_runtime_finalize::{
    append_checkpoint_recorded_event, append_finalize_step, build_final_run, build_partial_run,
};
use closure_runtime_rounds::{allocate_step, build_followup_input, execute_round, record_round};
use closure_runtime_state::{
    merge_dispatch_outcome, next_step, operation_status, record_auto_tool_round_limit, stop_source,
};
use round_context::{DynamicRoundContextInput, build_round_context};

#[path = "closure_runtime_checkpoint.rs"]
mod closure_runtime_checkpoint;
#[path = "closure_runtime_events.rs"]
mod closure_runtime_events;
#[path = "closure_runtime_finalize.rs"]
mod closure_runtime_finalize;
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
        let mut step_index = 0u32;
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
        let initial_round = execute_round(
            &operation,
            provider,
            &refs,
            &initial_round_context,
            1,
            operation.payload.input.clone(),
        )?;
        let mut prepared_request = initial_round.prepared_request;
        let mut provider_response = initial_round.provider_response;
        let mut provider_debug = initial_round.provider_debug;
        let mut parsed_output = initial_round.parsed_output;
        let mut dispatched_tools = initial_round.dispatched_tools.clone();
        let mut assistant_response_text = initial_round.assistant_response_text;
        let mut control_feedback = initial_round.control_feedback;
        let mut round_count = 1usize;
        let max_auto_tool_rounds = 6usize;
        let context_snapshot = ContextSnapshotRecord {
            operation_id: operation.operation_id.clone(),
            trace_id: operation.trace_id.clone(),
            refs: refs.clone(),
            input: operation.payload.input.clone(),
            context: operation.payload.context.clone(),
            role: operation.payload.role.clone(),
            provider_path: operation.payload.provider_path.clone(),
            provider_strategy: operation.payload.provider_strategy,
            protocol_version: operation.payload.protocol_version.clone(),
            stream: operation.payload.stream,
            captured_at: operation.submitted_at.clone(),
        };
        let mut provider_request_records = Vec::new();
        let mut provider_response_records = Vec::new();
        let mut round_records = Vec::new();
        let context_build_step =
            allocate_step(&mut step_index, &operation.operation_id, "context_build");
        let mut step_records = vec![turn_records::step_record(
            context_build_step.step_id,
            &turn_id,
            &operation.operation_id,
            &operation.trace_id,
            &refs,
            context_build_step.step_index,
            "context_build",
            "completed",
            &operation.submitted_at,
            format!(
                "assembled context for role={} with continuity_tail={} messages",
                operation.payload.role.role_id.as_str(),
                operation.payload.context.continuity_tail.len()
            ),
            Some(format!(
                "context/recent_contexts.json#operation_id={}",
                operation.operation_id
            )),
            Some(format!(
                "provider/recent_provider_requests.json#operation_id={}",
                operation.operation_id
            )),
            Some("provider_request".into()),
        )];

        let mut tool_records = vec![trace_records::provider_tool_record(
            &operation.operation_id,
            &operation.trace_id,
            &refs,
            &prepared_request,
            &provider_response,
            assistant_response_text.as_str(),
            &operation.submitted_at,
        )];
        tool_records.extend(dispatched_tools.tool_records.clone());
        let mut latest_round_tool_records = initial_round.dispatched_tools.tool_records.clone();
        record_round(
            &mut step_index,
            &operation,
            &refs,
            &turn_id,
            1,
            &prepared_request,
            &provider_response,
            &parsed_output,
            &control_feedback,
            &dispatched_tools,
            assistant_response_text.as_str(),
            &mut provider_request_records,
            &mut provider_response_records,
            &mut round_records,
            &mut step_records,
        );

        while !dispatched_tools.stop_requested
            && !dispatched_tools.yield_requested
            && !parsed_output.tool_calls.is_empty()
            && round_count < max_auto_tool_rounds
        {
            let next_round_index = round_count as u32 + 1;
            let followup_input = build_followup_input(
                operation.payload.input.as_str(),
                assistant_response_text.as_str(),
                &latest_round_tool_records,
            );
            let followup_round_context = build_round_context(DynamicRoundContextInput {
                role_id: operation.payload.role.role_id.as_str(),
                base_context: &operation.payload.context,
                operation_id: &operation.operation_id,
                trace_id: &operation.trace_id,
                round_index: next_round_index,
                current_input: &followup_input,
                previous_assistant_response: Some(assistant_response_text.as_str()),
                recent_tool_records: &tool_records,
            });
            let followup_round = execute_round(
                &operation,
                provider,
                &refs,
                &followup_round_context,
                next_round_index,
                followup_input,
            )?;
            tool_records.push(trace_records::provider_tool_record(
                &operation.operation_id,
                &operation.trace_id,
                &refs,
                &followup_round.prepared_request,
                &followup_round.provider_response,
                followup_round.assistant_response_text.as_str(),
                &operation.submitted_at,
            ));
            tool_records.extend(followup_round.dispatched_tools.tool_records.clone());
            let current_round_tools = followup_round.dispatched_tools.clone();
            latest_round_tool_records = current_round_tools.tool_records.clone();
            merge_dispatch_outcome(&mut dispatched_tools, &current_round_tools);
            prepared_request = followup_round.prepared_request;
            provider_response = followup_round.provider_response;
            provider_debug = followup_round.provider_debug;
            parsed_output = followup_round.parsed_output;
            assistant_response_text = followup_round.assistant_response_text;
            control_feedback = followup_round.control_feedback;
            round_count += 1;
            record_round(
                &mut step_index,
                &operation,
                &refs,
                &turn_id,
                round_count as u32,
                &prepared_request,
                &provider_response,
                &parsed_output,
                &control_feedback,
                &current_round_tools,
                assistant_response_text.as_str(),
                &mut provider_request_records,
                &mut provider_response_records,
                &mut round_records,
                &mut step_records,
            );
        }
        record_auto_tool_round_limit(
            &mut dispatched_tools,
            &parsed_output,
            round_count,
            max_auto_tool_rounds,
        );

        let closure_stopped = dispatched_tools.stop_requested;
        let closure_waiting_external = dispatched_tools.yield_requested;
        let resume_checkpoint = build_resume_checkpoint(
            &operation,
            &refs,
            &turn_id,
            round_records.last(),
            &parsed_output,
            &dispatched_tools,
            assistant_response_text.as_str(),
            &latest_round_tool_records,
            &operation.submitted_at,
        );
        let stop_source = stop_source(closure_waiting_external, closure_stopped);
        let operation_status = operation_status(closure_waiting_external, closure_stopped);
        let progress = ProgressBlock {
            progress_id: format!("progress-{}", operation.operation_id),
            refs: refs.clone(),
            phase: "inference_completed".into(),
            blocker: None,
            next_step: Some(next_step(dispatched_tools.reminder_scheduled, closure_stopped).into()),
            health_hint: Some("healthy".into()),
            tool_snapshots: tool_records
                .iter()
                .map(|record| ToolSnapshot {
                    tool_name: record.tool_name.clone(),
                    status: record.status.clone(),
                    summary: record
                        .output_summary
                        .clone()
                        .or_else(|| record.input_summary.clone())
                        .unwrap_or_else(|| record.purpose.clone()),
                })
                .collect(),
        };
        let note = ExecutionNote {
            note_id: format!("note-{}", operation.operation_id),
            refs: refs.clone(),
            summary: if dispatched_tools.note_hints.is_empty() {
                format!(
                    "provider {} returned: {}",
                    prepared_request.provider_name,
                    assistant_response_text.as_str()
                )
            } else {
                format!(
                    "provider {} returned: {}; {}",
                    prepared_request.provider_name,
                    assistant_response_text.as_str(),
                    dispatched_tools.note_hints.join(" | ")
                )
            },
            decision: Some(if dispatched_tools.reminder_scheduled {
                "wait_for_system_self_reminder".into()
            } else if !closure_stopped {
                "continue_reasoning".into()
            } else if control_feedback.is_continuation {
                "continue_current_task".into()
            } else {
                "observe_topic_continuity".into()
            }),
            lesson: None,
            blocker: None,
            next_step: Some(next_step(dispatched_tools.reminder_scheduled, closure_stopped).into()),
            control_feedback: Some(control_feedback.clone()),
            created_at: operation.submitted_at.clone(),
        };
        let reasoning_view = trace_records::reasoning_view_record(
            &operation.operation_id,
            &operation.trace_id,
            &refs,
            &operation.submitted_at,
            &note,
            &control_feedback,
            &tool_records,
        );
        let digest = DigestRecord {
            digest_id: digest_id.clone(),
            closure_id: closure_id.clone(),
            refs: refs.clone(),
            summary: format!(
                "closure {} with model {} and answer {}",
                if closure_waiting_external {
                    "waiting_external"
                } else if closure_stopped {
                    "stopped"
                } else {
                    "checkpointed_without_reasoning_stop"
                },
                prepared_request.model,
                assistant_response_text.as_str()
            ),
            continuity_tail: vec![
                operation.payload.input.clone(),
                assistant_response_text.clone(),
            ],
            note_refs: vec![note.note_id.clone()],
            artifact_candidates: vec![format!(
                "provider:{}:{}:{}",
                prepared_request.provider_name,
                prepared_request.model,
                assistant_response_text.as_str()
            )],
            control_feedback: Some(control_feedback.clone()),
            created_at: operation.submitted_at.clone(),
        };
        let routing_decision = turn_records::routing_decision_record(
            &operation.operation_id,
            &operation.trace_id,
            &refs,
            &operation.submitted_at,
            &control_feedback,
        );
        let routing_action = routing_actions::derive_routing_action(&routing_decision);
        append_finalize_step(
            &mut step_index,
            &operation,
            &refs,
            &turn_id,
            &digest_id,
            stop_source,
            assistant_response_text.as_str(),
            &progress,
            &note,
            &mut step_records,
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
            &progress,
            &note,
            &tool_records,
            &provider_request_records,
            &provider_response_records,
            &step_records,
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
                round_records: &round_records,
                step_records: &mut step_records,
                tool_records: &tool_records,
                dispatched_tools: &dispatched_tools,
                round_count,
                closure_stopped,
                progress: &progress,
                note: &note,
                reasoning_view: &reasoning_view,
                routing_decision: &routing_decision,
                routing_action: &routing_action,
                digest: &digest,
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
            &tool_records,
            &provider_request_records,
            &provider_response_records,
            &round_records,
            &step_records,
            &progress,
            &note,
            &reasoning_view,
            &digest,
            &turn_record,
            &routing_decision,
            &routing_action,
            resume_checkpoint.as_ref(),
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
