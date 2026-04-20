use fin_contracts::{
    AgentId, ClosureTraceRecord, ContextSnapshotRecord, ControlFeedback, DigestRecord, EntityRefs,
    EventEnvelope, ExecutionNote, InferenceOperationPayload, MinimalContextView, OperationEnvelope,
    ProgressBlock, ProviderEventPayload, ProviderPath, ProviderRequestRecord,
    ProviderResponseRecord, ProviderStrategy, ReasoningViewRecord, RoleProfileRef, RoundRecord,
    RoutingActionRecord, RoutingDecisionRecord, SanitizedProviderDebug, StepRecord,
    ToolExecutionRecord, ToolSnapshot, TurnRecord,
};
use fin_provider::{InferenceProvider, PreparedRequest, ProviderRequest, ProviderResponse};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;
mod activity_cards;
#[cfg(test)]
mod activity_cards_tests;
#[cfg(test)]
mod assembler_tests;
mod closure_runtime;
mod context_blocks;
mod context_view;
#[cfg(test)]
mod context_view_tests;
mod control_feedback;
mod control_plane;
mod model_input_assembler;
mod model_output;
#[cfg(test)]
mod model_output_tests;
mod prompt_assembly;
#[cfg(test)]
mod prompt_tests;
mod routing_actions;
mod scheduler;
mod session_materializer;
mod session_record_journal;
mod skill_loader;
mod tool_catalog;
mod tool_dispatch;
mod tool_dispatch_control;
mod tool_dispatch_extended;
mod tool_dispatch_extended_collab;
mod tool_dispatch_extended_collab_coordination;
mod tool_dispatch_extended_collab_mailbox;
mod tool_dispatch_extended_exec;
mod tool_dispatch_extended_patch;
mod tool_dispatch_extended_patch_v4a;
mod tool_dispatch_extended_query;
mod tool_dispatch_extended_query_history;
mod tool_dispatch_extended_query_image;
mod tool_dispatch_extended_query_task;
mod tool_dispatch_peer;
#[cfg(test)]
mod tool_dispatch_query_tests;
#[cfg(test)]
mod tool_dispatch_tests;
mod tool_semantics;
mod trace_records;
mod turn_records;
pub use activity_cards::build_activity_cards;
pub use context_view::{ContextAssemblyInput, ContextViewBuilder};
pub use control_feedback::ControlFeedbackBuilder;
pub use control_plane::{
    PendingInputDequeue, apply_segment_merge, clear_waiting_state_if_due, dequeue_pending_input,
    failed_state, interrupted_segment, new_pending_input, paused_state, resumed_state,
    running_state, segment_merge, state_after_run, state_with_pending_count,
};
pub use model_input_assembler::ModelInputAssembler;
pub use model_output::{ModelOutputParser, ParsedModelOutput};
pub use scheduler::derive_scheduler_decision;
pub use session_materializer::{
    SessionMaterializationReceipt, SessionMaterializer, SessionMessageRecord,
    append_framework_events,
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
        fin_shared::require_non_empty("submitted_at", &request.submitted_at)?;
        let payload = InferenceOperationPayload {
            input: request.input,
            role: worker.policy.role.clone(),
            provider_path: worker.policy.provider_path.clone(),
            provider_strategy: worker.policy.provider_strategy,
            protocol_version: worker.policy.protocol_version.clone(),
            stream: worker.policy.stream,
            context: request.context,
        };
        payload.validate()?;

        let mut operation = OperationEnvelope::new(
            request.operation_id,
            "start_inference",
            request.submitted_at,
            worker.source.clone(),
            request.trace_id,
            payload,
        );
        operation.refs = request.refs;
        operation.timeout_ms = Some(worker.policy.timeout_ms);
        Ok(operation)
    }
}
#[derive(Debug, Clone, PartialEq)]
pub struct ClosureRun {
    pub operation: OperationEnvelope<InferenceOperationPayload>,
    pub prepared_request: PreparedRequest,
    pub provider_response: ProviderResponse,
    pub assistant_response_text: String,
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

pub use closure_runtime::M1Runtime;

#[cfg(test)]
mod tests;
