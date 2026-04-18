use fin_contracts::{ControlFeedback, ExecutionNote, ProgressBlock};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DebugBinding {
    pub project_id: String,
    pub project_label: String,
    pub runtime_home: String,
    pub session_id: Option<String>,
    pub task_id: Option<String>,
    pub session_messages_path: Option<String>,
    pub recent_contexts_path: Option<String>,
    pub recent_digests_path: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChatSendRequest {
    pub message: String,
    #[serde(default)]
    pub input_kind: Option<String>,
}

impl ChatSendRequest {
    pub fn is_status_probe(&self) -> bool {
        matches!(self.input_kind.as_deref(), Some("status_probe"))
            || self.message.trim_start().starts_with("/status")
    }

    pub fn normalized_message(&self) -> String {
        let trimmed = self.message.trim();
        if let Some(stripped) = trimmed.strip_prefix("/status") {
            let normalized = stripped.trim();
            if normalized.is_empty() {
                "current status".into()
            } else {
                normalized.into()
            }
        } else {
            trimmed.into()
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChatSendResponse {
    pub binding: DebugBinding,
    pub answer: String,
    pub digest_id: String,
    pub events_count: usize,
    #[serde(default)]
    pub response_kind: String,
    #[serde(default)]
    pub freshness: Option<String>,
    #[serde(default)]
    pub control_feedback: Option<ControlFeedback>,
    #[serde(default)]
    pub progress: Option<ProgressBlock>,
    #[serde(default)]
    pub note: Option<ExecutionNote>,
}
