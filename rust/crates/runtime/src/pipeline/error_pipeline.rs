use crate::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ErrorErr01Detected {
    pub source_node: String,
    pub fact: String,
    pub captured_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ErrorErr02SourceClassified {
    pub detected: ErrorErr01Detected,
    pub source_class: ErrSourceClass,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ErrSourceClass {
    Input,
    Provider,
    Model,
    Tool,
    Runtime,
    Channel,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ErrorErr03RuntimeClassified {
    pub classified: ErrorErr02SourceClassified,
    pub decision: ErrRuntimeDecision,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ErrRuntimeDecision {
    Retryable { reason: String },
    Blocked { reason: String },
    Failed { reason: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ErrorErr04SessionRecorded {
    pub classified: ErrorErr03RuntimeClassified,
    pub recorded_event_id: String,
    pub ledger_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ErrorErr05UserVisible {
    pub recorded: ErrorErr04SessionRecorded,
    pub user_message: String,
    pub safe_for_channel: bool,
}

#[derive(Debug, Default)]
pub struct ErrorErr01DetectedBuilder;

impl ErrorErr01DetectedBuilder {
    pub fn build(
        &self,
        source_node: impl Into<String>,
        fact: impl Into<String>,
        captured_at: impl Into<String>,
    ) -> Result<ErrorErr01Detected, RuntimeError> {
        let source_node = source_node.into();
        let fact = fact.into();
        fin_shared::require_non_empty("source_node", &source_node)?;
        fin_shared::require_non_empty("fact", &fact)?;
        Ok(ErrorErr01Detected {
            source_node,
            fact,
            captured_at: captured_at.into(),
        })
    }
}

#[derive(Debug, Default)]
pub struct ErrorErr02SourceClassifiedBuilder;

impl ErrorErr02SourceClassifiedBuilder {
    pub fn build(
        &self,
        detected: ErrorErr01Detected,
        source_class: ErrSourceClass,
    ) -> ErrorErr02SourceClassified {
        ErrorErr02SourceClassified {
            detected,
            source_class,
        }
    }
}

#[derive(Debug, Default)]
pub struct ErrorErr03RuntimeClassifiedBuilder;

impl ErrorErr03RuntimeClassifiedBuilder {
    pub fn build(
        &self,
        classified: ErrorErr02SourceClassified,
        decision: ErrRuntimeDecision,
    ) -> ErrorErr03RuntimeClassified {
        ErrorErr03RuntimeClassified {
            classified,
            decision,
        }
    }
}

#[derive(Debug, Default)]
pub struct ErrorErr04SessionRecordedBuilder;

impl ErrorErr04SessionRecordedBuilder {
    pub fn build(
        &self,
        classified: ErrorErr03RuntimeClassified,
        recorded_event_id: impl Into<String>,
        ledger_path: impl Into<String>,
    ) -> Result<ErrorErr04SessionRecorded, RuntimeError> {
        let recorded_event_id = recorded_event_id.into();
        let ledger_path = ledger_path.into();
        fin_shared::require_non_empty("recorded_event_id", &recorded_event_id)?;
        fin_shared::require_non_empty("ledger_path", &ledger_path)?;
        Ok(ErrorErr04SessionRecorded {
            classified,
            recorded_event_id,
            ledger_path,
        })
    }
}

#[derive(Debug, Default)]
pub struct ErrorErr05UserVisibleBuilder;

impl ErrorErr05UserVisibleBuilder {
    pub fn build(
        &self,
        recorded: ErrorErr04SessionRecorded,
        user_message: impl Into<String>,
    ) -> Result<ErrorErr05UserVisible, RuntimeError> {
        let user_message = user_message.into();
        fin_shared::require_non_empty("user_message", &user_message)?;
        let safe_for_channel = !user_message.trim().is_empty();
        Ok(ErrorErr05UserVisible {
            recorded,
            user_message,
            safe_for_channel,
        })
    }
}

pub fn classify_runtime_error(
    runtime_error: &RuntimeError,
) -> (ErrSourceClass, ErrRuntimeDecision) {
    let source = match runtime_error {
        RuntimeError::Config(_) => ErrSourceClass::Input,
        RuntimeError::InvalidOperation(_) => ErrSourceClass::Input,
        RuntimeError::Io { .. } => ErrSourceClass::Runtime,
        RuntimeError::Provider(_) => ErrSourceClass::Provider,
        RuntimeError::Serialize(_) => ErrSourceClass::Runtime,
        RuntimeError::State(_) => ErrSourceClass::Runtime,
    };
    let decision = ErrRuntimeDecision::Failed {
        reason: runtime_error.to_string(),
    };
    (source, decision)
}
