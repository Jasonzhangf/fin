use fin_contracts::{
    AgentId, ClosureTraceRecord, ContextSnapshotRecord, ControlFeedback, DigestRecord, EntityRefs,
    EventEnvelope, ExecutionNote,
    InferenceOperationPayload, MinimalContextView, OperationEnvelope, ProgressBlock,
    ProviderEventPayload, ProviderPath, ProviderStrategy, ReasoningViewRecord, RoleProfileRef,
    SanitizedProviderDebug, ToolExecutionRecord, ToolSnapshot,
};
use fin_provider::{InferenceProvider, PreparedRequest, ProviderRequest, ProviderResponse};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;
mod context_blocks;
mod context_view;
mod control_feedback;
mod model_input_assembler;
mod model_output;
mod prompt_assembly;
mod session_materializer;
mod skill_loader;
mod trace_records;
#[cfg(test)]
mod assembler_tests;
#[cfg(test)]
mod context_view_tests;
#[cfg(test)]
mod model_output_tests;
#[cfg(test)]
mod prompt_tests;
pub use context_view::{ContextAssemblyInput, ContextViewBuilder};
pub use control_feedback::ControlFeedbackBuilder;
pub use model_input_assembler::ModelInputAssembler;
pub use model_output::{ModelOutputParser, ParsedModelOutput};
pub use session_materializer::{SessionMaterializationReceipt, SessionMaterializer, SessionMessageRecord};
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
    pub progress: ProgressBlock,
    pub note: ExecutionNote,
    pub reasoning_view: ReasoningViewRecord,
    pub digest: DigestRecord,
    pub closure_trace: ClosureTraceRecord,
    pub events: Vec<EventEnvelope<Value>>,
}
#[derive(Debug, Clone)]
pub struct M1Runtime {
    source: String,
    sequence: u64,
}
impl Default for M1Runtime {
    fn default() -> Self {
        Self::new("runtime")
    }
}

impl M1Runtime {
    pub fn new(source: impl Into<String>) -> Self {
        Self {
            source: source.into(),
            sequence: 0,
        }
    }

    pub fn run_closure(
        &mut self,
        operation: OperationEnvelope<InferenceOperationPayload>,
        provider: &impl InferenceProvider,
    ) -> Result<ClosureRun, RuntimeError> {
        operation.validate()?;
        operation.payload.validate()?;
        let refs = operation.refs.clone();
        let rendered_input =
            ModelInputAssembler::default().assemble(&operation.payload.input, &operation.payload.context);
        let prepared_request = provider.prepare_request(&ProviderRequest {
            input: operation.payload.input.clone(),
            rendered_input: Some(rendered_input),
            override_model: Some(
                operation
                    .payload
                    .provider_path
                    .primary_target()
                    .model
                    .clone(),
            ),
        });
        let provider_response = provider.execute_prepared(&prepared_request)?;
        let provider_debug = SanitizedProviderDebug {
            user_agent: prepared_request.user_agent.clone(),
            request_headers: prepared_request.sanitized_headers.clone(),
        };
        let parsed_output =
            ModelOutputParser::default().parse(&operation.payload, &prepared_request, &provider_response);
        let assistant_response_text = parsed_output.user_response.clone();
        let fallback_feedback = ControlFeedbackBuilder.build(
            &operation.payload,
            &prepared_request,
            &provider_response,
        );
        let mut control_feedback = ControlFeedbackBuilder::default()
            .merge_with_fallback(parsed_output.control_feedback.clone(), fallback_feedback);
        ControlFeedbackBuilder::default().rewrite_runtime_heuristic_candidates(
            &mut control_feedback,
            &prepared_request,
            &provider_response,
            assistant_response_text.as_str(),
        );
        let context_snapshot = ContextSnapshotRecord {
            operation_id: operation.operation_id.clone(),
            trace_id: operation.trace_id.clone(),
            refs: refs.clone(),
            input: operation.payload.input.clone(),
            context: operation.payload.context.clone(),
            role: operation.payload.role.clone(),
            provider_path: operation.payload.provider_path.clone(),
            provider_strategy: operation.payload.provider_strategy,
            protocol_version: operation.payload.protocol_version.clone(),
            stream: operation.payload.stream,
            captured_at: operation.submitted_at.clone(),
        };

        let progress = ProgressBlock {
            progress_id: format!("progress-{}", operation.operation_id),
            refs: refs.clone(),
            phase: "inference_completed".into(),
            blocker: None,
            next_step: Some("render_projection".into()),
            health_hint: Some("healthy".into()),
            tool_snapshots: vec![ToolSnapshot {
                tool_name: "provider.call".into(),
                status: "completed".into(),
                summary: format!(
                    "{} -> {} @ {} => {}",
                    prepared_request.provider_name,
                    prepared_request.model,
                    prepared_request.endpoint,
                    assistant_response_text.as_str()
                ),
            }],
        };
        let tool_records = vec![trace_records::provider_tool_record(
            &operation.operation_id,
            &operation.trace_id,
            &refs,
            &prepared_request,
            &provider_response,
            assistant_response_text.as_str(),
            &operation.submitted_at,
        )];

        let note = ExecutionNote {
            note_id: format!("note-{}", operation.operation_id),
            refs: refs.clone(),
            summary: format!(
                "provider {} returned: {}",
                prepared_request.provider_name,
                assistant_response_text.as_str()
            ),
            decision: Some(if control_feedback.is_continuation {
                "continue_current_task".into()
            } else {
                "observe_topic_continuity".into()
            }),
            lesson: None,
            blocker: None,
            next_step: Some("render_projection".into()),
            control_feedback: Some(control_feedback.clone()),
            created_at: operation.submitted_at.clone(),
        };
        let reasoning_view = trace_records::reasoning_view_record(
            &operation.operation_id,
            &operation.trace_id,
            &refs,
            &operation.submitted_at,
            &note,
            &control_feedback,
            &tool_records,
        );

        let digest = DigestRecord {
            digest_id: format!("digest-{}", operation.operation_id),
            closure_id: format!("closure-{}", operation.operation_id),
            refs: refs.clone(),
            summary: format!(
                "closure finished with model {} and answer {}",
                prepared_request.model,
                assistant_response_text.as_str()
            ),
            continuity_tail: vec![
                operation.payload.input.clone(),
                assistant_response_text.clone(),
            ],
            note_refs: vec![note.note_id.clone()],
            artifact_candidates: vec![format!(
                "provider:{}:{}:{}",
                prepared_request.provider_name,
                prepared_request.model,
                assistant_response_text.as_str()
            )],
            control_feedback: Some(control_feedback.clone()),
            created_at: operation.submitted_at.clone(),
        };

        let mut events = Vec::new();
        {
            let mut push_event = |event_type: &str, payload: Value| -> Result<(), RuntimeError> {
                events.push(self.event(
                    event_type,
                    &operation.trace_id,
                    &operation.submitted_at,
                    &refs,
                    Some(operation.operation_id.clone()),
                    payload,
                )?);
                Ok(())
            };
            push_event(
                "operation.accepted",
                serde_json::json!({"operation_type": operation.operation_type.clone()}),
            )?;
            push_event(
                "inference.started",
                serde_json::json!({
                    "provider": prepared_request.provider_name.clone(),
                    "model": prepared_request.model.clone(),
                    "input": operation.payload.input.clone(),
                    "rendered_input": prepared_request.rendered_input.clone(),
                    "context": operation.payload.context.clone(),
                    "role": operation.payload.role.clone(),
                    "provider_path": operation.payload.provider_path.clone(),
                    "provider_strategy": operation.payload.provider_strategy,
                    "protocol_version": operation.payload.protocol_version.clone(),
                    "stream": operation.payload.stream,
                }),
            )?;
        let provider_started_payload = ProviderEventPayload {
            provider_name: prepared_request.provider_name.clone(),
            model: prepared_request.model.clone(),
            endpoint: prepared_request.endpoint.clone(),
            output_text: None,
            response_id: None,
            stop_reason: None,
            status: None,
            debug: Some(provider_debug.clone()),
        };
        let provider_payload = ProviderEventPayload {
            provider_name: prepared_request.provider_name.clone(),
            model: prepared_request.model.clone(),
            endpoint: prepared_request.endpoint.clone(),
            output_text: Some(provider_response.output_text.clone()),
            response_id: provider_response.response_id.clone(),
            stop_reason: provider_response.stop_reason.clone(),
            status: Some(provider_response.status),
            debug: Some(provider_debug),
        };
            push_event(
                "provider.operation_accepted",
                serde_json::to_value(&provider_started_payload)?,
            )?;
            push_event(
                "provider.gateway_request_sent",
                serde_json::json!({
                    "provider_name": prepared_request.provider_name.clone(),
                    "model": prepared_request.model.clone(),
                    "endpoint": prepared_request.endpoint.clone(),
                }),
            )?;
            push_event(
                "provider.gateway_response_received",
                serde_json::to_value(&provider_payload)?,
            )?;
            push_event(
                "provider.response_normalized",
                serde_json::to_value(&provider_payload)?,
            )?;
            push_event("provider.completed", serde_json::to_value(&provider_payload)?)?;
            push_event(
                "model.output_parsed",
                serde_json::json!({
                    "contract_detected": parsed_output.contract_detected,
                    "control_feedback_parsed": parsed_output.control_feedback.is_some(),
                    "control_feedback_salvaged": parsed_output.control_feedback_salvaged,
                    "assistant_response": assistant_response_text.as_str(),
                    "control_feedback_origin": control_feedback.origin,
                }),
            )?;
            push_event("progress.updated", serde_json::to_value(&progress)?)?;
            push_event("tool.execution_recorded", serde_json::to_value(&tool_records)?)?;
            push_event(
                "control.feedback_recorded",
                serde_json::to_value(&control_feedback)?,
            )?;
            push_event("execution_note.appended", serde_json::to_value(&note)?)?;
            push_event(
                "reasoning.view_recorded",
                serde_json::to_value(&reasoning_view)?,
            )?;
            push_event("digest.finalized", serde_json::to_value(&digest)?)?;
        }
        let partial_run = ClosureRun {
            operation: operation.clone(),
            prepared_request: prepared_request.clone(),
            provider_response: provider_response.clone(),
            assistant_response_text: assistant_response_text.clone(),
            control_feedback: control_feedback.clone(),
            context_snapshot: context_snapshot.clone(),
            tool_records: tool_records.clone(),
            progress: progress.clone(),
            note: note.clone(),
            reasoning_view: reasoning_view.clone(),
            digest: digest.clone(),
            closure_trace: ClosureTraceRecord::default(),
            events: events.clone(),
        };
        let closure_trace = trace_records::closure_trace_record(&partial_run);
        events.push(self.event(
            "closure.trace_recorded",
            &operation.trace_id,
            &operation.submitted_at,
            &refs,
            Some(operation.operation_id.clone()),
            serde_json::to_value(&closure_trace)?,
        )?);
        events.push(self.event(
            "operation.completed",
            &operation.trace_id,
            &operation.submitted_at,
            &refs,
            Some(operation.operation_id.clone()),
            serde_json::json!({
                "status": "ok",
                "answer": assistant_response_text.clone(),
            }),
        )?);

        Ok(ClosureRun {
            operation,
            prepared_request,
            provider_response,
            assistant_response_text,
            control_feedback,
            context_snapshot,
            tool_records,
            progress,
            note,
            reasoning_view,
            digest,
            closure_trace,
            events,
        })
    }

    fn event(
        &mut self,
        event_type: &str,
        trace_id: &str,
        occurred_at: &str,
        refs: &EntityRefs,
        operation_id: Option<String>,
        payload: Value,
    ) -> Result<EventEnvelope<Value>, RuntimeError> {
        self.sequence += 1;
        let mut event = EventEnvelope::new(
            format!("evt-{}", self.sequence),
            event_type,
            occurred_at.to_string(),
            self.source.clone(),
            trace_id.to_string(),
            self.sequence,
            payload,
        );
        event.refs = refs.clone();
        event.operation_id = operation_id;
        event.validate()?;
        Ok(event)
    }
}

#[cfg(test)]
mod tests;
