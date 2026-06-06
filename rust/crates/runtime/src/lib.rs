use fin_contracts::{
    AgentId, ClosureTraceRecord, ContextSnapshotRecord, ControlFeedback, DigestRecord, EntityRefs,
    EventEnvelope, ExecutionNote, InferenceOperationPayload,
    MinimalContextView, OperationEnvelope, ProgressBlock, ProviderEventPayload, ProviderPath,
    ProviderRequestRecord, ProviderResponseRecord, ProviderStrategy, ReasoningViewRecord,
    RoleProfileRef, RoundRecord, RoutingActionRecord, RoutingDecisionRecord,
    SanitizedProviderDebug, StepRecord, ToolExecutionRecord, ToolSnapshot, TurnRecord,
};
use fin_provider::{InferenceProvider, PreparedRequest, ProviderRequest, ProviderResponse};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;
use crate::pipeline::input::{
    ChannelMetadata, InputIn01ChannelRaw, InputIn02NormalizedBuilder, InputIn03OperationBuilder,
    InputIn04SessionBoundBuilder, InputIn05ReasoningSeedBuilder,
};
mod activity_cards;
#[cfg(test)]
mod activity_cards_tests;
mod agent;
mod task;
mod closure;
#[cfg(test)]
mod run_closure_error_center_tests;
#[cfg(test)]
mod fault_injection_tests;
mod context;
mod control;

mod pipeline;
#[cfg(test)]
mod execution_checkpoint_tests;

mod model;

mod prompt;
mod round_context;
#[cfg(test)]
mod round_loop_runtime_tests;
#[cfg(test)]
mod round_loop_runtime_tests_contract_retry;
#[cfg(test)]
mod round_loop_runtime_tests_full_history;


mod session;

mod runtime_home;



mod tools;



pub use activity_cards::{build_activity_cards, build_activity_cards_for_session};
pub use agent::naming::{
    AgentAssignmentSummary, AllocatedAgentIdentity, allocate_local_agent_identity,
    create_named_local_worker, persist_assignment_summary, read_assignment_summary,
    resolve_agent_identity_by_worker_id, resolve_device_name,
};
pub use task::assignment_queue::{
    AssignmentRecord, append_assignment_record, read_assignment_queue,
    target_agent_name_from_worker_id, update_assignment_record,
};
pub use context::view::{ContextAssemblyInput, ContextViewBuilder};
pub use runtime_home::source_visibility::uses_ephemeral_session_persistence;
pub use control::feedback::ControlFeedbackBuilder;
pub use control::plane::{
    PendingInputDequeue, apply_segment_merge, clear_waiting_state_if_due, dequeue_pending_input,
    failed_state, interrupted_segment, new_pending_input, paused_state, resumed_state,
    running_state, segment_merge, state_after_run, state_with_pending_count,
};
pub use model::input_assembler::ModelInputAssembler;
pub use model::parser::{ModelOutputParser, ParsedModelOutput};
pub use control::owner_loop::derive_owner_loop_action_for_runtime;
pub use control::scheduler::derive_scheduler_decision;
pub use session::materializer::{
    SessionMaterializationReceipt, SessionMaterializer, SessionMessageRecord,
    append_framework_events,
};
pub use task::board_snapshot::TaskSummary;
pub use task::handoff::{TaskHandoffReceipt, handoff_project_task};
pub use task::store::{
    StoredTaskRecord, TaskMutationReceipt, create_task_record, load_task_record,
    session_dir_for_session_id, update_task_record,
};
#[derive(Debug, Error)]
pub enum RuntimeError {
    #[error(transparent)]
    Config(#[from] fin_config::ConfigError),
    #[error(transparent)]
    InvalidOperation(#[from] fin_shared::SharedError),
    #[error(transparent)]
    Provider(#[from] fin_provider::ProviderError),
    #[error("failed to serialize runtime payload: {0}")]
    Serialize(#[from] serde_json::Error),
    #[error("invalid runtime state: {0}")]
    State(String),
    #[error("io error at '{path}': {source}")]
    Io {
        path: String,
        #[source]
        source: std::io::Error,
    },
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimePolicySnapshot {
    pub role: RoleProfileRef,
    pub protocol_version: String,
    pub provider_strategy: ProviderStrategy,
    pub provider_path: ProviderPath,
    pub stream: bool,
    pub timeout_ms: u64,
}
impl RuntimePolicySnapshot {
    pub fn from_system(
        system: &fin_config::SystemConfig,
        role_id: Option<&str>,
    ) -> Result<Self, RuntimeError> {
        let (resolved_role_id, role_profile) = system.role_profile(role_id)?;

        Ok(Self {
            role: RoleProfileRef::new(resolved_role_id)?,
            protocol_version: system.policy.protocol_version.clone(),
            provider_strategy: role_profile.provider_path.strategy,
            provider_path: role_profile.provider_path.as_provider_path()?,
            stream: role_profile.stream,
            timeout_ms: role_profile.timeout_ms,
        })
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkerRuntime {
    pub agent_id: AgentId,
    pub worker_id: String,
    pub source: String,
    pub policy: RuntimePolicySnapshot,
}
impl WorkerRuntime {
    pub fn from_system(
        system: &fin_config::SystemConfig,
        agent_id: impl Into<String>,
        worker_id: impl Into<String>,
        source: impl Into<String>,
        role_id: Option<&str>,
    ) -> Result<Self, RuntimeError> {
        let worker_id = worker_id.into();
        fin_shared::require_non_empty("worker_id", &worker_id)?;

        let source = source.into();
        fin_shared::require_non_empty("source", &source)?;

        Ok(Self {
            agent_id: AgentId::new(agent_id)?,
            worker_id,
            source,
            policy: RuntimePolicySnapshot::from_system(system, role_id)?,
        })
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InferenceRequest {
    pub operation_id: String,
    pub trace_id: String,
    pub submitted_at: String,
    pub refs: EntityRefs,
    pub input: String,
    pub context: MinimalContextView,
}
#[derive(Debug, Clone, Default)]
pub struct InferenceOperationBuilder;

impl InferenceOperationBuilder {
    pub fn build(
        &self,
        worker: &WorkerRuntime,
        request: InferenceRequest,
    ) -> Result<OperationEnvelope<InferenceOperationPayload>, RuntimeError> {
        let raw = InputIn01ChannelRaw {
            operation_id: request.operation_id.clone(),
            trace_id: request.trace_id.clone(),
            submitted_at: request.submitted_at.clone(),
            source: worker.source.clone(),
            refs: request.refs.clone(),
            raw_input: request.input.clone(),
            raw_context: request.context.clone(),
            raw_attachments: Vec::new(),
            channel_metadata: ChannelMetadata { channel: String::new(), origin: worker.source.clone(), received_at: request.submitted_at.clone() },
        };
        let normalized = InputIn02NormalizedBuilder.build(raw)?;
        let operation_node = InputIn03OperationBuilder.build(normalized, worker)?;
        let session_bound = InputIn04SessionBoundBuilder.build(operation_node)?;
        let seed = InputIn05ReasoningSeedBuilder.build(session_bound)?;
        Ok(seed.operation)
    }
}
#[derive(Debug, Clone, PartialEq)]
pub struct ClosureRun {
    pub operation: OperationEnvelope<InferenceOperationPayload>,
    pub prepared_request: PreparedRequest,
    pub provider_response: ProviderResponse,
    pub assistant_response_text: String,
    pub conversation_user_input: Option<String>,
    pub control_feedback: ControlFeedback,
    pub context_snapshot: ContextSnapshotRecord,
    pub tool_records: Vec<ToolExecutionRecord>,
    pub provider_request_records: Vec<ProviderRequestRecord>,
    pub provider_response_records: Vec<ProviderResponseRecord>,
    pub round_records: Vec<RoundRecord>,
    pub step_records: Vec<StepRecord>,
    pub progress: ProgressBlock,
    pub note: ExecutionNote,
    pub reasoning_view: ReasoningViewRecord,
    pub digest: DigestRecord,
    pub turn_record: TurnRecord,
    pub routing_decision: RoutingDecisionRecord,
    pub routing_action: RoutingActionRecord,
    pub closure_trace: ClosureTraceRecord,
    pub events: Vec<EventEnvelope<Value>>,
}

pub use closure::closure_runtime::M1Runtime;

#[cfg(test)]
mod tests;
