use super::*;
use crate::model_output::ModelToolCall;
use fin_contracts::ToolExecutionRecord;
use crate::control_feedback::ControlFeedbackBuilder;
use crate::model_output::ModelOutputParser;
use crate::tool_dispatch;

#[derive(Debug, Clone)]
pub(super) struct ReasonReq01Seed {
    pub(super) operation: OperationEnvelope<InferenceOperationPayload>,
    pub(super) refs: EntityRefs,
    pub(super) round_index: u32,
    pub(super) input: String,
    pub(super) context: MinimalContextView,
    pub(super) prior_tool_calls: Vec<ModelToolCall>,
    pub(super) tool_results: Vec<ToolExecutionRecord>,
}

#[derive(Debug, Clone)]
pub(super) struct ReasonReq02ContextPlan {
    pub(super) seed: ReasonReq01Seed,
    pub(super) rendered_input: String,
}

#[derive(Debug, Clone)]
pub(super) struct ReasonReq03BudgetedContext {
    pub(super) context_plan: ReasonReq02ContextPlan,
}

#[derive(Debug, Clone)]
pub(super) struct ReasonReq04RenderedInput {
    pub(super) budgeted_context: ReasonReq03BudgetedContext,
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
}

#[derive(Default)]
pub(super) struct ReasonReq01SeedBuilder;
impl ReasonReq01SeedBuilder {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn build(
        &self,
        operation: OperationEnvelope<InferenceOperationPayload>,
        refs: EntityRefs,
        round_index: u32,
        input: String,
        context: MinimalContextView,
        prior_tool_calls: Vec<ModelToolCall>,
        tool_results: Vec<ToolExecutionRecord>,
    ) -> ReasonReq01Seed {
        ReasonReq01Seed { operation, refs, round_index, input, context, prior_tool_calls, tool_results }
    }
}

#[derive(Default)]
pub(super) struct ReasonReq02ContextPlanBuilder;
impl ReasonReq02ContextPlanBuilder {
    pub(super) fn build(&self, seed: ReasonReq01Seed) -> ReasonReq02ContextPlan {
        let rendered_input = ModelInputAssembler::default().assemble(&seed.input, &seed.context);
        ReasonReq02ContextPlan { seed, rendered_input }
    }
}

#[derive(Default)]
pub(super) struct ReasonReq03BudgetedContextBuilder;
impl ReasonReq03BudgetedContextBuilder {
    pub(super) fn build(&self, context_plan: ReasonReq02ContextPlan) -> ReasonReq03BudgetedContext {
        ReasonReq03BudgetedContext { context_plan }
    }
}

#[derive(Default)]
pub(super) struct ReasonReq04RenderedInputBuilder;
impl ReasonReq04RenderedInputBuilder {
    pub(super) fn build(&self, budgeted: ReasonReq03BudgetedContext) -> ReasonReq04RenderedInput {
        ReasonReq04RenderedInput { budgeted_context: budgeted }
    }
}

#[derive(Default)]
pub(super) struct ReasonReq05ProviderCallBuilder;
impl ReasonReq05ProviderCallBuilder {
    pub(super) fn build(&self, rendered: ReasonReq04RenderedInput) -> ReasonReq05ProviderCall {
        let seed = &rendered.budgeted_context.context_plan.seed;
        let provider_request = ProviderRequest {
            input: seed.input.clone(),
            rendered_input: Some(rendered.budgeted_context.context_plan.rendered_input.clone()),
            override_model: Some(
                seed.operation.payload.provider_path.primary_target().model.clone(),
            ),
            tools: crate::closure_runtime_rounds_tools::build_provider_tool_specs(&seed.context),
            prior_tool_calls: seed.prior_tool_calls.iter().map(crate::closure_runtime_rounds_tools::model_tool_call_to_provider_tool_call).collect(),
            tool_results: crate::closure_runtime_rounds_tools::build_provider_tool_results(&seed.context, &seed.tool_results),
        };
        ReasonReq05ProviderCall { rendered_input: rendered, provider_request }
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
        Ok(ReasonResp06ModelOutput { provider_call, prepared_request, provider_response, provider_debug })
    }
}

#[derive(Default)]
pub(super) struct ReasonResp07ParsedContractParser;
impl ReasonResp07ParsedContractParser {
    pub(super) fn parse(&self, model_output: ReasonResp06ModelOutput) -> ReasonResp07ParsedContract {
        let parsed_output = ModelOutputParser::default().parse(
            &model_output.provider_call.rendered_input.budgeted_context.context_plan.seed.operation.payload,
            &model_output.prepared_request,
            &model_output.provider_response,
        );
        let assistant_response_text = parsed_output.user_response.clone();
        ReasonResp07ParsedContract { model_output, parsed_output, assistant_response_text }
    }
}

#[derive(Default)]
pub(super) struct ReasonResp08RuntimeDecisionBuilder;
impl ReasonResp08RuntimeDecisionBuilder {
    pub(super) fn build(&self, parsed_contract: ReasonResp07ParsedContract) -> ReasonResp08RuntimeDecision {
        let seed = &parsed_contract.model_output.provider_call.rendered_input.budgeted_context.context_plan.seed;
        let dispatched_tools = tool_dispatch::execute_model_tools(
            &seed.operation.operation_id,
            &seed.operation.trace_id,
            &seed.refs,
            &seed.operation.submitted_at,
            &seed.context,
            seed.round_index,
            &parsed_contract.parsed_output.tool_calls,
        );
        let runtime_default = ControlFeedbackBuilder.build(
            &seed.operation.payload,
            &parsed_contract.model_output.prepared_request,
            &parsed_contract.model_output.provider_response,
        );
        let mut control_feedback = ControlFeedbackBuilder.merge_with_runtime_observation(
            parsed_contract.parsed_output.control_feedback.clone(),
            runtime_default,
        );
        ControlFeedbackBuilder.rewrite_runtime_observation_candidates(
            &mut control_feedback,
            &parsed_contract.model_output.prepared_request,
            &parsed_contract.model_output.provider_response,
            parsed_contract.assistant_response_text.as_str(),
        );
        ReasonResp08RuntimeDecision { parsed_contract, dispatched_tools, control_feedback }
    }
}

#[derive(Default)]
pub(super) struct ReasonResp09ClosureBuilder;
impl ReasonResp09ClosureBuilder {
    pub(super) fn build(&self, decision: ReasonResp08RuntimeDecision) -> ReasonResp09Closure {
        let pc = decision.parsed_contract;
        let mo = pc.model_output;
        ReasonResp09Closure {
            prepared_request: mo.prepared_request,
            provider_response: mo.provider_response,
            provider_debug: mo.provider_debug,
            parsed_output: pc.parsed_output,
            dispatched_tools: decision.dispatched_tools,
            assistant_response_text: pc.assistant_response_text,
            control_feedback: decision.control_feedback,
        }
    }
}

pub(super) fn reason_pipeline(
    operation: OperationEnvelope<InferenceOperationPayload>,
    refs: EntityRefs,
    provider: &impl InferenceProvider,
    round_index: u32,
    input: String,
    context: MinimalContextView,
    prior_tool_calls: Vec<ModelToolCall>,
    tool_results: Vec<ToolExecutionRecord>,
) -> Result<ReasonResp09Closure, RuntimeError> {
    let seed = ReasonReq01SeedBuilder.build(operation, refs, round_index, input, context, prior_tool_calls, tool_results);
    let plan = ReasonReq02ContextPlanBuilder.build(seed);
    let budgeted = ReasonReq03BudgetedContextBuilder.build(plan);
    let rendered = ReasonReq04RenderedInputBuilder.build(budgeted);
    let call = ReasonReq05ProviderCallBuilder.build(rendered);
    let out = ReasonResp06ModelOutputParser::build_parser().parse(call, provider)?;
    let contract = ReasonResp07ParsedContractParser.parse(out);
    let decision = ReasonResp08RuntimeDecisionBuilder.build(contract);
    Ok(ReasonResp09ClosureBuilder.build(decision))
}

impl ReasonResp06ModelOutputParser {
    fn build_parser() -> Self { Self }
}
