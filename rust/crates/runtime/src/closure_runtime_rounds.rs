use super::*;
use crate::tool_history_render::render_current_tool_execution_history;

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
    pub(super) compacted_history: Option<CompactedHistoryRecord>,
}

pub(super) fn execute_round(
    operation: &OperationEnvelope<InferenceOperationPayload>,
    provider: &impl InferenceProvider,
    refs: &EntityRefs,
    round_context: &MinimalContextView,
    round_index: u32,
    input: String,
) -> Result<RoundExecution, RuntimeError> {
    let assembler = ModelInputAssembler::default();
    let assembly_plan = assembler.assembly_plan(&input, round_context);
    let budget_decision = ContextBudgetManager::default().decide(&assembly_plan, None);
    // Prefix drift detection: if cached_ratio < 0.3 on a warm turn (round_index > 0),
    // the provider prefix may have drifted. Log a warning event.
    if round_index > 0 && budget_decision.cached_ratio < 0.3 && budget_decision.cached_ratio > 0.0 {
        eprintln!(
            "prefix_drift_detected: cached_ratio={:.2} round={}",
            budget_decision.cached_ratio, round_index
        );
    }
    let compacted_history =
        if budget_decision.decision == ContextCompactionDecisionKind::PreTurnCompact {
            Some(compact_round_history(
                operation,
                refs,
                round_context,
                &budget_decision,
            ))
        } else {
            None
        };
    let rendered_input =
        render_input_for_budget_decision(&assembly_plan, compacted_history.as_ref());
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
        prompt_cache_key: operation.refs.session_id.clone(),
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
        compacted_history,
    })
}

fn render_input_for_budget_decision(
    plan: &ContextAssemblyPlan,
    compacted_history: Option<&CompactedHistoryRecord>,
) -> String {
    let sections = plan
        .sections
        .iter()
        .map(|section| {
            let body = if section.section_id == "history.current_interaction_ledger" {
                compacted_history
                    .map(render_compacted_history_body)
                    .unwrap_or_else(|| section.body.clone())
            } else {
                section.body.clone()
            };
            format!("{}:\n{}", section.title, body)
        })
        .collect::<Vec<_>>();
    sections.join("\n\n")
}

fn render_compacted_history_body(record: &CompactedHistoryRecord) -> String {
    let mut lines = Vec::new();
    if !record.summary.trim().is_empty() {
        lines.push(format!("Compacted summary:\n{}", record.summary));
    }
    if !record.retained_messages.is_empty() {
        lines.push(format!(
            "Retained recent messages:\n- {}",
            record.retained_messages.join("\n- ")
        ));
    }
    if !record.retained_artifact_refs.is_empty() {
        lines.push(format!(
            "Retained artifact refs:\n- {}",
            record.retained_artifact_refs.join("\n- ")
        ));
    }
    lines.join("\n")
}

fn compact_round_history(
    operation: &OperationEnvelope<InferenceOperationPayload>,
    refs: &EntityRefs,
    round_context: &MinimalContextView,
    decision: &ContextBudgetDecision,
) -> CompactedHistoryRecord {
    let history = round_context.history.clone().unwrap_or_default();
    let digest_records = round_context
        .knowledge
        .as_ref()
        .map(|knowledge| fin_contracts::DigestRecord {
            digest_id: format!("digest-context-{}", operation.operation_id),
            closure_id: format!("closure-{}", operation.operation_id),
            refs: refs.clone(),
            summary: round_context.summary.clone().unwrap_or_default(),
            continuity_tail: round_context.continuity_tail.clone(),
            note_refs: Vec::new(),
            artifact_candidates: knowledge.artifact_candidates.clone(),
            control_feedback: None,
            created_at: operation.submitted_at.clone(),
        })
        .into_iter()
        .collect();
    ContextCompactionEngine.compact(CompactionInput {
        session_id: refs
            .session_id
            .clone()
            .unwrap_or_else(|| "session-m1".into()),
        task_id: refs.task_id.clone(),
        trigger_reason: decision.reason.clone(),
        recent_messages: history.recent_messages,
        digest_records,
        tool_records: Vec::new(),
        retain_recent_count: 8,
        compacted_at: operation.submitted_at.clone(),
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
    context: &MinimalContextView,
    original_input: &str,
    assistant_response: &str,
    tool_records: &[ToolExecutionRecord],
) -> String {
    let tool_lines = render_current_tool_execution_history(context, tool_records);
    format!(
        "Continue the same turn with the latest tool results.\nOriginal request: {original_input}\nLast assistant response: {assistant_response}\nExecuted tool results (authoritative client facts, full current history):\n- {}\nInspect these tool results before deciding whether another tool is needed. If the task is complete, answer directly and emit reasoning.stop.",
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
