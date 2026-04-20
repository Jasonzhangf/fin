use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolSemanticView {
    pub tool_call_id: String,
    pub operation_id: String,
    pub tool_name: String,
    pub category: String,
    pub verb: String,
    pub object_kind: String,
    pub object_label: String,
    pub summary: String,
    #[serde(default)]
    pub detail: Option<String>,
    pub status: String,
    pub started_at: String,
    #[serde(default)]
    pub ended_at: Option<String>,
    #[serde(default)]
    pub artifact_refs: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActivitySourceSummary {
    pub source_id: String,
    pub title: String,
    pub state: String,
    pub summary: String,
    pub visibility: String,
    pub auto_promoted: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceActivityCardView {
    pub source_id: String,
    pub source_kind: String,
    pub title: String,
    pub visibility: String,
    pub state: String,
    pub summary: String,
    #[serde(default)]
    pub focus_label: Option<String>,
    pub auto_promoted: bool,
    #[serde(default)]
    pub current_activity: Option<String>,
    #[serde(default)]
    pub recent_actions: Vec<ToolSemanticView>,
    #[serde(default)]
    pub waiting_detail: Option<String>,
    #[serde(default)]
    pub failure_detail: Option<String>,
    #[serde(default)]
    pub session_id: Option<String>,
    #[serde(default)]
    pub task_id: Option<String>,
    pub updated_at: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct UserActivityCardView {
    pub owner_source_id: String,
    pub header: String,
    pub state: String,
    #[serde(default)]
    pub focus_source_id: Option<String>,
    #[serde(default)]
    pub focus_summary: Option<String>,
    #[serde(default)]
    pub stage: Option<String>,
    #[serde(default)]
    pub recent_items: Vec<String>,
    #[serde(default)]
    pub active_sources: Vec<ActivitySourceSummary>,
    #[serde(default)]
    pub waiting_detail: Option<String>,
    #[serde(default)]
    pub failure_detail: Option<String>,
    pub updated_at: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActivityCardsSnapshot {
    #[serde(default)]
    pub session_id: Option<String>,
    #[serde(default)]
    pub task_id: Option<String>,
    pub generated_at: String,
    #[serde(default)]
    pub user_card: Option<UserActivityCardView>,
    #[serde(default)]
    pub source_cards: Vec<SourceActivityCardView>,
    #[serde(default)]
    pub tool_semantics: Vec<ToolSemanticView>,
}
