//! Provider response block and protocol payload parsing.

use crate::blocks::errors::ProviderError;
use crate::blocks::request::{PreparedRequest, TokenUsage};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderResponse {
    pub provider_name: String,
    pub model: String,
    pub output_text: String,
    pub response_id: Option<String>,
    pub stop_reason: Option<String>,
    pub status: u16,
    #[serde(default)]
    pub usage: Option<TokenUsage>,
}

pub fn parse_anthropic_response(
    request: &PreparedRequest,
    status: u16,
    body: &str,
) -> Result<ProviderResponse, ProviderError> {
    let parsed: Value = serde_json::from_str(body).map_err(|err| ProviderError::ParseResponse {
        message: err.to_string(),
    })?;
    let output_text = parsed
        .get("content")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| {
                    item.get("type")
                        .and_then(Value::as_str)
                        .filter(|kind| *kind == "text")
                        .and_then(|_| item.get("text"))
                        .and_then(Value::as_str)
                })
                .collect::<String>()
        })
        .unwrap_or_default();

    Ok(ProviderResponse {
        provider_name: request.provider_name.clone(),
        model: request.model.clone(),
        output_text,
        response_id: parsed.get("id").and_then(Value::as_str).map(str::to_string),
        stop_reason: parsed
            .get("stop_reason")
            .and_then(Value::as_str)
            .map(str::to_string),
        status,
        usage: parse_anthropic_usage(&parsed),
    })
}

fn parse_anthropic_usage(parsed: &Value) -> Option<TokenUsage> {
    let usage = parsed.get("usage")?;
    Some(TokenUsage {
        prompt_tokens: usage.get("input_tokens").and_then(Value::as_u64),
        completion_tokens: usage.get("output_tokens").and_then(Value::as_u64),
        total_tokens: match (
            usage.get("input_tokens").and_then(Value::as_u64),
            usage.get("output_tokens").and_then(Value::as_u64),
        ) {
            (Some(input), Some(output)) => Some(input + output),
            _ => usage.get("total_tokens").and_then(Value::as_u64),
        },
        cached_tokens: usage.get("cache_read_input_tokens").and_then(Value::as_u64),
        reasoning_tokens: usage
            .get("completion_tokens_details")
            .and_then(|value| value.get("reasoning_tokens"))
            .and_then(Value::as_u64),
        usage_source: "provider_anthropic".into(),
    })
}

pub fn parse_openai_response(
    request: &PreparedRequest,
    status: u16,
    body: &str,
) -> Result<ProviderResponse, ProviderError> {
    let parsed: Value = serde_json::from_str(body).map_err(|err| ProviderError::ParseResponse {
        message: err.to_string(),
    })?;
    ensure_openai_success_payload(status, &parsed)?;
    let first_choice = parsed
        .get("choices")
        .and_then(Value::as_array)
        .and_then(|choices| choices.first());
    let output_text = first_choice
        .and_then(|choice| choice.get("message"))
        .and_then(|message| message.get("content"))
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    Ok(ProviderResponse {
        provider_name: request.provider_name.clone(),
        model: request.model.clone(),
        output_text,
        response_id: parsed.get("id").and_then(Value::as_str).map(str::to_string),
        stop_reason: first_choice
            .and_then(|choice| choice.get("finish_reason"))
            .and_then(Value::as_str)
            .map(str::to_string),
        status,
        usage: parse_openai_usage(&parsed),
    })
}

fn ensure_openai_success_payload(status: u16, parsed: &Value) -> Result<(), ProviderError> {
    if let Some(error) = parsed.get("error") {
        let message = error
            .get("message")
            .and_then(Value::as_str)
            .or_else(|| error.as_str())
            .unwrap_or("openai-compatible provider returned error payload");
        return Err(ProviderError::HttpStatus {
            status,
            body: message.to_string(),
        });
    }

    let has_choices = parsed
        .get("choices")
        .and_then(Value::as_array)
        .is_some_and(|choices| !choices.is_empty());
    if has_choices {
        return Ok(());
    }

    if let Some(base_resp) = parsed.get("base_resp") {
        let status_code = base_resp
            .get("status_code")
            .and_then(Value::as_u64)
            .unwrap_or(status as u64) as u16;
        let status_msg = base_resp
            .get("status_msg")
            .and_then(Value::as_str)
            .unwrap_or("openai-compatible provider returned empty choices");
        return Err(ProviderError::HttpStatus {
            status: status_code,
            body: status_msg.to_string(),
        });
    }

    Err(ProviderError::ParseResponse {
        message: "openai-compatible provider response missing choices".into(),
    })
}

fn parse_openai_usage(parsed: &Value) -> Option<TokenUsage> {
    let usage = parsed.get("usage")?;
    Some(TokenUsage {
        prompt_tokens: usage.get("prompt_tokens").and_then(Value::as_u64),
        completion_tokens: usage.get("completion_tokens").and_then(Value::as_u64),
        total_tokens: usage.get("total_tokens").and_then(Value::as_u64),
        cached_tokens: usage
            .get("prompt_tokens_details")
            .and_then(|value| value.get("cached_tokens"))
            .and_then(Value::as_u64),
        reasoning_tokens: usage
            .get("completion_tokens_details")
            .and_then(|value| value.get("reasoning_tokens"))
            .and_then(Value::as_u64),
        usage_source: "provider_openai_compatible".into(),
    })
}
