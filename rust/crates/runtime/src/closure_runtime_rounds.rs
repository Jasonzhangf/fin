use super::*;
use crate::model_output::ModelToolCall;
use serde_json::json;
use std::{fs, path::PathBuf};

pub(super) struct StepAllocation {
    pub(super) step_id: String,
    pub(super) step_index: u32,
}

#[derive(Debug, Clone)]
pub(super) struct RoundExecution {
    pub(super) prepared_request: PreparedRequest,
    pub(super) provider_response: ProviderResponse,
    pub(super) provider_debug: SanitizedProviderDebug,
    pub(super) parsed_output: ParsedModelOutput,
    pub(super) dispatched_tools: tool_dispatch::ToolDispatchOutcome,
    pub(super) assistant_response_text: String,
    pub(super) control_feedback: ControlFeedback,
}

pub(super) fn execute_round(
    operation: &OperationEnvelope<InferenceOperationPayload>,
    provider: &impl InferenceProvider,
    refs: &EntityRefs,
    round_context: &MinimalContextView,
    round_index: u32,
    input: String,
    prior_tool_calls: &[ModelToolCall],
    tool_results: &[ToolExecutionRecord],
) -> Result<RoundExecution, RuntimeError> {
    let rendered_input = ModelInputAssembler::default().assemble(&input, round_context);
    let prepared_request = provider.prepare_request(&ProviderRequest {
        input,
        rendered_input: Some(rendered_input),
        override_model: Some(
            operation
                .payload
                .provider_path
                .primary_target()
                .model
                .clone(),
        ),
        tools: build_provider_tool_specs(round_context),
        prior_tool_calls: prior_tool_calls
            .iter()
            .map(model_tool_call_to_provider_tool_call)
            .collect(),
        tool_results: build_provider_tool_results(round_context, tool_results),
    });
    let provider_response = provider.execute_prepared(&prepared_request)?;
    let provider_debug = SanitizedProviderDebug {
        user_agent: prepared_request.user_agent.clone(),
        request_headers: prepared_request.sanitized_headers.clone(),
    };
    let parsed_output = ModelOutputParser::default().parse(
        &operation.payload,
        &prepared_request,
        &provider_response,
    );
    let dispatched_tools = tool_dispatch::execute_model_tools(
        &operation.operation_id,
        &operation.trace_id,
        refs,
        &operation.submitted_at,
        round_context,
        round_index,
        &parsed_output.tool_calls,
    );
    let assistant_response_text = parsed_output.user_response.clone();
    let runtime_observation_feedback =
        ControlFeedbackBuilder.build(&operation.payload, &prepared_request, &provider_response);
    let mut control_feedback = ControlFeedbackBuilder::default()
        .merge_with_runtime_observation(
            parsed_output.control_feedback.clone(),
            runtime_observation_feedback,
        );
    ControlFeedbackBuilder::default().rewrite_runtime_observation_candidates(
        &mut control_feedback,
        &prepared_request,
        &provider_response,
        assistant_response_text.as_str(),
    );
    Ok(RoundExecution {
        prepared_request,
        provider_response,
        provider_debug,
        parsed_output,
        dispatched_tools,
        assistant_response_text,
        control_feedback,
    })
}

pub(super) fn build_context_snapshot(
    operation: &OperationEnvelope<InferenceOperationPayload>,
    refs: &EntityRefs,
) -> ContextSnapshotRecord {
    ContextSnapshotRecord {
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
    }
}

pub(super) fn build_context_build_step_record(
    step_id: String,
    step_index: u32,
    operation: &OperationEnvelope<InferenceOperationPayload>,
    refs: &EntityRefs,
    turn_id: &str,
) -> StepRecord {
    turn_records::step_record(
        step_id,
        turn_id,
        &operation.operation_id,
        &operation.trace_id,
        refs,
        step_index,
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
    )
}

#[allow(clippy::too_many_arguments)]
pub(super) fn record_round(
    step_index: &mut u32,
    operation: &OperationEnvelope<InferenceOperationPayload>,
    refs: &EntityRefs,
    turn_id: &str,
    round_index: u32,
    attempt_index: u32,
    accepted_attempt: bool,
    prepared: &PreparedRequest,
    response: &ProviderResponse,
    parsed: &ParsedModelOutput,
    feedback: &ControlFeedback,
    dispatched: &tool_dispatch::ToolDispatchOutcome,
    assistant_text: &str,
    validation_errors: &[String],
    provider_request_records: &mut Vec<ProviderRequestRecord>,
    provider_response_records: &mut Vec<ProviderResponseRecord>,
    round_records: &mut Vec<RoundRecord>,
    step_records: &mut Vec<StepRecord>,
) {
    let provider_step = allocate_step(step_index, &operation.operation_id, "provider_request");
    let request_record = turn_records::provider_request_record(
        &operation.operation_id,
        &operation.trace_id,
        refs,
        turn_id,
        &provider_step.step_id,
        round_index,
        attempt_index,
        prepared,
        &operation.submitted_at,
    );
    let response_record = turn_records::provider_response_record(
        &operation.operation_id,
        &operation.trace_id,
        refs,
        turn_id,
        &provider_step.step_id,
        round_index,
        attempt_index,
        &request_record.request_id,
        response,
        &operation.submitted_at,
    );
    let model_parse_step = allocate_step(step_index, &operation.operation_id, "model_parse");
    let control_feedback_step =
        allocate_step(step_index, &operation.operation_id, "control_feedback");
    let tool_dispatch_step = allocate_step(step_index, &operation.operation_id, "tool_dispatch");

    provider_request_records.push(request_record.clone());
    provider_response_records.push(response_record.clone());
    step_records.push(turn_records::step_record(
        provider_step.step_id.clone(),
        turn_id,
        &operation.operation_id,
        &operation.trace_id,
        refs,
        provider_step.step_index,
        "provider_request",
        if response.status >= 400 {
            "failed"
        } else {
            "completed"
        },
        &operation.submitted_at,
        format!(
            "round {round_index} attempt {attempt_index} provider {}:{} -> status={} stop_reason={}",
            prepared.provider_name,
            prepared.model,
            response.status,
            response.stop_reason.as_deref().unwrap_or("unknown")
        ),
        Some(format!(
            "provider/recent_provider_requests.json#request_id={}",
            request_record.request_id
        )),
        Some(format!(
            "provider/recent_provider_responses.json#response_record_id={}",
            response_record.response_record_id
        )),
        Some("model_parse".into()),
    ));
    step_records.push(turn_records::step_record(
        model_parse_step.step_id.clone(),
        turn_id,
        &operation.operation_id,
        &operation.trace_id,
        refs,
        model_parse_step.step_index,
        "model_parse",
        "completed",
        &operation.submitted_at,
        format!(
            "round {round_index} attempt {attempt_index} parsed provider output: contract_detected={} tool_calls={} tool_calls_status={} tool_calls_invalid_reason={} accepted={} validation_errors={} assistant={}",
            parsed.contract_detected,
            parsed.tool_calls.len(),
            parsed.tool_calls_parse_status,
            parsed
                .tool_calls_invalid_reason
                .as_deref()
                .unwrap_or("none"),
            accepted_attempt,
            if validation_errors.is_empty() {
                "none".to_string()
            } else {
                validation_errors.join(" | ")
            },
            assistant_text.trim()
        ),
        Some(format!(
            "provider/recent_provider_responses.json#response_record_id={}",
            response_record.response_record_id
        )),
        Some(format!(
            "control/latest.json#operation_id={}",
            operation.operation_id
        )),
        Some("control_feedback".into()),
    ));
    step_records.push(turn_records::step_record(
        control_feedback_step.step_id.clone(),
        turn_id,
        &operation.operation_id,
        &operation.trace_id,
        refs,
        control_feedback_step.step_index,
        "control_feedback",
        "completed",
        &operation.submitted_at,
        format!(
            "round {round_index} attempt {attempt_index} control feedback: continuation={} shift={} simple={} reason={}",
            feedback.is_continuation,
            feedback.topic_shift_confidence,
            feedback.simple_query_confidence,
            feedback.reason
        ),
        Some(format!(
            "control/latest.json#operation_id={}",
            operation.operation_id
        )),
        Some(format!(
            "tasks/routing/recent_decisions.json#operation_id={}",
            operation.operation_id
        )),
        Some("tool_dispatch".into()),
    ));
    step_records.push(turn_records::step_record(
        tool_dispatch_step.step_id.clone(),
        turn_id,
        &operation.operation_id,
        &operation.trace_id,
        refs,
        tool_dispatch_step.step_index,
        "tool_dispatch",
        if parsed.tool_calls.is_empty() && parsed.tool_calls_block_present {
            "failed"
        } else if parsed.tool_calls.is_empty() {
            "skipped"
        } else {
            "completed"
        },
        &operation.submitted_at,
        if parsed.tool_calls.is_empty() && parsed.tool_calls_block_present {
            format!(
                "round {round_index} attempt {attempt_index} detected non-executable tool block: status={} reason={}",
                parsed.tool_calls_parse_status,
                parsed
                    .tool_calls_invalid_reason
                    .as_deref()
                    .unwrap_or("unknown")
            )
        } else if parsed.tool_calls.is_empty() {
            format!("round {round_index} attempt {attempt_index} no model tools requested")
        } else {
            format!(
                "round {round_index} attempt {attempt_index} executed {} tool calls, parse_status={}, stop_requested={}, reminder_scheduled={}",
                parsed.tool_calls.len(),
                parsed.tool_calls_parse_status,
                dispatched.stop_requested,
                dispatched.reminder_scheduled
            )
        },
        Some(format!(
            "tools/recent_tool_records.json#operation_id={}",
            operation.operation_id
        )),
        Some(format!(
            "reasoning/recent_reasoning_views.json#operation_id={}",
            operation.operation_id
        )),
        Some("finalize".into()),
    ));
    if accepted_attempt {
        round_records.push(RoundRecord {
            round_id: format!("round-{}-{round_index:02}", operation.operation_id),
            turn_id: turn_id.to_string(),
            operation_id: operation.operation_id.clone(),
            trace_id: operation.trace_id.clone(),
            round_index,
            created_at: operation.submitted_at.clone(),
            refs: refs.clone(),
            provider_step_id: provider_step.step_id,
            model_parse_step_id: model_parse_step.step_id,
            control_feedback_step_id: control_feedback_step.step_id,
            tool_dispatch_step_id: tool_dispatch_step.step_id,
            request_id: request_record.request_id,
            response_record_id: response_record.response_record_id,
            contract_detected: parsed.contract_detected,
            control_feedback_parsed: parsed.control_feedback.is_some(),
            control_feedback_salvaged: parsed.control_feedback_salvaged,
            tool_calls_count: parsed.tool_calls.len(),
            assistant_response_summary: assistant_text.to_string(),
            control_feedback_origin: feedback.origin.clone(),
            stop_requested: dispatched.stop_requested,
            yield_requested: dispatched.yield_requested,
            reminder_scheduled: dispatched.reminder_scheduled,
        });
    }
}

pub(super) fn build_followup_input(
    original_input: &str,
    assistant_response: &str,
    no_tool_calls: bool,
) -> String {
    if no_tool_calls {
        format!(
            "Continue the same turn.\nOriginal request: {original_input}\nLast assistant response: {assistant_response}\nYour last response had no tool calls and no reasoning.stop signal. You must explicitly declare whether the turn should end.\nIf the task is now complete, call reasoning.stop and emit control feedback that proves completion (task_completed=true plus non-empty completion_evidence and final_conclusions). If this is just a simple chat closure, set is_simple_chat=true. If you are blocked and need the user to do something, set blocked=true, needs_user_involve=true, and fill blocked_reason plus what_needs_to_be_done_by_user.\nIf you need to do more work instead, call the appropriate tools.\nOnly request reasoning.stop when the current reasoning cycle should really stop and the control feedback already contains one of those valid closure channels.\nIf you produce no visible user response (empty fin_user_response), you must still output a default acknowledgement like 'Processing, please wait.' inside fin_user_response."
        )
    } else {
        format!(
            "Continue the same turn.\nOriginal request: {original_input}\nLast assistant response: {assistant_response}\nThe previous round's native tool results are attached in this request; inspect them directly before deciding whether another tool is needed.\nIf the task is now complete, answer directly and emit control feedback that proves completion (task_completed=true plus non-empty completion_evidence and final_conclusions). If this is just a simple chat closure, set is_simple_chat=true. If you are blocked and need the user to do something, set blocked=true, needs_user_involve=true, and fill blocked_reason plus what_needs_to_be_done_by_user.\nOnly request reasoning.stop when the current reasoning cycle should really stop and the control feedback already contains one of those valid closure channels. Otherwise continue reasoning and do not stop yet."
        )
    }
}

pub(super) fn allocate_step(
    step_index: &mut u32,
    operation_id: &str,
    step_kind: &str,
) -> StepAllocation {
    *step_index += 1;
    StepAllocation {
        step_id: format!("step-{operation_id}-{:02}-{step_kind}", *step_index),
        step_index: *step_index,
    }
}

pub(super) fn append_step_event_id(step_records: &mut [StepRecord], step_id: &str, event_id: &str) {
    if let Some(step) = step_records.iter_mut().find(|item| item.step_id == step_id) {
        step.event_ids.push(event_id.to_string());
    }
}

#[path = "closure_runtime_rounds_tools.rs"]
mod closure_runtime_rounds_tools;
use self::closure_runtime_rounds_tools::{
    build_provider_tool_results, build_provider_tool_specs, model_tool_call_to_provider_tool_call,
};
