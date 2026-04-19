use super::*;

pub(super) struct StepAllocation {
    pub(super) step_id: String,
    pub(super) step_index: u32,
}

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
    round_index: u32,
    input: String,
) -> Result<RoundExecution, RuntimeError> {
    let rendered_input =
        ModelInputAssembler::default().assemble(&input, &operation.payload.context);
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
        &operation.payload.context,
        round_index,
        &parsed_output.tool_calls,
    );
    let assistant_response_text = parsed_output.user_response.clone();
    let fallback_feedback =
        ControlFeedbackBuilder.build(&operation.payload, &prepared_request, &provider_response);
    let mut control_feedback = ControlFeedbackBuilder::default()
        .merge_with_fallback(parsed_output.control_feedback.clone(), fallback_feedback);
    ControlFeedbackBuilder::default().rewrite_runtime_heuristic_candidates(
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

#[allow(clippy::too_many_arguments)]
pub(super) fn record_round(
    step_index: &mut u32,
    operation: &OperationEnvelope<InferenceOperationPayload>,
    refs: &EntityRefs,
    turn_id: &str,
    round_index: u32,
    prepared: &PreparedRequest,
    response: &ProviderResponse,
    parsed: &ParsedModelOutput,
    feedback: &ControlFeedback,
    dispatched: &tool_dispatch::ToolDispatchOutcome,
    assistant_text: &str,
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
            "round {round_index} provider {}:{} -> status={} stop_reason={}",
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
            "round {round_index} parsed provider output: contract_detected={} tool_calls={} assistant={}",
            parsed.contract_detected,
            parsed.tool_calls.len(),
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
            "round {round_index} control feedback: continuation={} shift={} simple={} reason={}",
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
        if parsed.tool_calls.is_empty() {
            "skipped"
        } else {
            "completed"
        },
        &operation.submitted_at,
        if parsed.tool_calls.is_empty() {
            format!("round {round_index} no model tools requested")
        } else {
            format!(
                "round {round_index} executed {} tool calls, stop_requested={}, reminder_scheduled={}",
                parsed.tool_calls.len(),
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

pub(super) fn build_followup_input(
    original_input: &str,
    assistant_response: &str,
    tool_records: &[ToolExecutionRecord],
) -> String {
    let tool_lines = tool_records
        .iter()
        .rev()
        .take(4)
        .map(|record| {
            format!(
                "{} => {}",
                record.tool_name,
                record
                    .output_summary
                    .clone()
                    .or_else(|| record.error_summary.clone())
                    .unwrap_or_else(|| record.status.clone())
            )
        })
        .collect::<Vec<_>>();
    format!(
        "Continue the same turn with the latest tool results.\nOriginal request: {original_input}\nLast assistant response: {assistant_response}\nRecent tool results:\n- {}",
        if tool_lines.is_empty() {
            "none".to_string()
        } else {
            tool_lines.join("\n- ")
        }
    )
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
