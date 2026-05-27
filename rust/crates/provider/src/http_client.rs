use crate::ProviderError;
use fin_shared::summarize_error_chain;
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

#[cfg(test)]
mod tests {
    use super::*;
    use fin_shared::{
        DEFAULT_RETRY_ATTEMPTS, DEFAULT_RETRY_BASE_BACKOFF_SECS, exponential_backoff,
    };
    use reqwest::blocking::Client;

    #[test]
    fn build_client_uses_blocking_client_builder() {
        let _client: Client = build_client().expect("client should build");
    }

    #[test]
    fn exponential_backoff_starts_at_one_second() {
        assert_eq!(DEFAULT_RETRY_ATTEMPTS, 5);
        assert_eq!(DEFAULT_RETRY_BASE_BACKOFF_SECS, 1);
        assert_eq!(exponential_backoff(1), Duration::from_secs(1));
        assert_eq!(exponential_backoff(2), Duration::from_secs(2));
        assert_eq!(exponential_backoff(3), Duration::from_secs(4));
        assert_eq!(exponential_backoff(4), Duration::from_secs(8));
        assert_eq!(exponential_backoff(5), Duration::from_secs(16));
    }
}
