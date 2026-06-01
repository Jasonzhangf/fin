use super::*;

pub(crate) fn compact_round_history(
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

#[derive(Debug, Clone)]
pub(super) struct ReasonReq01Seed {
    pub(super) operation: OperationEnvelope<InferenceOperationPayload>,
    pub(super) refs: EntityRefs,
    pub(super) round_index: u32,
    pub(super) input: String,
    pub(super) context: MinimalContextView,
}

#[derive(Debug, Clone)]
pub(super) struct ReasonReq02ContextPlan {
    pub(super) seed: ReasonReq01Seed,
    pub(super) assembly_plan: ContextAssemblyPlan,
}

#[derive(Debug, Clone)]
pub(super) struct ReasonReq03BudgetedContext {
    pub(super) context_plan: ReasonReq02ContextPlan,
    pub(super) budget_decision: ContextBudgetDecision,
    pub(super) compacted_history: Option<CompactedHistoryRecord>,
}

#[derive(Debug, Clone)]
pub(super) struct ReasonReq04RenderedInput {
    pub(super) budgeted_context: ReasonReq03BudgetedContext,
    pub(super) rendered_input: String,
}

#[derive(Debug, Clone)]
pub(super) struct ReasonReq05ProviderCall {
    pub(super) rendered_input: ReasonReq04RenderedInput,
    pub(super) provider_request: ProviderRequest,
}

#[derive(Debug, Clone)]
pub(super) struct ReasonResp06ModelOutput {
    pub(super) provider_call: ReasonReq05ProviderCall,
    pub(super) prepared_request: PreparedRequest,
    pub(super) provider_response: ProviderResponse,
    pub(super) provider_debug: SanitizedProviderDebug,
}

#[derive(Debug, Clone)]
pub(super) struct ReasonResp07ParsedContract {
    pub(super) model_output: ReasonResp06ModelOutput,
    pub(super) parsed_output: ParsedModelOutput,
    pub(super) assistant_response_text: String,
}

#[derive(Debug, Clone)]
pub(super) struct ReasonResp08RuntimeDecision {
    pub(super) parsed_contract: ReasonResp07ParsedContract,
    pub(super) dispatched_tools: tool_dispatch::ToolDispatchOutcome,
    pub(super) control_feedback: ControlFeedback,
}

#[derive(Debug, Clone)]
pub(super) struct ReasonResp09Closure {
    pub(super) prepared_request: PreparedRequest,
    pub(super) provider_response: ProviderResponse,
    pub(super) provider_debug: SanitizedProviderDebug,
    pub(super) parsed_output: ParsedModelOutput,
    pub(super) dispatched_tools: tool_dispatch::ToolDispatchOutcome,
    pub(super) assistant_response_text: String,
    pub(super) control_feedback: ControlFeedback,
    pub(super) compacted_history: Option<CompactedHistoryRecord>,
}

#[derive(Default)]
pub(super) struct ReasonReq02ContextPlanBuilder;

impl ReasonReq02ContextPlanBuilder {
    pub(super) fn build(&self, seed: ReasonReq01Seed) -> ReasonReq02ContextPlan {
        let assembly_plan =
            ModelInputAssembler::default().assembly_plan(&seed.input, &seed.context);
        ReasonReq02ContextPlan {
            seed,
            assembly_plan,
        }
    }
}

#[derive(Default)]
pub(super) struct ReasonReq03BudgetedContextBuilder;

impl ReasonReq03BudgetedContextBuilder {
    pub(super) fn build(&self, context_plan: ReasonReq02ContextPlan) -> ReasonReq03BudgetedContext {
        let budget_decision =
            ContextBudgetManager::default().decide(&context_plan.assembly_plan, None);
        if context_plan.seed.round_index > 0
            && budget_decision.cached_ratio < 0.3
            && budget_decision.cached_ratio > 0.0
        {
            eprintln!(
                "prefix_drift_detected: cached_ratio={:.2} round={}",
                budget_decision.cached_ratio, context_plan.seed.round_index
            );
        }
        let compacted_history =
            if budget_decision.decision == ContextCompactionDecisionKind::PreTurnCompact {
                Some(compact_round_history(
                    &context_plan.seed.operation,
                    &context_plan.seed.refs,
                    &context_plan.seed.context,
                    &budget_decision,
                ))
            } else {
                None
            };
        ReasonReq03BudgetedContext {
            context_plan,
            budget_decision,
            compacted_history,
        }
    }
}

#[derive(Default)]
pub(super) struct ReasonReq04RenderedInputBuilder;

impl ReasonReq04RenderedInputBuilder {
    pub(super) fn build(
        &self,
        budgeted_context: ReasonReq03BudgetedContext,
    ) -> ReasonReq04RenderedInput {
        let _ = &budgeted_context.budget_decision;
        let rendered_input = render_input_for_budget_decision(
            &budgeted_context.context_plan.assembly_plan,
            budgeted_context.compacted_history.as_ref(),
        );
        ReasonReq04RenderedInput {
            budgeted_context,
            rendered_input,
        }
    }
}

#[derive(Default)]
pub(super) struct ReasonReq05ProviderCallBuilder;

impl ReasonReq05ProviderCallBuilder {
    pub(super) fn build(
        &self,
        rendered_input: ReasonReq04RenderedInput,
    ) -> ReasonReq05ProviderCall {
        let provider_request = ProviderRequest {
            input: rendered_input
                .budgeted_context
                .context_plan
                .seed
                .input
                .clone(),
            rendered_input: Some(rendered_input.rendered_input.clone()),
            override_model: Some(
                rendered_input
                    .budgeted_context
                    .context_plan
                    .seed
                    .operation
                    .payload
                    .provider_path
                    .primary_target()
                    .model
                    .clone(),
            ),
            prompt_cache_key: rendered_input
                .budgeted_context
                .context_plan
                .seed
                .operation
                .refs
                .session_id
                .clone(),
        };
        ReasonReq05ProviderCall {
            rendered_input,
            provider_request,
        }
    }
}

#[derive(Default)]
pub(super) struct ReasonResp06ModelOutputParser;

impl ReasonResp06ModelOutputParser {
    pub(super) fn parse(
        &self,
        provider_call: ReasonReq05ProviderCall,
        provider: &impl InferenceProvider,
    ) -> Result<ReasonResp06ModelOutput, RuntimeError> {
        let prepared_request = provider.prepare_request(&provider_call.provider_request);
        let provider_response = provider.execute_prepared(&prepared_request)?;
        let provider_debug = SanitizedProviderDebug {
            user_agent: prepared_request.user_agent.clone(),
            request_headers: prepared_request.sanitized_headers.clone(),
        };
        Ok(ReasonResp06ModelOutput {
            provider_call,
            prepared_request,
            provider_response,
            provider_debug,
        })
    }
}

#[derive(Default)]
pub(super) struct ReasonResp07ParsedContractParser;

impl ReasonResp07ParsedContractParser {
    pub(super) fn parse(
        &self,
        model_output: ReasonResp06ModelOutput,
    ) -> ReasonResp07ParsedContract {
        let parsed_output = ModelOutputParser::default().parse(
            &model_output
                .provider_call
                .rendered_input
                .budgeted_context
                .context_plan
                .seed
                .operation
                .payload,
            &model_output.prepared_request,
            &model_output.provider_response,
        );
        let assistant_response_text = parsed_output.user_response.clone();
        ReasonResp07ParsedContract {
            model_output,
            parsed_output,
            assistant_response_text,
        }
    }
}

#[derive(Default)]
pub(super) struct ReasonResp08RuntimeDecisionBuilder;

impl ReasonResp08RuntimeDecisionBuilder {
    pub(super) fn build(
        &self,
        parsed_contract: ReasonResp07ParsedContract,
    ) -> ReasonResp08RuntimeDecision {
        let seed = &parsed_contract
            .model_output
            .provider_call
            .rendered_input
            .budgeted_context
            .context_plan
            .seed;
        let dispatched_tools = tool_dispatch::execute_model_tools(
            &seed.operation.operation_id,
            &seed.operation.trace_id,
            &seed.refs,
            &seed.operation.submitted_at,
            &seed.context,
            seed.round_index,
            &parsed_contract.parsed_output.tool_calls,
        );
        let runtime_default_feedback = ControlFeedbackBuilder.build(
            &seed.operation.payload,
            &parsed_contract.model_output.prepared_request,
            &parsed_contract.model_output.provider_response,
        );
        let mut control_feedback = ControlFeedbackBuilder::default().merge_with_runtime_defaults(
            parsed_contract.parsed_output.control_feedback.clone(),
            runtime_default_feedback,
        );
        ControlFeedbackBuilder::default().rewrite_runtime_heuristic_candidates(
            &mut control_feedback,
            &parsed_contract.model_output.prepared_request,
            &parsed_contract.model_output.provider_response,
            parsed_contract.assistant_response_text.as_str(),
        );
        ReasonResp08RuntimeDecision {
            parsed_contract,
            dispatched_tools,
            control_feedback,
        }
    }
}

#[derive(Default)]
pub(super) struct ReasonResp09ClosureBuilder;

impl ReasonResp09ClosureBuilder {
    pub(super) fn build(
        &self,
        runtime_decision: ReasonResp08RuntimeDecision,
    ) -> ReasonResp09Closure {
        let compacted_history = runtime_decision
            .parsed_contract
            .model_output
            .provider_call
            .rendered_input
            .budgeted_context
            .compacted_history
            .clone();
        ReasonResp09Closure {
            prepared_request: runtime_decision
                .parsed_contract
                .model_output
                .prepared_request,
            provider_response: runtime_decision
                .parsed_contract
                .model_output
                .provider_response,
            provider_debug: runtime_decision.parsed_contract.model_output.provider_debug,
            parsed_output: runtime_decision.parsed_contract.parsed_output,
            dispatched_tools: runtime_decision.dispatched_tools,
            assistant_response_text: runtime_decision.parsed_contract.assistant_response_text,
            control_feedback: runtime_decision.control_feedback,
            compacted_history,
        }
    }
}

pub(super) fn render_input_for_budget_decision(
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
    if !record.retained_tool_refs.is_empty() {
        lines.push(format!(
            "Retained tool refs:\n- {}",
            record.retained_tool_refs.join("\n- ")
        ));
    }
    lines.join("\n")
}
