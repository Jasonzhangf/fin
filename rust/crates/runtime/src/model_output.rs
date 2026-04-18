use fin_contracts::{ControlFeedback, InferenceOperationPayload};
use fin_provider::{PreparedRequest, ProviderResponse};
use serde::{Deserialize, Serialize};
use serde_json::Value;

const USER_RESPONSE_TAG: &str = "fin_user_response";
const CONTROL_FEEDBACK_TAG: &str = "fin_control_feedback";
const TOOL_CALLS_TAG: &str = "fin_tool_calls";
const CONTROL_FEEDBACK_KEYS: &[&str] = &[
    "origin",
    "is_continuation",
    "is_simple_query",
    "candidate_task_id",
    "candidate_topic_thread_id",
    "continuity_confidence",
    "topic_shift_confidence",
    "simple_query_confidence",
    "previous_topic_summary",
    "current_topic_summary",
    "note_candidate",
    "digest_candidate",
    "reason",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedModelOutput {
    pub user_response: String,
    pub control_feedback: Option<ControlFeedback>,
    pub control_feedback_salvaged: bool,
    pub contract_detected: bool,
    pub tool_calls: Vec<ModelToolCall>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelToolCall {
    pub tool_name: String,
    #[serde(default)]
    pub arguments: Value,
}

#[derive(Debug, Clone, Default)]
pub struct ModelOutputParser;

impl ModelOutputParser {
    pub fn parse(
        &self,
        payload: &InferenceOperationPayload,
        request: &PreparedRequest,
        response: &ProviderResponse,
    ) -> ParsedModelOutput {
        let raw = response.output_text.trim();
        let user_response = extract_tag(raw, USER_RESPONSE_TAG)
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| strip_structured_blocks(raw).trim().to_string());
        let (control_feedback, control_feedback_salvaged) = extract_tag(raw, CONTROL_FEEDBACK_TAG)
            .and_then(|block| parse_control_feedback(&block))
            .map(|result| {
                (
                    Some(normalize_feedback(
                        result.feedback,
                        payload,
                        request,
                        response,
                        result.salvaged,
                    )),
                    result.salvaged,
                )
            })
            .unwrap_or((None, false));
        let tool_calls = extract_tag(raw, TOOL_CALLS_TAG)
            .map(|block| parse_tool_calls(&block))
            .unwrap_or_default();

        ParsedModelOutput {
            user_response: if user_response.is_empty() {
                raw.to_string()
            } else {
                user_response
            },
            control_feedback,
            control_feedback_salvaged,
            contract_detected: raw.contains("<fin_user_response>")
                || raw.contains("<fin_control_feedback>")
                || raw.contains("<fin_tool_calls>"),
            tool_calls,
        }
    }
}

fn parse_tool_calls(raw: &str) -> Vec<ModelToolCall> {
    let Ok(value) = serde_json::from_str::<Value>(raw.trim()) else {
        return Vec::new();
    };
    match value {
        Value::Array(items) => items.into_iter().filter_map(normalize_tool_call).collect(),
        other => normalize_tool_call(other).into_iter().collect(),
    }
}

fn normalize_tool_call(value: Value) -> Option<ModelToolCall> {
    let object = value.as_object()?;
    let tool_name = object
        .get("tool_name")
        .or_else(|| object.get("name"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())?
        .to_string();
    let arguments = object
        .get("arguments")
        .or_else(|| object.get("args"))
        .cloned()
        .unwrap_or_else(|| Value::Object(Default::default()));
    Some(ModelToolCall {
        tool_name,
        arguments,
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ParsedControlFeedback {
    feedback: ControlFeedback,
    salvaged: bool,
}

fn parse_control_feedback(raw: &str) -> Option<ParsedControlFeedback> {
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

fn normalize_feedback(
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

fn extract_tag(raw: &str, tag: &str) -> Option<String> {
    let start_tag = format!("<{tag}>");
    let end_tag = format!("</{tag}>");
    let start = raw.find(&start_tag)?;
    let content_start = start + start_tag.len();
    let end = raw[content_start..].find(&end_tag)?;
    Some(raw[content_start..content_start + end].to_string())
}

fn strip_structured_blocks(raw: &str) -> String {
    let mut cleaned = raw.to_string();
    for tag in [CONTROL_FEEDBACK_TAG, TOOL_CALLS_TAG] {
        cleaned = remove_tag_block(&cleaned, tag);
    }
    cleaned
}

fn remove_tag_block(raw: &str, tag: &str) -> String {
    let start_tag = format!("<{tag}>");
    let end_tag = format!("</{tag}>");
    let Some(start) = raw.find(&start_tag) else {
        return raw.to_string();
    };
    let Some(end) = raw[start..].find(&end_tag) else {
        return raw.to_string();
    };
    let end = start + end + end_tag.len();
    let mut cleaned = String::new();
    cleaned.push_str(raw[..start].trim_end());
    if !cleaned.is_empty() && end < raw.len() {
        cleaned.push_str("\n\n");
    }
    cleaned.push_str(raw[end..].trim_start());
    cleaned
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
