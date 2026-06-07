use crate::ProviderError;
use reqwest::Error as ReqwestError;
use reqwest::blocking::Client;
use std::collections::BTreeMap;
use std::net::{IpAddr, SocketAddr};
use std::time::Duration;

const REQUEST_TIMEOUT_SECS: u64 = 15 * 60;
const CONNECT_TIMEOUT_SECS: u64 = 15;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RequestFailure {
    pub message: String,
    pub retryable: bool,
}

pub fn build_client(resolve_overrides: &BTreeMap<String, IpAddr>) -> Result<Client, ProviderError> {
    let mut builder = Client::builder()
        .connect_timeout(Duration::from_secs(CONNECT_TIMEOUT_SECS))
        .timeout(Duration::from_secs(REQUEST_TIMEOUT_SECS));
    for (host, ip) in resolve_overrides {
        builder = builder.resolve(host, SocketAddr::new(*ip, 0));
    }
    builder.build().map_err(|err| ProviderError::Request {
        message: format!("client build failed: {}", summarize_error_chain(&err)),
    })
}

pub fn classify_reqwest_error(
    err: ReqwestError,
    stage: &str,
    endpoint: &str,
    attempt: usize,
    attempts: usize,
) -> RequestFailure {
    let retryable = err.is_timeout() || err.is_connect() || err.is_request() || err.is_body();
    let message = format!(
        "{stage} failed at attempt {attempt}/{attempts}; endpoint={endpoint}; timeout={}; connect={}; request={}; body={}; decode={}; source={}",
        err.is_timeout(),
        err.is_connect(),
        err.is_request(),
        err.is_body(),
        err.is_decode(),
        summarize_error_chain(&err),
    );
    RequestFailure { message, retryable }
}
pub fn classify_http_status(
    status: u16,
    body: &str,
    attempt: usize,
    attempts: usize,
) -> RequestFailure {
    let retryable = status == 429 || status == 503 || status >= 500;
    let body_lower = body.to_ascii_lowercase();
    let is_quota = body_lower.contains("quota")
        || body_lower.contains("usage limit")
        || body_lower.contains("usage_limit")
        || body_lower.contains("rate_limit")
        || body_lower.contains("rate limit")
        || body_lower.contains("usage limit exceeded")
        || body_lower.contains("weekly usage limit reached")
        || body.contains("余额不足")
        || body.contains("无可用资源包");
    let retryable = retryable || (is_quota && status == 400);
    let message = format!(
        "http status {status} at attempt {attempt}/{attempts}; retryable={retryable}; body_snippet={}",
        &body[..body.len().min(200)],
    );
    RequestFailure { message, retryable }
}

fn summarize_error_chain(err: &dyn std::error::Error) -> String {
    let mut parts = vec![err.to_string()];
    let mut current = err.source();
    while let Some(source) = current {
        parts.push(source.to_string());
        current = source.source();
    }
    parts.join(" | caused_by=")
}

#[cfg(test)]
mod tests {
    use super::*;
    use reqwest::blocking::Client;

    #[test]
    fn build_client_uses_blocking_client_builder() {
        let _client: Client = build_client(&BTreeMap::new()).expect("client should build");
    }

    #[test]
    fn classify_http_status_retries_rate_limit_and_quota_errors() {
        assert!(classify_http_status(429, "rate limit", 1, 5).retryable);
        assert!(classify_http_status(400, "weekly usage limit reached", 1, 5).retryable);
        assert!(classify_http_status(400, "余额不足或无可用资源包", 1, 5).retryable);
        assert!(!classify_http_status(400, "invalid request", 1, 5).retryable);
    }
}
