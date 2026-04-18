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
        let trimmed_input = payload.input.trim();
        let word_count = trimmed_input.split_whitespace().count();
        let punctuation_count = trimmed_input
            .chars()
            .filter(|ch| matches!(ch, '?' | '？' | '!' | '！'))
            .count();
        let has_history = payload
            .context
            .history
            .as_ref()
            .map(|history| !history.recent_messages.is_empty())
            .unwrap_or(false);
        let has_task = payload
            .context
            .control
            .as_ref()
            .and_then(|control| control.task_id.as_ref())
            .is_some();
        let is_simple_query = word_count <= 12 && punctuation_count <= 1 && !has_history;
        let continuity_confidence = if has_task { 92 } else { 58 };
        let topic_shift_confidence = if has_history { 18 } else { 36 };
        let simple_query_confidence = if is_simple_query { 88 } else { 24 };

        ControlFeedback {
            origin: "runtime_heuristic".into(),
            is_continuation: has_task,
            is_simple_query,
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
            continuity_confidence,
            topic_shift_confidence,
            simple_query_confidence,
            previous_topic_summary: payload.context.summary.clone(),
            current_topic_summary: Some(short_topic_summary(trimmed_input)),
            note_candidate: format!(
                "provider {}:{} answered current turn with stop_reason={}",
                request.provider_name,
                request.model,
                response.stop_reason.as_deref().unwrap_or("unknown")
            ),
            digest_candidate: format!(
                "closure on {}:{} kept task continuity={} and produced answer {}",
                request.provider_name,
                request.model,
                has_task,
                response.output_text
            ),
            reason: if is_simple_query {
                "short single-turn request without prior history".into()
            } else if has_task {
                "existing task binding and recent context suggest continuity".into()
            } else {
                "no explicit task binding; keep observing continuity in later turns".into()
            },
        }
    }

    pub fn merge_with_fallback(
        &self,
        parsed: Option<ControlFeedback>,
        fallback: ControlFeedback,
    ) -> ControlFeedback {
        let Some(parsed) = parsed else {
            return fallback;
        };
        ControlFeedback {
            origin: parsed.origin,
            is_continuation: parsed.is_continuation,
            is_simple_query: parsed.is_simple_query,
            candidate_task_id: parsed.candidate_task_id.or(fallback.candidate_task_id),
            candidate_topic_thread_id: parsed
                .candidate_topic_thread_id
                .or(fallback.candidate_topic_thread_id),
            continuity_confidence: if parsed.continuity_confidence == 0 {
                fallback.continuity_confidence
            } else {
                parsed.continuity_confidence
            },
            topic_shift_confidence: if parsed.topic_shift_confidence == 0 {
                fallback.topic_shift_confidence
            } else {
                parsed.topic_shift_confidence
            },
            simple_query_confidence: if parsed.simple_query_confidence == 0 {
                fallback.simple_query_confidence
            } else {
                parsed.simple_query_confidence
            },
            previous_topic_summary: parsed
                .previous_topic_summary
                .or(fallback.previous_topic_summary),
            current_topic_summary: parsed.current_topic_summary.or(fallback.current_topic_summary),
            note_candidate: if parsed.note_candidate.trim().is_empty() {
                fallback.note_candidate
            } else {
                parsed.note_candidate
            },
            digest_candidate: if parsed.digest_candidate.trim().is_empty() {
                fallback.digest_candidate
            } else {
                parsed.digest_candidate
            },
            reason: if parsed.reason.trim().is_empty() {
                fallback.reason
            } else {
                parsed.reason
            },
        }
    }

    pub fn rewrite_runtime_heuristic_candidates(
        &self,
        feedback: &mut ControlFeedback,
        request: &PreparedRequest,
        response: &ProviderResponse,
        assistant_response_text: &str,
    ) {
        if feedback.origin != "runtime_heuristic" {
            return;
        }
        feedback.note_candidate = format!(
            "provider {}:{} answered current turn with stop_reason={}",
            request.provider_name,
            request.model,
            response.stop_reason.as_deref().unwrap_or("unknown")
        );
        feedback.digest_candidate = format!(
            "closure on {}:{} kept task continuity={} and produced answer {}",
            request.provider_name,
            request.model,
            feedback.is_continuation,
            assistant_response_text
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
