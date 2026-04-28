use super::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ParsedControlFeedback {
    pub(super) feedback: ControlFeedback,
    pub(super) salvaged: bool,
}

pub(super) fn parse_control_feedback(raw: &str) -> Option<ParsedControlFeedback> {
    let value = serde_json::from_str::<Value>(raw.trim()).ok()?;
    let object = value.as_object()?;
    if !object
        .keys()
        .any(|key| CONTROL_FEEDBACK_KEYS.iter().any(|expected| key == expected))
    {
        return None;
    }
    if let Ok(feedback) = serde_json::from_value::<ControlFeedback>(value.clone()) {
        return Some(ParsedControlFeedback {
            feedback,
            salvaged: false,
        });
    }
    salvage_control_feedback(object).map(|feedback| ParsedControlFeedback {
        feedback,
        salvaged: true,
    })
}

pub(super) fn normalize_feedback(
    mut feedback: ControlFeedback,
    payload: &InferenceOperationPayload,
    request: &PreparedRequest,
    response: &ProviderResponse,
    salvaged: bool,
) -> ControlFeedback {
    feedback.origin = if feedback.origin.trim().is_empty() {
        if salvaged {
            "model_output_contract_masked".into()
        } else {
            "model_output_contract_v1".into()
        }
    } else {
        feedback.origin
    };
    feedback.continuity_confidence = feedback.continuity_confidence.min(100);
    feedback.topic_shift_confidence = feedback.topic_shift_confidence.min(100);
    feedback.simple_query_confidence = feedback.simple_query_confidence.min(100);
    if feedback.candidate_task_id.is_none() {
        feedback.candidate_task_id = payload
            .context
            .control
            .as_ref()
            .and_then(|control| control.task_id.clone());
    }
    if feedback.candidate_topic_thread_id.is_none() {
        feedback.candidate_topic_thread_id = payload
            .context
            .control
            .as_ref()
            .and_then(|control| control.topic_thread_id.clone());
    }
    if feedback.current_topic_summary.is_none() {
        feedback.current_topic_summary = Some(short_text(&payload.input, 72));
    }
    if feedback.note_candidate.trim().is_empty() {
        feedback.note_candidate = format!(
            "provider {}:{} answered current turn with stop_reason={}",
            request.provider_name,
            request.model,
            response.stop_reason.as_deref().unwrap_or("unknown")
        );
    }
    if feedback.digest_candidate.trim().is_empty() {
        feedback.digest_candidate = format!(
            "closure on {}:{} produced answer {}",
            request.provider_name, request.model, response.output_text
        );
    }
    if feedback.reason.trim().is_empty() {
        feedback.reason = "parsed from model output contract".into();
    }
    feedback
}

fn salvage_control_feedback(object: &serde_json::Map<String, Value>) -> Option<ControlFeedback> {
    let mut feedback = ControlFeedback::default();
    let mut matched = 0usize;

    if let Some(value) = object.get("origin").and_then(mask_string) {
        feedback.origin = value;
        matched += 1;
    }
    if let Some(value) = object.get("is_continuation").and_then(mask_bool) {
        feedback.is_continuation = value;
        matched += 1;
    }
    if let Some(value) = object.get("is_simple_query").and_then(mask_bool) {
        feedback.is_simple_query = value;
        matched += 1;
    }
    if let Some(value) = object.get("task_completed").and_then(mask_bool) {
        feedback.task_completed = value;
        matched += 1;
    }
    if let Some(value) = object.get("is_simple_chat").and_then(mask_bool) {
        feedback.is_simple_chat = value;
        matched += 1;
    }
    if let Some(value) = object.get("blocked").and_then(mask_bool) {
        feedback.blocked = value;
        matched += 1;
    }
    if let Some(value) = object.get("needs_user_involve").and_then(mask_bool) {
        feedback.needs_user_involve = value;
        matched += 1;
    }
    if let Some(value) = object.get("candidate_task_id").and_then(mask_string) {
        feedback.candidate_task_id = Some(value);
        matched += 1;
    }
    if let Some(value) = object
        .get("candidate_topic_thread_id")
        .and_then(mask_string)
    {
        feedback.candidate_topic_thread_id = Some(value);
        matched += 1;
    }
    if let Some(value) = object
        .get("continuity_confidence")
        .and_then(mask_confidence)
    {
        feedback.continuity_confidence = value;
        matched += 1;
    }
    if let Some(value) = object
        .get("topic_shift_confidence")
        .and_then(mask_confidence)
    {
        feedback.topic_shift_confidence = value;
        matched += 1;
    }
    if let Some(value) = object
        .get("simple_query_confidence")
        .and_then(mask_confidence)
    {
        feedback.simple_query_confidence = value;
        matched += 1;
    }
    if let Some(value) = object.get("previous_topic_summary").and_then(mask_string) {
        feedback.previous_topic_summary = Some(value);
        matched += 1;
    }
    if let Some(value) = object.get("current_topic_summary").and_then(mask_string) {
        feedback.current_topic_summary = Some(value);
        matched += 1;
    }
    if let Some(values) = object
        .get("completion_evidence")
        .and_then(mask_string_array)
    {
        feedback.completion_evidence = values;
        matched += 1;
    }
    if let Some(values) = object.get("final_conclusions").and_then(mask_string_array) {
        feedback.final_conclusions = values;
        matched += 1;
    }
    if let Some(value) = object.get("blocked_reason").and_then(mask_string) {
        feedback.blocked_reason = Some(value);
        matched += 1;
    }
    if let Some(value) = object
        .get("what_needs_to_be_done_by_user")
        .and_then(mask_string)
    {
        feedback.what_needs_to_be_done_by_user = Some(value);
        matched += 1;
    }
    if let Some(value) = object.get("note_candidate").and_then(mask_string) {
        feedback.note_candidate = value;
        matched += 1;
    }
    if let Some(value) = object.get("digest_candidate").and_then(mask_string) {
        feedback.digest_candidate = value;
        matched += 1;
    }
    if let Some(value) = object.get("reason").and_then(mask_string) {
        feedback.reason = value;
        matched += 1;
    }

    (matched > 0).then_some(feedback)
}

fn mask_string(value: &Value) -> Option<String> {
    match value {
        Value::String(text) => {
            let trimmed = text.trim();
            (!trimmed.is_empty()).then(|| trimmed.to_string())
        }
        Value::Number(number) => Some(number.to_string()),
        Value::Bool(boolean) => Some(boolean.to_string()),
        _ => None,
    }
}

fn mask_bool(value: &Value) -> Option<bool> {
    match value {
        Value::Bool(boolean) => Some(*boolean),
        Value::String(text) => match text.trim().to_ascii_lowercase().as_str() {
            "true" | "1" | "yes" => Some(true),
            "false" | "0" | "no" => Some(false),
            _ => None,
        },
        Value::Number(number) => number.as_f64().map(|raw| raw != 0.0),
        _ => None,
    }
}

fn mask_string_array(value: &Value) -> Option<Vec<String>> {
    let Value::Array(items) = value else {
        return None;
    };
    let values = items.iter().filter_map(mask_string).collect::<Vec<_>>();
    (!values.is_empty()).then_some(values)
}

fn mask_confidence(value: &Value) -> Option<u8> {
    let raw = match value {
        Value::Number(number) => number.as_f64(),
        Value::String(text) => text.trim().parse::<f64>().ok(),
        _ => None,
    }?;
    let normalized = if (0.0..=1.0).contains(&raw) {
        (raw * 100.0).round()
    } else {
        raw.round()
    };
    Some(normalized.clamp(0.0, 100.0) as u8)
}

fn short_text(value: &str, limit: usize) -> String {
    if value.chars().count() <= limit {
        value.trim().to_string()
    } else {
        let mut result = value.chars().take(limit).collect::<String>();
        result.push('…');
        result
    }
}
