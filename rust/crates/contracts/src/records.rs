use crate::{EntityRefs, InputAttachmentSummary};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

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

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderRequestRecord {
    pub request_id: String,
    pub turn_id: String,
    pub step_id: String,
    pub round_index: u32,
    pub operation_id: String,
    pub trace_id: String,
    #[serde(flatten)]
    pub refs: EntityRefs,
    pub provider_name: String,
    pub model: String,
    pub endpoint: String,
    pub input: String,
    pub rendered_input: String,
    #[serde(default)]
    pub user_agent: Option<String>,
    #[serde(default)]
    pub sanitized_headers: BTreeMap<String, String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderResponseRecord {
    pub response_record_id: String,
    pub request_id: String,
    pub turn_id: String,
    pub step_id: String,
    pub round_index: u32,
    pub operation_id: String,
    pub trace_id: String,
    #[serde(flatten)]
    pub refs: EntityRefs,
    pub status: u16,
    pub response_id: Option<String>,
    pub stop_reason: Option<String>,
    pub output_text: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RoundRecord {
    pub round_id: String,
    pub turn_id: String,
    pub operation_id: String,
    pub trace_id: String,
    pub round_index: u32,
    pub created_at: String,
    #[serde(flatten)]
    pub refs: EntityRefs,
    pub provider_step_id: String,
    pub model_parse_step_id: String,
    pub control_feedback_step_id: String,
    pub tool_dispatch_step_id: String,
    pub request_id: String,
    pub response_record_id: String,
    pub contract_detected: bool,
    pub control_feedback_parsed: bool,
    pub control_feedback_salvaged: bool,
    pub tool_calls_count: usize,
    pub assistant_response_summary: String,
    pub control_feedback_origin: String,
    pub stop_requested: bool,
    pub yield_requested: bool,
    pub reminder_scheduled: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct StepRecord {
    pub step_id: String,
    pub turn_id: String,
    pub operation_id: String,
    pub trace_id: String,
    pub step_index: u32,
    pub step_kind: String,
    pub status: String,
    pub started_at: String,
    #[serde(default)]
    pub ended_at: Option<String>,
    #[serde(flatten)]
    pub refs: EntityRefs,
    pub summary: String,
    #[serde(default)]
    pub input_ref: Option<String>,
    #[serde(default)]
    pub output_ref: Option<String>,
    #[serde(default)]
    pub event_ids: Vec<String>,
    #[serde(default)]
    pub progress_ref: Option<String>,
    #[serde(default)]
    pub note_refs: Vec<String>,
    #[serde(default)]
    pub blocked_by_step_id: Option<String>,
    #[serde(default)]
    pub next_step_hint: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TurnRecord {
    pub turn_id: String,
    pub closure_id: String,
    pub operation_id: String,
    pub trace_id: String,
    pub turn_index: Option<u64>,
    pub status: String,
    pub created_at: String,
    #[serde(default)]
    pub completed_at: Option<String>,
    #[serde(flatten)]
    pub refs: EntityRefs,
    pub user_input: String,
    #[serde(default)]
    pub assistant_visible_output: Option<String>,
    #[serde(default)]
    pub progress_summary: Option<String>,
    #[serde(default)]
    pub control_feedback_ref: Option<String>,
    #[serde(default)]
    pub execution_note_refs: Vec<String>,
    #[serde(default)]
    pub context_snapshot_ref: Option<String>,
    #[serde(default)]
    pub reasoning_view_refs: Vec<String>,
    #[serde(default)]
    pub tool_record_refs: Vec<String>,
    #[serde(default)]
    pub provider_request_ref: Option<String>,
    #[serde(default)]
    pub provider_response_ref: Option<String>,
    #[serde(default)]
    pub closure_trace_ref: Option<String>,
    #[serde(default)]
    pub digest_ref: Option<String>,
    #[serde(default)]
    pub step_ids: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RoutingDecisionRecord {
    pub decision_id: String,
    pub operation_id: String,
    pub trace_id: String,
    #[serde(flatten)]
    pub refs: EntityRefs,
    pub created_at: String,
    pub disposition: String,
    pub requires_user_confirmation: bool,
    #[serde(default)]
    pub candidate_task_id: Option<String>,
    #[serde(default)]
    pub candidate_topic_thread_id: Option<String>,
    pub continuity_confidence: u8,
    pub topic_shift_confidence: u8,
    pub simple_query_confidence: u8,
    #[serde(default)]
    pub previous_topic_summary: Option<String>,
    #[serde(default)]
    pub current_topic_summary: Option<String>,
    pub reason: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RoutingActionRecord {
    pub action_id: String,
    pub decision_id: String,
    pub operation_id: String,
    pub trace_id: String,
    #[serde(flatten)]
    pub refs: EntityRefs,
    pub created_at: String,
    pub action_kind: String,
    pub source_disposition: String,
    pub apply_immediately: bool,
    pub prompt_user: bool,
    #[serde(default)]
    pub prompt_text: Option<String>,
    #[serde(default)]
    pub suggested_task_id: Option<String>,
    #[serde(default)]
    pub suggested_topic_thread_id: Option<String>,
    pub confidence: u8,
    pub reason: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SchedulerDecisionRecord {
    pub decision_id: String,
    pub created_at: String,
    #[serde(flatten)]
    pub refs: EntityRefs,
    pub state_status: String,
    pub pending_input_count: usize,
    #[serde(default)]
    pub latest_routing_action_kind: Option<String>,
    pub action_kind: String,
    pub continue_until_blocked: bool,
    #[serde(default)]
    pub blocked_by: Option<String>,
    pub reason: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SchedulerTickRecord {
    pub tick_id: String,
    pub created_at: String,
    #[serde(default)]
    pub completed_at: Option<String>,
    #[serde(flatten)]
    pub refs: EntityRefs,
    pub source: String,
    pub status: String,
    pub decisions_recorded: usize,
    pub drove_count: usize,
    pub initial_pending_input_count: usize,
    pub final_pending_input_count: usize,
    #[serde(default)]
    pub final_decision_id: Option<String>,
    #[serde(default)]
    pub final_action_kind: Option<String>,
    #[serde(default)]
    pub blocked_by: Option<String>,
    #[serde(default)]
    pub last_response_kind: Option<String>,
    pub result_summary: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SupervisorCycleRecord {
    pub cycle_id: String,
    pub created_at: String,
    #[serde(default)]
    pub completed_at: Option<String>,
    #[serde(flatten)]
    pub refs: EntityRefs,
    pub source: String,
    pub status: String,
    #[serde(default)]
    pub tick_id: Option<String>,
    pub tick_count: usize,
    pub drove_count: usize,
    pub pending_input_count_before: usize,
    pub pending_input_count_after: usize,
    #[serde(default)]
    pub final_tick_status: Option<String>,
    #[serde(default)]
    pub final_action_kind: Option<String>,
    #[serde(default)]
    pub blocked_by: Option<String>,
    #[serde(default)]
    pub blocked_kind: Option<String>,
    #[serde(default)]
    pub next_wake_hint: Option<String>,
    #[serde(default)]
    pub next_check_at: Option<String>,
    #[serde(default)]
    pub heartbeat_interval_ms: Option<u64>,
    #[serde(default)]
    pub lease_ttl_ms: Option<u64>,
    pub result_summary: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SupervisorHeartbeatRecord {
    pub heartbeat_id: String,
    pub created_at: String,
    #[serde(flatten)]
    pub refs: EntityRefs,
    pub source: String,
    pub status: String,
    #[serde(default)]
    pub observed_cycle_id: Option<String>,
    #[serde(default)]
    pub triggered_cycle_id: Option<String>,
    pub due_for_tick: bool,
    pub stale_lease: bool,
    #[serde(default)]
    pub blocked_kind: Option<String>,
    #[serde(default)]
    pub next_check_at: Option<String>,
    #[serde(default)]
    pub lease_deadline_at: Option<String>,
    pub result_summary: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionStateRecord {
    pub state_id: String,
    #[serde(flatten)]
    pub refs: EntityRefs,
    pub status: String,
    #[serde(default)]
    pub active_turn_id: Option<String>,
    #[serde(default)]
    pub active_step_id: Option<String>,
    #[serde(default)]
    pub resume_from_step_id: Option<String>,
    pub pending_input_count: usize,
    pub accepts_user_input: bool,
    #[serde(default)]
    pub reason: Option<String>,
    pub updated_at: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PendingInputRecord {
    pub pending_input_id: String,
    #[serde(flatten)]
    pub refs: EntityRefs,
    pub input_kind: String,
    #[serde(default)]
    pub source: String,
    pub message: String,
    #[serde(default)]
    pub attachments: Vec<InputAttachmentSummary>,
    pub status: String,
    pub enqueue_reason: String,
    pub enqueued_at: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PauseCheckpointRecord {
    pub checkpoint_id: String,
    #[serde(flatten)]
    pub refs: EntityRefs,
    #[serde(default)]
    pub turn_id: Option<String>,
    #[serde(default)]
    pub active_step_id: Option<String>,
    #[serde(default)]
    pub resume_from_step_id: Option<String>,
    #[serde(default)]
    pub reason: Option<String>,
    pub paused_at: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct InterruptedSegmentRecord {
    pub segment_id: String,
    #[serde(flatten)]
    pub refs: EntityRefs,
    #[serde(default)]
    pub interrupted_turn_id: Option<String>,
    #[serde(default)]
    pub interrupted_step_id: Option<String>,
    #[serde(default)]
    pub resume_from_step_id: Option<String>,
    pub status: String,
    #[serde(default)]
    pub reason: Option<String>,
    pub created_at: String,
    #[serde(default)]
    pub merged_into_turn_id: Option<String>,
    #[serde(default)]
    pub merged_into_operation_id: Option<String>,
    #[serde(default)]
    pub merged_at: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SegmentMergeRecord {
    pub merge_id: String,
    pub segment_id: String,
    #[serde(flatten)]
    pub refs: EntityRefs,
    #[serde(default)]
    pub interrupted_turn_id: Option<String>,
    pub resumed_turn_id: String,
    pub resumed_operation_id: String,
    pub strategy: String,
    pub created_at: String,
}
