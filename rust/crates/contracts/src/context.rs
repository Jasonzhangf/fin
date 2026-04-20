use crate::ProviderStrategy;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextControlBlock {
    pub session_id: Option<String>,
    pub task_id: Option<String>,
    pub topic_thread_id: Option<String>,
    pub dispatch_id: Option<String>,
    pub worker_id: Option<String>,
    pub operation_id: Option<String>,
    pub trace_id: Option<String>,
    pub protocol_version: Option<String>,
    pub provider_strategy: Option<ProviderStrategy>,
    pub stream: Option<bool>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RolePromptBlock {
    #[serde(default)]
    pub role_id: String,
    #[serde(default, alias = "prompt_summary")]
    pub current_prompt_summary: String,
    #[serde(default)]
    pub prompt_history: Vec<String>,
    #[serde(default)]
    pub prompt_lineage: Vec<String>,
    #[serde(default)]
    pub prompt_modules: Vec<PromptModuleEntry>,
    #[serde(default)]
    pub prompt_layers: Vec<PromptLayerSummary>,
    #[serde(default)]
    pub behavior_rules: Vec<String>,
    #[serde(default)]
    pub output_contract: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PromptModuleEntry {
    #[serde(default)]
    pub module_id: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub summary: String,
    #[serde(default)]
    pub source: String,
    #[serde(default)]
    pub priority: u8,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PromptLayerSummary {
    #[serde(default)]
    pub layer_id: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub summary: String,
    #[serde(default)]
    pub source: String,
    #[serde(default)]
    pub module_ids: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectRef {
    #[serde(default)]
    pub project_id: String,
    #[serde(default)]
    pub label: String,
    #[serde(default)]
    pub root: Option<String>,
    #[serde(default)]
    pub state: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolCatalogEntry {
    #[serde(default)]
    pub tool_name: String,
    #[serde(default)]
    pub kind: String,
    #[serde(default)]
    pub summary: String,
    #[serde(default)]
    pub purpose: String,
    #[serde(default)]
    pub when_to_use: Vec<String>,
    #[serde(default)]
    pub when_not_to_use: Vec<String>,
    #[serde(default)]
    pub input_schema_summary: String,
    #[serde(default)]
    pub output_schema_summary: String,
    #[serde(default)]
    pub side_effects: Vec<String>,
    #[serde(default)]
    pub example_uses: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolCatalogBlock {
    #[serde(default)]
    pub model_tools: Vec<ToolCatalogEntry>,
    #[serde(default)]
    pub framework_tools: Vec<ToolCatalogEntry>,
    #[serde(default)]
    pub tool_selection_policy: Vec<String>,
    #[serde(default)]
    pub disabled_tools: Vec<String>,
    #[serde(default)]
    pub hard_guards: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct HistoryBlock {
    #[serde(default)]
    pub recent_messages: Vec<String>,
    #[serde(default)]
    pub recent_digests: Vec<String>,
    #[serde(default)]
    pub recent_reasoning: Vec<String>,
    #[serde(default)]
    pub recent_tool_activity: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct KnowledgeArtifactBlock {
    #[serde(default)]
    pub digest_summaries: Vec<String>,
    #[serde(default)]
    pub artifact_candidates: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectContextBlock {
    #[serde(default)]
    pub primary_project: Option<ProjectRef>,
    #[serde(default)]
    pub active_projects: Vec<ProjectRef>,
    #[serde(default)]
    pub projects: Vec<ProjectRef>,
    #[serde(default)]
    pub project_label: Option<String>,
    #[serde(default)]
    pub project_root: Option<String>,
    #[serde(default)]
    pub runtime_home: Option<String>,
    #[serde(default)]
    pub cwd: Option<String>,
    #[serde(default)]
    pub selected_paths: Vec<String>,
    #[serde(default)]
    pub relative_selected_paths: Vec<String>,
    #[serde(default)]
    pub scope_summary: Option<String>,
    #[serde(default)]
    pub focus_summary: Option<String>,
    #[serde(default)]
    pub active_task_id: Option<String>,
    #[serde(default)]
    pub task_board_summary: Option<String>,
    #[serde(default)]
    pub known_task_ids: Vec<String>,
    #[serde(default)]
    pub active_agent_ids: Vec<String>,
    #[serde(default)]
    pub agent_presence_summary: Option<String>,
    #[serde(default)]
    pub supervision_actions: Vec<String>,
    #[serde(default)]
    pub project_supervision_summary: Option<String>,
    #[serde(default)]
    pub assignment_queue_summary: Option<String>,
    #[serde(default)]
    pub mailbox_summary: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PeerDescriptorSummary {
    #[serde(default)]
    pub peer_id: String,
    #[serde(default)]
    pub label: String,
    #[serde(default)]
    pub peer_kind: String,
    #[serde(default)]
    pub presence_state: String,
    #[serde(default)]
    pub health_state: Option<String>,
    #[serde(default)]
    pub capability_ids: Vec<String>,
    #[serde(default)]
    pub supports_session_binding: bool,
    #[serde(default)]
    pub supports_agentic_execution: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PeerBindingSummary {
    #[serde(default)]
    pub owner_peer_id: Option<String>,
    #[serde(default)]
    pub bound_peer_id: Option<String>,
    #[serde(default)]
    pub binding_scope: Option<String>,
    #[serde(default)]
    pub binding_state: Option<String>,
    #[serde(default)]
    pub lease_ttl_ms: Option<u64>,
    #[serde(default)]
    pub rebind_hint: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DaemonStateSummary {
    #[serde(default)]
    pub daemon_id: Option<String>,
    #[serde(default)]
    pub supervision_state: Option<String>,
    #[serde(default)]
    pub status_summary: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PeerContextBlock {
    #[serde(default)]
    pub topology_summary: Option<String>,
    #[serde(default)]
    pub active_peer_ids: Vec<String>,
    #[serde(default)]
    pub peers: Vec<PeerDescriptorSummary>,
    #[serde(default)]
    pub binding: Option<PeerBindingSummary>,
    #[serde(default)]
    pub daemon: Option<DaemonStateSummary>,
    #[serde(default)]
    pub routing_hints: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct InputAttachmentSummary {
    #[serde(default)]
    pub attachment_id: Option<String>,
    #[serde(default)]
    pub kind: String,
    #[serde(default)]
    pub content_type: Option<String>,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub local_path: Option<String>,
    #[serde(default)]
    pub size_bytes: Option<u64>,
    #[serde(default)]
    pub width: Option<u32>,
    #[serde(default)]
    pub height: Option<u32>,
    #[serde(default)]
    pub source: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CurrentInputBlock {
    pub input: String,
    pub source: String,
    pub operation_id: String,
    pub trace_id: String,
    #[serde(default)]
    pub attachments: Vec<InputAttachmentSummary>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MinimalContextView {
    #[serde(default)]
    pub continuity_tail: Vec<String>,
    pub summary: Option<String>,
    pub control: Option<ContextControlBlock>,
    pub role_prompt: Option<RolePromptBlock>,
    pub tools: Option<ToolCatalogBlock>,
    pub history: Option<HistoryBlock>,
    pub knowledge: Option<KnowledgeArtifactBlock>,
    pub project: Option<ProjectContextBlock>,
    #[serde(default)]
    pub peer: Option<PeerContextBlock>,
    pub current_input: Option<CurrentInputBlock>,
}
