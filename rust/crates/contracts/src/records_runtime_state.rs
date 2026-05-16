use super::*;

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
    #[serde(default)]
    pub resume_checkpoint_ready: Option<bool>,
    #[serde(default)]
    pub resume_checkpoint_id: Option<String>,
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
    pub resume_checkpoint_id: Option<String>,
    #[serde(default)]
    pub reason: Option<String>,
    pub paused_at: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionCheckpointRecord {
    pub checkpoint_id: String,
    #[serde(flatten)]
    pub refs: EntityRefs,
    pub trace_id: String,
    pub source_operation_id: String,
    pub source_turn_id: String,
    pub source_step_id: String,
    pub checkpoint_kind: String,
    pub status: String,
    pub source_round_index: u32,
    pub next_round_index: u32,
    pub resume_input: String,
    #[serde(default)]
    pub summary: Option<String>,
    pub created_at: String,
    #[serde(default)]
    pub consumed_at: Option<String>,
    #[serde(default)]
    pub consumed_by_operation_id: Option<String>,
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
