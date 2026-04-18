use crate::EntityRefs;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolExecutionRecord {
    pub tool_call_id: String,
    pub operation_id: String,
    pub trace_id: String,
    #[serde(flatten)]
    pub refs: EntityRefs,
    pub tool_name: String,
    pub tool_kind: String,
    pub title: String,
    pub purpose: String,
    #[serde(default)]
    pub target_kind: Option<String>,
    #[serde(default)]
    pub target_ref: Option<String>,
    #[serde(default)]
    pub input_summary: Option<String>,
    #[serde(default)]
    pub output_summary: Option<String>,
    pub status: String,
    pub started_at: String,
    #[serde(default)]
    pub ended_at: Option<String>,
    #[serde(default)]
    pub duration_ms: Option<u64>,
    #[serde(default)]
    pub side_effects: Vec<String>,
    #[serde(default)]
    pub artifact_refs: Vec<String>,
    #[serde(default)]
    pub error_summary: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReasoningViewRecord {
    pub reasoning_id: String,
    pub operation_id: String,
    pub trace_id: String,
    #[serde(flatten)]
    pub refs: EntityRefs,
    pub created_at: String,
    pub summary: String,
    #[serde(default)]
    pub decision_summary: Option<String>,
    #[serde(default)]
    pub continuity_summary: Option<String>,
    #[serde(default)]
    pub tool_intent_summary: Option<String>,
    #[serde(default)]
    pub risk_summary: Option<String>,
    #[serde(default)]
    pub next_step: Option<String>,
    #[serde(default)]
    pub source_refs: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClosureTraceRecord {
    pub closure_id: String,
    pub digest_id: String,
    pub operation_id: String,
    pub trace_id: String,
    #[serde(flatten)]
    pub refs: EntityRefs,
    pub created_at: String,
    pub user_input: String,
    pub assistant_response: String,
    pub rendered_input: String,
    pub provider_name: String,
    pub provider_model: String,
    pub provider_endpoint: String,
    pub provider_raw_output: String,
    pub context_snapshot_path: String,
    pub control_feedback_path: String,
    pub reasoning_view_path: String,
    #[serde(default)]
    pub tool_record_paths: Vec<String>,
    #[serde(default)]
    pub source_event_ids: Vec<String>,
}
