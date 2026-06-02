use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ControlFeedback {
    #[serde(default)]
    pub origin: String,
    #[serde(default)]
    pub is_continuation: bool,
    #[serde(default)]
    pub is_simple_query: bool,
    #[serde(default)]
    pub candidate_task_id: Option<String>,
    #[serde(default)]
    pub candidate_topic_thread_id: Option<String>,
    #[serde(default)]
    pub continuity_confidence: u8,
    #[serde(default)]
    pub topic_shift_confidence: u8,
    #[serde(default)]
    pub simple_query_confidence: u8,
    #[serde(default)]
    pub previous_topic_summary: Option<String>,
    #[serde(default)]
    pub current_topic_summary: Option<String>,
    #[serde(default)]
    pub note_candidate: String,
    #[serde(default)]
    pub digest_candidate: String,
    #[serde(default)]
    pub reason: String,
    #[serde(default)]
    pub task_completed: bool,
    #[serde(default)]
    pub is_simple_chat: bool,
    #[serde(default)]
    pub blocked: bool,
    #[serde(default)]
    pub needs_user_involve: bool,
    #[serde(default)]
    pub completion_evidence: Vec<String>,
    #[serde(default)]
    pub final_conclusions: Vec<String>,
    #[serde(default)]
    pub blocked_reason: Option<String>,
    #[serde(default)]
    pub what_needs_to_be_done_by_user: Option<String>,
}
