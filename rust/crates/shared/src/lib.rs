use std::{env, path::PathBuf, time::Duration};

pub mod agents;
pub mod io;
use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum SharedError {
    #[error("required value '{field}' is empty")]
    EmptyValue { field: &'static str },
}

pub const DEFAULT_RETRY_ATTEMPTS: usize = 5;
pub const DEFAULT_RETRY_BASE_BACKOFF_SECS: u64 = 1;

pub fn require_non_empty(field: &'static str, value: &str) -> Result<(), SharedError> {
    if value.trim().is_empty() {
        return Err(SharedError::EmptyValue { field });
    }
    Ok(())
}

pub fn build_trace_id(scope: &str, ordinal: u64) -> String {
    format!("{scope}-{ordinal}")
}

pub fn expand_home_path(raw: &str) -> PathBuf {
    if raw == "~" {
        return env::var_os("HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(raw));
    }

    if let Some(stripped) = raw.strip_prefix("~/") {
        return env::var_os("HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("~"))
            .join(stripped);
    }

    PathBuf::from(raw)
}

pub fn exponential_backoff(attempt: usize) -> Duration {
    let exponent = attempt.saturating_sub(1).min(16) as u32;
    Duration::from_secs(
        DEFAULT_RETRY_BASE_BACKOFF_SECS.saturating_mul(2_u64.saturating_pow(exponent)),
    )
}

pub fn summarize_error_chain(err: &dyn std::error::Error) -> String {
    let mut parts = vec![err.to_string()];
    let mut current = err.source();
    while let Some(source) = current {
        parts.push(source.to_string());
        current = source.source();
    }
    parts.join(" | caused_by=")
}

pub use io::{SharedIoError, append_jsonl, read_json_or_empty, write_json};

pub use agents::{
    AgentIdentity, AgentKind, AgentRunRecord, CapabilityDescriptor, CloseAgentResult, ContextMode,
    ContextPolicy, RegisterPrimaryAgentInput, ResumeAgentResult, SpawnSubagentInput,
    WaitAgentResult, primary_path, validate_context_policy,
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_values_are_rejected() {
        let result = require_non_empty("source", "   ");
        assert_eq!(result, Err(SharedError::EmptyValue { field: "source" }));
    }

    #[test]
    fn trace_id_builder_is_stable() {
        assert_eq!(build_trace_id("dispatch", 7), "dispatch-7");
    }

    #[test]
    fn expands_tilde_prefixed_home_paths() {
        let expanded = expand_home_path("~/.fin");
        assert!(expanded.ends_with(".fin"));
        assert!(expanded.is_absolute());
    }

    #[test]
    fn exponential_backoff_starts_at_one_second() {
        assert_eq!(exponential_backoff(1), Duration::from_secs(1));
        assert_eq!(exponential_backoff(2), Duration::from_secs(2));
        assert_eq!(exponential_backoff(3), Duration::from_secs(4));
        assert_eq!(exponential_backoff(4), Duration::from_secs(8));
        assert_eq!(exponential_backoff(5), Duration::from_secs(16));
    }

    #[test]
    fn summarize_error_chain_keeps_all_sources() {
        let err = std::io::Error::other("outer");
        let summary = summarize_error_chain(&err);
        assert!(summary.contains("outer"));
    }
}
