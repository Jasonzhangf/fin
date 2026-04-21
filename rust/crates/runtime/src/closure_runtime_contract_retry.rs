use super::closure_runtime_rounds::{RoundExecution, execute_round};
use super::*;

pub(super) const MAX_OUTPUT_CONTRACT_RETRIES: usize = 3;

pub(super) struct RoundAttemptExecution {
    pub(super) attempt_index: u32,
    pub(super) round: RoundExecution,
    pub(super) validation_errors: Vec<String>,
}

pub(super) struct ContractRetriedRound {
    pub(super) final_round: RoundExecution,
    pub(super) attempts: Vec<RoundAttemptExecution>,
    pub(super) summary: ContractRetrySummary,
}

#[derive(Debug, Clone, Default)]
pub(super) struct ContractRetrySummary {
    pub(super) retry_count: usize,
    pub(super) limit_reached: bool,
    pub(super) remaining_errors: Vec<String>,
}

pub(super) fn execute_round_with_contract_retries(
    operation: &OperationEnvelope<InferenceOperationPayload>,
    provider: &impl InferenceProvider,
    refs: &EntityRefs,
    round_context: &MinimalContextView,
    round_index: u32,
    input: String,
) -> Result<ContractRetriedRound, RuntimeError> {
    let round = execute_round(
        operation,
        provider,
        refs,
        round_context,
        round_index,
        input.clone(),
    )?;
    let mut retry_events = Vec::new();
    let mut errors =
        validate_model_output_contract(&round.parsed_output, &round.assistant_response_text);
    let mut attempts = vec![RoundAttemptExecution {
        attempt_index: 1,
        round,
        validation_errors: errors.clone(),
    }];
    let mut retry_count = 0usize;

    while !errors.is_empty() && retry_count < MAX_OUTPUT_CONTRACT_RETRIES {
        retry_count += 1;
        retry_events.push((
            "model.output_contract_retry_requested".into(),
            serde_json::json!({
                "round_index": round_index,
                "retry_attempt": retry_count,
                "max_retries": MAX_OUTPUT_CONTRACT_RETRIES,
                "validation_errors": errors,
            }),
        ));
        let retry_input = build_contract_retry_input(
            input.as_str(),
            attempts
                .last()
                .expect("initial attempt must exist")
                .round
                .provider_response
                .output_text
                .as_str(),
            &errors,
            retry_count,
            MAX_OUTPUT_CONTRACT_RETRIES,
        );
        let round = execute_round(
            operation,
            provider,
            refs,
            round_context,
            round_index,
            retry_input.clone(),
        )?;
        errors =
            validate_model_output_contract(&round.parsed_output, &round.assistant_response_text);
        attempts.push(RoundAttemptExecution {
            attempt_index: attempts.len() as u32 + 1,
            round,
            validation_errors: errors.clone(),
        });
    }

    let summary = ContractRetrySummary {
        retry_count,
        limit_reached: !errors.is_empty(),
        remaining_errors: errors.clone(),
    };
    let final_attempt = attempts
        .last_mut()
        .expect("at least one round attempt must exist");
    append_contract_retry_summary(
        &mut final_attempt.round.dispatched_tools,
        round_index,
        &summary,
        retry_events,
    );
    Ok(ContractRetriedRound {
        final_round: final_attempt.round.clone(),
        attempts,
        summary,
    })
}

fn append_contract_retry_summary(
    outcome: &mut tool_dispatch::ToolDispatchOutcome,
    round_index: u32,
    summary: &ContractRetrySummary,
    mut retry_events: Vec<(String, Value)>,
) {
    if summary.retry_count == 0 {
        return;
    }
    outcome.events.append(&mut retry_events);
    if summary.limit_reached {
        outcome.note_hints.push(format!(
            "output contract retry limit reached after {} attempt(s)",
            summary.retry_count
        ));
        outcome.events.push((
            "model.output_contract_retry_limit_reached".into(),
            serde_json::json!({
                "round_index": round_index,
                "retry_count": summary.retry_count,
                "max_retries": MAX_OUTPUT_CONTRACT_RETRIES,
                "remaining_errors": summary.remaining_errors,
            }),
        ));
    } else {
        outcome.note_hints.push(format!(
            "output contract repaired after {} retry attempt(s)",
            summary.retry_count
        ));
        outcome.events.push((
            "model.output_contract_retry_succeeded".into(),
            serde_json::json!({
                "round_index": round_index,
                "retry_count": summary.retry_count,
            }),
        ));
    }
}

fn validate_model_output_contract(
    parsed: &ParsedModelOutput,
    assistant_response_text: &str,
) -> Vec<String> {
    let mut errors = Vec::new();
    if assistant_response_text.trim().is_empty() {
        errors.push("missing or empty <fin_user_response> content".into());
    }
    if parsed.tool_calls_block_present && parsed.tool_calls.is_empty() {
        errors.push(format!(
            "detected <fin_tool_calls> but it is not executable: status={} reason={}",
            parsed.tool_calls_parse_status,
            parsed
                .tool_calls_invalid_reason
                .as_deref()
                .unwrap_or("unknown")
        ));
    }
    errors
}

fn build_contract_retry_input(
    original_input: &str,
    previous_raw_output: &str,
    validation_errors: &[String],
    retry_attempt: usize,
    max_retries: usize,
) -> String {
    let errors = validation_errors
        .iter()
        .map(|item| format!("- {item}"))
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        "The previous output did not satisfy the fin structured output contract.\nKeep the same intent and semantics. Do not add new facts, new tool intentions, or new conclusions. Only repair the output shape.\nRetry attempt: {retry_attempt}/{max_retries}\nOriginal request:\n{original_input}\nValidation errors:\n{errors}\nPrevious raw output:\n{previous_raw_output}\nRe-emit the full response using the required fin blocks only."
    )
}
