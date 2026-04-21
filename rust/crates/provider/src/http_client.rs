use crate::ProviderError;
use reqwest::Error as ReqwestError;
use reqwest::blocking::Client;
use std::time::Duration;

const REQUEST_TIMEOUT_SECS: u64 = 15 * 60;
const CONNECT_TIMEOUT_SECS: u64 = 15;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RequestFailure {
    pub message: String,
    pub retryable: bool,
}

pub fn build_client() -> Result<Client, ProviderError> {
    Client::builder()
        .connect_timeout(Duration::from_secs(CONNECT_TIMEOUT_SECS))
        .timeout(Duration::from_secs(REQUEST_TIMEOUT_SECS))
        .build()
        .map_err(|err| ProviderError::Request {
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
        let _client: Client = build_client().expect("client should build");
    }
}
