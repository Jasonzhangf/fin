//! Anthropic wire payload/header builder.

use crate::blocks::errors::ProviderError;
use reqwest::header::{ACCEPT, CONTENT_TYPE, HeaderMap, HeaderName, HeaderValue, USER_AGENT};
use serde_json::Value;
use std::collections::BTreeMap;

pub const ANTHROPIC_MAX_OUTPUT_TOKENS: u64 = 2048;

pub fn build_payload(model: &str, rendered_input: &str) -> Value {
    serde_json::json!({
        "model": model,
        "max_tokens": ANTHROPIC_MAX_OUTPUT_TOKENS,
        "messages": [
            {
                "role": "user",
                "content": rendered_input,
            }
        ]
    })
}

pub fn build_headers(
    api_key: &str,
    user_agent: &str,
    custom_headers: &BTreeMap<String, String>,
) -> Result<HeaderMap, ProviderError> {
    let mut headers = build_custom_headers(custom_headers)?;
    headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
    headers.insert(ACCEPT, HeaderValue::from_static("application/json"));
    headers.insert(
        HeaderName::from_static("x-api-key"),
        HeaderValue::from_str(api_key).map_err(|err| ProviderError::InvalidHeader {
            name: "x-api-key".into(),
            message: err.to_string(),
        })?,
    );
    headers.insert(
        HeaderName::from_static("anthropic-version"),
        HeaderValue::from_static("2023-06-01"),
    );
    headers.insert(
        USER_AGENT,
        HeaderValue::from_str(user_agent).map_err(|err| ProviderError::InvalidHeader {
            name: "user-agent".into(),
            message: err.to_string(),
        })?,
    );
    Ok(headers)
}

pub fn build_sanitized_headers(
    user_agent: &str,
    custom_headers: &BTreeMap<String, String>,
) -> BTreeMap<String, String> {
    let mut headers = BTreeMap::new();
    for (name, value) in custom_headers {
        if is_reserved_runtime_header(name) {
            continue;
        }
        headers.insert(name.clone(), sanitize_header_value(name, value));
    }
    headers.insert("user-agent".into(), user_agent.into());
    headers.insert("accept".into(), "application/json".into());
    headers.insert("content-type".into(), "application/json".into());
    headers.insert("anthropic-version".into(), "2023-06-01".into());
    headers.insert("x-api-key".into(), "<redacted>".into());
    headers
}

fn build_custom_headers(
    custom_headers: &BTreeMap<String, String>,
) -> Result<HeaderMap, ProviderError> {
    let mut headers = HeaderMap::new();
    for (name, value) in custom_headers {
        let header_name = HeaderName::from_bytes(name.as_bytes()).map_err(|err| {
            ProviderError::InvalidHeader {
                name: name.clone(),
                message: err.to_string(),
            }
        })?;
        let header_value =
            HeaderValue::from_str(value).map_err(|err| ProviderError::InvalidHeader {
                name: name.clone(),
                message: err.to_string(),
            })?;
        headers.insert(header_name, header_value);
    }
    Ok(headers)
}

fn is_reserved_runtime_header(name: &str) -> bool {
    matches!(
        name.trim().to_ascii_lowercase().as_str(),
        "x-api-key" | "anthropic-version" | "content-type" | "accept" | "user-agent"
    )
}

fn sanitize_header_value(name: &str, value: &str) -> String {
    let name = name.trim().to_ascii_lowercase();
    if ["authorization", "x-api-key", "cookie"]
        .iter()
        .any(|candidate| name == *candidate)
        || ["token", "secret", "apikey", "api-key", "auth"]
            .iter()
            .any(|needle| name.contains(needle))
    {
        "<redacted>".into()
    } else {
        value.into()
    }
}
