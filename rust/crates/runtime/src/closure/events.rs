use super::rounds::append_step_event_id;
use super::*;

pub(super) struct EventEmissionInput<'a> {
    pub(super) operation: &'a OperationEnvelope<InferenceOperationPayload>,
    pub(super) refs: &'a EntityRefs,
    pub(super) prepared_request: &'a PreparedRequest,
    pub(super) provider_response: &'a ProviderResponse,
    pub(super) provider_debug: &'a SanitizedProviderDebug,
    pub(super) parsed_output: &'a ParsedModelOutput,
    pub(super) control_feedback: &'a ControlFeedback,
    pub(super) assistant_response_text: &'a str,
    pub(super) round_records: &'a [RoundRecord],
    pub(super) step_records: &'a mut Vec<StepRecord>,
    pub(super) tool_records: &'a [ToolExecutionRecord],
    pub(super) dispatched_tools: &'a tool_dispatch::ToolDispatchOutcome,
    pub(super) round_count: usize,
    pub(super) closure_stopped: bool,
    pub(super) progress: &'a ProgressBlock,
    pub(super) note: &'a ExecutionNote,
    pub(super) reasoning_view: &'a ReasoningViewRecord,
    pub(super) routing_decision: &'a RoutingDecisionRecord,
    pub(super) routing_action: &'a RoutingActionRecord,
    pub(super) digest: &'a DigestRecord,
    pub(super) turn_record: &'a TurnRecord,
}

pub(super) fn emit_runtime_events(
    runtime: &mut M1Runtime,
    input: EventEmissionInput<'_>,
) -> Result<Vec<EventEnvelope<Value>>, RuntimeError> {
    let mut events = Vec::new();
    let mut push_event = |event_type: &str, payload: Value| -> Result<String, RuntimeError> {
        let event = runtime.event(
            event_type,
            &input.operation.trace_id,
            &input.operation.submitted_at,
            input.refs,
            Some(input.operation.operation_id.clone()),
            payload,
        )?;
        let event_id = event.event_id.clone();
        events.push(event);
        Ok(event_id)
    };

    let operation_accepted_event_id = push_event(
        "operation.accepted",
        serde_json::json!({"operation_type": input.operation.operation_type.clone()}),
    )?;
    let inference_started_event_id = push_event(
        "inference.started",
        serde_json::json!({
            "provider": input.prepared_request.provider_name.clone(),
            "model": input.prepared_request.model.clone(),
            "input": input.operation.payload.input.clone(),
            "rendered_input": input.prepared_request.rendered_input.clone(),
            "context": input.operation.payload.context.clone(),
            "role": input.operation.payload.role.clone(),
            "provider_path": input.operation.payload.provider_path.clone(),
            "provider_strategy": input.operation.payload.provider_strategy,
            "protocol_version": input.operation.payload.protocol_version.clone(),
            "stream": input.operation.payload.stream,
        }),
    )?;
    append_step_event_id(
        input.step_records,
        &format!("step-{}-01-context_build", input.operation.operation_id),
        &operation_accepted_event_id,
    );
    append_step_event_id(
        input.step_records,
        &format!("step-{}-01-context_build", input.operation.operation_id),
        &inference_started_event_id,
    );

    let provider_started_payload = ProviderEventPayload {
        provider_name: input.prepared_request.provider_name.clone(),
        model: input.prepared_request.model.clone(),
        endpoint: input.prepared_request.endpoint.clone(),
        output_text: None,
        response_id: None,
        stop_reason: None,
        status: None,
        debug: Some(input.provider_debug.clone()),
    };
    let provider_payload = ProviderEventPayload {
        provider_name: input.prepared_request.provider_name.clone(),
        model: input.prepared_request.model.clone(),
        endpoint: input.prepared_request.endpoint.clone(),
        output_text: Some(input.provider_response.output_text.clone()),
        response_id: input.provider_response.response_id.clone(),
        stop_reason: input.provider_response.stop_reason.clone(),
        status: Some(input.provider_response.status),
        debug: Some(input.provider_debug.clone()),
    };
    push_event(
        "provider.operation_accepted",
        serde_json::to_value(&provider_started_payload)?,
    )?;
    push_event(
        "provider.gateway_request_sent",
        serde_json::json!({
            "provider_name": input.prepared_request.provider_name.clone(),
            "model": input.prepared_request.model.clone(),
            "endpoint": input.prepared_request.endpoint.clone(),
        }),
    )?;
    push_event(
        "provider.gateway_response_received",
        serde_json::to_value(&provider_payload)?,
    )?;
    push_event(
        "provider.response_normalized",
        serde_json::to_value(&provider_payload)?,
    )?;
    push_event(
        "provider.completed",
        serde_json::to_value(&provider_payload)?,
    )?;

    for round in input.round_records {
        let provider_round_event_id = push_event(
            "provider.round_completed",
            serde_json::json!({
                "round_index": round.round_index,
                "request_id": round.request_id,
                "response_record_id": round.response_record_id,
            }),
        )?;
        append_step_event_id(
            input.step_records,
            &round.provider_step_id,
            &provider_round_event_id,
        );
        let model_round_event_id = push_event(
            "model.output_round_parsed",
            serde_json::json!({
                "round_index": round.round_index,
                "contract_detected": round.contract_detected,
                "control_feedback_parsed": round.control_feedback_parsed,
                "control_feedback_salvaged": round.control_feedback_salvaged,
                "tool_calls_count": round.tool_calls_count,
                "assistant_response": round.assistant_response_summary,
                "control_feedback_origin": round.control_feedback_origin,
            }),
        )?;
        append_step_event_id(
            input.step_records,
            &round.model_parse_step_id,
            &model_round_event_id,
        );
        let control_round_event_id = push_event(
            "control.feedback_round_recorded",
            serde_json::json!({
                "round_index": round.round_index,
                "origin": round.control_feedback_origin,
                "response_record_id": round.response_record_id,
            }),
        )?;
        append_step_event_id(
            input.step_records,
            &round.control_feedback_step_id,
            &control_round_event_id,
        );
        let tool_round_event_id = push_event(
            "tool.dispatch_round_completed",
            serde_json::json!({
                "round_index": round.round_index,
                "tool_calls_count": round.tool_calls_count,
                "stop_requested": round.stop_requested,
                "yield_requested": round.yield_requested,
                "reminder_scheduled": round.reminder_scheduled,
            }),
        )?;
        append_step_event_id(
            input.step_records,
            &round.tool_dispatch_step_id,
            &tool_round_event_id,
        );
    }

    push_event(
        "model.output_parsed",
        serde_json::json!({
            "contract_detected": input.parsed_output.contract_detected,
            "control_feedback_parsed": input.parsed_output.control_feedback.is_some(),
            "control_feedback_salvaged": input.parsed_output.control_feedback_salvaged,
            "tool_calls_block_present": input.parsed_output.tool_calls_block_present,
            "tool_calls_parse_status": input.parsed_output.tool_calls_parse_status.clone(),
            "tool_calls_invalid_reason": input.parsed_output.tool_calls_invalid_reason.clone(),
            "tool_calls_count": input.parsed_output.tool_calls.len(),
            "round_count": input.round_count,
            "assistant_response": input.assistant_response_text,
            "control_feedback_origin": input.control_feedback.origin,
        }),
    )?;
    if input.round_count > 1 {
        push_event(
            "reasoning.auto_tool_roundtrip_completed",
            serde_json::json!({
                "round_count": input.round_count,
                "stop_requested": input.closure_stopped,
            }),
        )?;
    }
    for (event_type, payload) in &input.dispatched_tools.events {
        push_event(event_type.as_str(), payload.clone())?;
    }
    push_event("progress.updated", serde_json::to_value(input.progress)?)?;
    push_event(
        "tool.execution_recorded",
        serde_json::to_value(input.tool_records)?,
    )?;
    push_event(
        "control.feedback_recorded",
        serde_json::to_value(input.control_feedback)?,
    )?;
    push_event("execution_note.appended", serde_json::to_value(input.note)?)?;
    push_event(
        "reasoning.view_recorded",
        serde_json::to_value(input.reasoning_view)?,
    )?;
    push_event(
        "routing.decision_recorded",
        serde_json::to_value(input.routing_decision)?,
    )?;
    push_event(
        "routing.action_derived",
        serde_json::to_value(input.routing_action)?,
    )?;
    let step_ledger_event_id = push_event(
        "step.ledger_recorded",
        serde_json::to_value(&input.step_records)?,
    )?;
    let digest_event_id = push_event("digest.finalized", serde_json::to_value(input.digest)?)?;
    let turn_event_id = push_event("turn.recorded", serde_json::to_value(input.turn_record)?)?;
    if let Some(finalize_step) = input.step_records.last_mut() {
        finalize_step.event_ids.push(step_ledger_event_id);
        finalize_step.event_ids.push(digest_event_id);
        finalize_step.event_ids.push(turn_event_id);
    }
    Ok(events)
}
