use super::*;

#[derive(Debug, Clone)]
pub(super) struct ClosureAccumulator {
    pub(super) compacted_history_records: Vec<CompactedHistoryRecord>,
    pub(super) provider_request_records: Vec<ProviderRequestRecord>,
    pub(super) provider_response_records: Vec<ProviderResponseRecord>,
    pub(super) round_records: Vec<RoundRecord>,
    pub(super) step_records: Vec<StepRecord>,
    pub(super) tool_records: Vec<ToolExecutionRecord>,
    pub(super) latest_round_tool_records: Vec<ToolExecutionRecord>,
    pub(super) contract_retry_summaries: Vec<closure_runtime_contract_retry::ContractRetrySummary>,
    pub(super) step_index: u32,
}

impl ClosureAccumulator {
    pub(super) fn new(
        operation: &OperationEnvelope<InferenceOperationPayload>,
        refs: &EntityRefs,
        turn_id: &str,
    ) -> Self {
        let mut step_index = 0u32;
        let context_build_step = closure_runtime_rounds::allocate_step(
            &mut step_index,
            &operation.operation_id,
            "context_build",
        );
        let step_records = vec![closure_runtime_rounds::build_context_build_step_record(
            context_build_step.step_id,
            context_build_step.step_index,
            operation,
            refs,
            turn_id,
        )];
        Self {
            compacted_history_records: Vec::new(),
            provider_request_records: Vec::new(),
            provider_response_records: Vec::new(),
            round_records: Vec::new(),
            step_records,
            tool_records: Vec::new(),
            latest_round_tool_records: Vec::new(),
            contract_retry_summaries: Vec::new(),
            step_index,
        }
    }

    pub(super) fn absorb_retry_bundle(
        &mut self,
        operation: &OperationEnvelope<InferenceOperationPayload>,
        refs: &EntityRefs,
        turn_id: &str,
        round_index: u32,
        submitted_at: &str,
        bundle: &closure_runtime_contract_retry::ContractRetriedRound,
    ) {
        self.compacted_history_records.extend(
            bundle
                .attempts
                .iter()
                .filter_map(|attempt| attempt.round.compacted_history.clone()),
        );
        self.contract_retry_summaries.push(bundle.summary.clone());
        for attempt in &bundle.attempts {
            self.tool_records.push(trace_records::provider_tool_record(
                &operation.operation_id,
                &operation.trace_id,
                refs,
                &attempt.round.prepared_request,
                &attempt.round.provider_response,
                attempt.round.assistant_response_text.as_str(),
                submitted_at,
            ));
            if attempt.attempt_index == bundle.attempts.len() as u32 {
                self.tool_records
                    .extend(attempt.round.dispatched_tools.tool_records.clone());
                self.latest_round_tool_records =
                    attempt.round.dispatched_tools.tool_records.clone();
            }
            closure_runtime_rounds::record_round(
                &mut self.step_index,
                operation,
                refs,
                turn_id,
                round_index,
                attempt.attempt_index,
                attempt.attempt_index == bundle.attempts.len() as u32,
                &attempt.round.prepared_request,
                &attempt.round.provider_response,
                &attempt.round.parsed_output,
                &attempt.round.control_feedback,
                &attempt.round.dispatched_tools,
                attempt.round.assistant_response_text.as_str(),
                &attempt.validation_errors,
                &mut self.provider_request_records,
                &mut self.provider_response_records,
                &mut self.round_records,
                &mut self.step_records,
            );
        }
    }
}
