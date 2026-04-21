use crate::model_output_shapes::{
    classify_invalid_tool_calls, extract_tag, partial_tool_signal_present, repair_json_shape,
    strip_json_code_fence, strip_structured_blocks,
};
use fin_contracts::{ControlFeedback, InferenceOperationPayload};
use fin_provider::{PreparedRequest, ProviderResponse};
use serde::{Deserialize, Serialize};
use serde_json::Value;

const USER_RESPONSE_TAG: &str = "fin_user_response";
const CONTROL_FEEDBACK_TAG: &str = "fin_control_feedback";
const TOOL_CALLS_TAG: &str = "fin_tool_calls";
const KNOWN_STRUCTURED_TAGS: &[&str] = &[USER_RESPONSE_TAG, CONTROL_FEEDBACK_TAG, TOOL_CALLS_TAG];
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
    pub tool_calls_block_present: bool,
    pub tool_calls_parse_status: String,
    pub tool_calls_invalid_reason: Option<String>,
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
        let user_response = extract_tag(raw, USER_RESPONSE_TAG, KNOWN_STRUCTURED_TAGS)
            .map(|value| value.content.trim().to_string())
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| {
                strip_structured_blocks(
                    raw,
                    &[CONTROL_FEEDBACK_TAG, TOOL_CALLS_TAG],
                    KNOWN_STRUCTURED_TAGS,
                )
                .trim()
                .to_string()
            });
        let (control_feedback, control_feedback_salvaged) =
            extract_tag(raw, CONTROL_FEEDBACK_TAG, KNOWN_STRUCTURED_TAGS)
                .and_then(|block| parse_control_feedback(&block.content))
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
        let parsed_tool_calls = extract_tag(raw, TOOL_CALLS_TAG, KNOWN_STRUCTURED_TAGS)
            .map(|block| parse_tool_calls(&block.content, block.repaired))
            .unwrap_or_else(ParsedToolCalls::absent);

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
            tool_calls_block_present: parsed_tool_calls.block_present,
            tool_calls_parse_status: parsed_tool_calls.parse_status,
            tool_calls_invalid_reason: parsed_tool_calls.invalid_reason,
            tool_calls: parsed_tool_calls.calls,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ParsedToolCalls {
    block_present: bool,
    parse_status: String,
    invalid_reason: Option<String>,
    calls: Vec<ModelToolCall>,
}

impl ParsedToolCalls {
    fn absent() -> Self {
        Self {
            block_present: false,
            parse_status: "absent".into(),
            invalid_reason: None,
            calls: Vec::new(),
        }
    }
}

fn parse_tool_calls(raw: &str, extraction_repaired: bool) -> ParsedToolCalls {
    let mut repaired = extraction_repaired;
    let mut working = raw.trim().to_string();
    let (without_fence, fence_repaired) = strip_json_code_fence(&working);
    working = without_fence;
    repaired |= fence_repaired;

    if let Ok(value) = serde_json::from_str::<Value>(&working) {
        return finalize_tool_calls(value, true, repaired, None);
    }

    if let Some(repaired_json) = repair_json_shape(&working) {
        repaired = true;
        if let Ok(value) = serde_json::from_str::<Value>(&repaired_json) {
            return finalize_tool_calls(value, true, repaired, None);
        }
    }

    let invalid_reason = classify_invalid_tool_calls(&working);
    ParsedToolCalls {
        block_present: true,
        parse_status: if partial_tool_signal_present(&working) {
            "masked_partial".into()
        } else {
            "invalid".into()
        },
        invalid_reason: Some(invalid_reason),
        calls: Vec::new(),
    }
}

fn finalize_tool_calls(
    value: Value,
    block_present: bool,
    repaired: bool,
    invalid_reason: Option<String>,
) -> ParsedToolCalls {
    match parse_tool_calls_value(value) {
        Ok((calls, normalized_repaired)) => ParsedToolCalls {
            block_present,
            parse_status: if repaired || normalized_repaired {
                "repaired_deterministic".into()
            } else {
                "exact".into()
            },
            invalid_reason,
            calls,
        },
        Err(reason) => ParsedToolCalls {
            block_present,
            parse_status: "invalid".into(),
            invalid_reason: Some(reason.into()),
            calls: Vec::new(),
        },
    }
}

fn parse_tool_calls_value(value: Value) -> Result<(Vec<ModelToolCall>, bool), &'static str> {
    let (items, repaired) = match value {
        Value::Array(items) => (items, false),
        other => (vec![other], true),
    };
    let mut normalized_repaired = repaired;
    let mut calls = Vec::with_capacity(items.len());
    for item in items {
        let normalized = normalize_tool_call(item)?;
        normalized_repaired |= normalized.repaired;
        calls.push(normalized.call);
    }
    Ok((calls, normalized_repaired))
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct NormalizedToolCall {
    call: ModelToolCall,
    repaired: bool,
}

fn normalize_tool_call(value: Value) -> Result<NormalizedToolCall, &'static str> {
    let object = value.as_object().ok_or("tool_call_not_object")?;
    let used_name_alias = object.contains_key("name") && !object.contains_key("tool_name");
    let tool_name = object
        .get("tool_name")
        .or_else(|| object.get("name"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or("missing_tool_name")?
        .to_string();
    let used_args_alias = object.contains_key("args") && !object.contains_key("arguments");
    let used_default_arguments = !object.contains_key("arguments") && !object.contains_key("args");
    let arguments = object
        .get("arguments")
        .or_else(|| object.get("args"))
        .cloned()
        .unwrap_or_else(|| Value::Object(Default::default()));
    Ok(NormalizedToolCall {
        call: ModelToolCall {
            tool_name,
            arguments,
        },
        repaired: used_name_alias || used_args_alias || used_default_arguments,
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

fn short_text(value: &str, limit: usize) -> String {
    if value.chars().count() <= limit {
        value.trim().to_string()
    } else {
        let mut result = value.chars().take(limit).collect::<String>();
        result.push('…');
        result
    }
}
