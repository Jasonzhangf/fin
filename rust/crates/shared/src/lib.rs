use std::{env, path::PathBuf};
use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum SharedError {
    #[error("required value '{field}' is empty")]
    EmptyValue { field: &'static str },
}

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
}