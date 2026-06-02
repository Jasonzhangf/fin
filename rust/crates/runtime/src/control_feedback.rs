use fin_contracts::{ControlFeedback, InferenceOperationPayload};
use fin_provider::{PreparedRequest, ProviderResponse};

#[derive(Debug, Clone, Default)]
pub struct ControlFeedbackBuilder;

impl ControlFeedbackBuilder {
    pub fn build(
        &self,
        payload: &InferenceOperationPayload,
        request: &PreparedRequest,
        response: &ProviderResponse,
    ) -> ControlFeedback {
        ControlFeedback {
            origin: "runtime_observation_only_v1".into(),
            is_continuation: false,
            is_simple_query: false,
            task_completed: false,
            is_simple_chat: false,
            blocked: false,
            needs_user_involve: false,
            candidate_task_id: payload
                .context
                .control
                .as_ref()
                .and_then(|control| control.task_id.clone()),
            candidate_topic_thread_id: payload
                .context
                .control
                .as_ref()
                .and_then(|control| control.topic_thread_id.clone()),
            continuity_confidence: 0,
            topic_shift_confidence: 0,
            simple_query_confidence: 0,
            previous_topic_summary: payload.context.summary.clone(),
            current_topic_summary: Some(short_topic_summary(payload.input.trim())),
            completion_evidence: Vec::new(),
            final_conclusions: Vec::new(),
            blocked_reason: None,
            what_needs_to_be_done_by_user: None,
            note_candidate: format!(
                "provider {}:{} answered current turn with stop_reason={}",
                request.provider_name,
                request.model,
                response.stop_reason.as_deref().unwrap_or("unknown")
            ),
            digest_candidate: format!(
                "closure on {}:{} produced answer {}",
                request.provider_name, request.model, response.output_text
            ),
            reason: "model control feedback missing or invalid; runtime recorded observation-only control without semantic routing intent".into(),
        }
    }

    pub fn merge_with_runtime_observation(
        &self,
        parsed: Option<ControlFeedback>,
        runtime_observation: ControlFeedback,
    ) -> ControlFeedback {
        let Some(parsed) = parsed else {
            return runtime_observation;
        };
        ControlFeedback {
            origin: parsed.origin,
            is_continuation: parsed.is_continuation,
            is_simple_query: parsed.is_simple_query,
            task_completed: parsed.task_completed,
            is_simple_chat: parsed.is_simple_chat,
            blocked: parsed.blocked,
            needs_user_involve: parsed.needs_user_involve,
            candidate_task_id: parsed
                .candidate_task_id
                .or(runtime_observation.candidate_task_id),
            candidate_topic_thread_id: parsed
                .candidate_topic_thread_id
                .or(runtime_observation.candidate_topic_thread_id),
            continuity_confidence: if parsed.continuity_confidence == 0 {
                runtime_observation.continuity_confidence
            } else {
                parsed.continuity_confidence
            },
            topic_shift_confidence: if parsed.topic_shift_confidence == 0 {
                runtime_observation.topic_shift_confidence
            } else {
                parsed.topic_shift_confidence
            },
            simple_query_confidence: if parsed.simple_query_confidence == 0 {
                runtime_observation.simple_query_confidence
            } else {
                parsed.simple_query_confidence
            },
            previous_topic_summary: parsed
                .previous_topic_summary
                .or(runtime_observation.previous_topic_summary),
            current_topic_summary: parsed
                .current_topic_summary
                .or(runtime_observation.current_topic_summary),
            completion_evidence: if parsed.completion_evidence.is_empty() {
                runtime_observation.completion_evidence
            } else {
                parsed.completion_evidence
            },
            final_conclusions: if parsed.final_conclusions.is_empty() {
                runtime_observation.final_conclusions
            } else {
                parsed.final_conclusions
            },
            blocked_reason: parsed
                .blocked_reason
                .or(runtime_observation.blocked_reason),
            what_needs_to_be_done_by_user: parsed
                .what_needs_to_be_done_by_user
                .or(runtime_observation.what_needs_to_be_done_by_user),
            note_candidate: if parsed.note_candidate.trim().is_empty() {
                runtime_observation.note_candidate
            } else {
                parsed.note_candidate
            },
            digest_candidate: if parsed.digest_candidate.trim().is_empty() {
                runtime_observation.digest_candidate
            } else {
                parsed.digest_candidate
            },
            reason: if parsed.reason.trim().is_empty() {
                runtime_observation.reason
            } else {
                parsed.reason
            },
        }
    }

    pub fn rewrite_runtime_observation_candidates(
        &self,
        feedback: &mut ControlFeedback,
        request: &PreparedRequest,
        response: &ProviderResponse,
        assistant_response_text: &str,
    ) {
        if feedback.origin != "runtime_observation_only_v1" {
            return;
        }
        feedback.note_candidate = format!(
            "provider {}:{} answered current turn with stop_reason={}",
            request.provider_name,
            request.model,
            response.stop_reason.as_deref().unwrap_or("unknown")
        );
        feedback.digest_candidate = format!(
            "closure on {}:{} produced answer {}",
            request.provider_name, request.model, assistant_response_text
        );
    }
}

fn short_topic_summary(input: &str) -> String {
    let compact = input.split_whitespace().collect::<Vec<_>>().join(" ");
    if compact.chars().count() <= 72 {
        compact
    } else {
        let mut summary = compact.chars().take(72).collect::<String>();
        summary.push('…');
        summary
    }
}
