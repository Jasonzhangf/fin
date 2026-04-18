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
}
