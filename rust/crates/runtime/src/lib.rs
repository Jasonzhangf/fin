use fin_contracts::{
    AgentId, ContextSnapshotRecord, DigestRecord, EntityRefs, EventEnvelope, ExecutionNote,
    InferenceOperationPayload, MinimalContextView, OperationEnvelope, ProgressBlock,
    ProviderEventPayload, ProviderPath, ProviderStrategy, RoleProfileRef, SanitizedProviderDebug,
    ToolSnapshot,
};
use fin_provider::{InferenceProvider, PreparedRequest, ProviderRequest, ProviderResponse};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

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
    pub context_snapshot: ContextSnapshotRecord,
    pub progress: ProgressBlock,
    pub note: ExecutionNote,
    pub digest: DigestRecord,
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

fn render_provider_input(input: &str, context: &MinimalContextView) -> String {
    let has_summary = context
        .summary
        .as_deref()
        .map(|value| !value.trim().is_empty())
        .unwrap_or(false);
    if !has_summary && context.continuity_tail.is_empty() {
        return input.to_string();
    }

    let mut sections = Vec::new();
    if let Some(summary) = context
        .summary
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    {
        sections.push(format!("Context summary:\n{summary}"));
    }
    if !context.continuity_tail.is_empty() {
        sections.push(format!(
            "Continuity tail:\n- {}",
            context.continuity_tail.join("\n- ")
        ));
    }
    sections.push(format!("Current user input:\n{input}"));
    sections.join("\n\n")
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
        let prepared_request = provider.prepare_request(&ProviderRequest {
            input: operation.payload.input.clone(),
            rendered_input: Some(render_provider_input(
                &operation.payload.input,
                &operation.payload.context,
            )),
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
                    provider_response.output_text
                ),
            }],
        };

        let note = ExecutionNote {
            note_id: format!("note-{}", operation.operation_id),
            refs: refs.clone(),
            summary: format!(
                "provider {} returned: {}",
                prepared_request.provider_name, provider_response.output_text
            ),
            decision: Some("real_inference_closure_verified".into()),
            lesson: None,
            blocker: None,
            next_step: Some("render_projection".into()),
            created_at: operation.submitted_at.clone(),
        };

        let digest = DigestRecord {
            digest_id: format!("digest-{}", operation.operation_id),
            closure_id: format!("closure-{}", operation.operation_id),
            refs: refs.clone(),
            summary: format!(
                "closure finished with model {} and answer {}",
                prepared_request.model, provider_response.output_text
            ),
            continuity_tail: vec![
                operation.payload.input.clone(),
                provider_response.output_text.clone(),
            ],
            note_refs: vec![note.note_id.clone()],
            artifact_candidates: vec![format!(
                "provider:{}:{}:{}",
                prepared_request.provider_name,
                prepared_request.model,
                provider_response.output_text
            )],
            created_at: operation.submitted_at.clone(),
        };

        let mut events = Vec::new();
        events.push(self.event(
            "operation.accepted",
            &operation.trace_id,
            &operation.submitted_at,
            &refs,
            Some(operation.operation_id.clone()),
            serde_json::json!({"operation_type": operation.operation_type.clone()}),
        )?);
        events.push(self.event(
            "inference.started",
            &operation.trace_id,
            &operation.submitted_at,
            &refs,
            Some(operation.operation_id.clone()),
            serde_json::json!({
                "provider": prepared_request.provider_name.clone(),
                "model": prepared_request.model.clone(),
                "input": operation.payload.input.clone(),
                "context": operation.payload.context.clone(),
                "role": operation.payload.role.clone(),
                "provider_path": operation.payload.provider_path.clone(),
                "provider_strategy": operation.payload.provider_strategy,
                "protocol_version": operation.payload.protocol_version.clone(),
                "stream": operation.payload.stream,
            }),
        )?);
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
        events.push(self.event(
            "provider.operation_accepted",
            &operation.trace_id,
            &operation.submitted_at,
            &refs,
            Some(operation.operation_id.clone()),
            serde_json::to_value(&provider_started_payload)?,
        )?);
        events.push(self.event(
            "provider.gateway_request_sent",
            &operation.trace_id,
            &operation.submitted_at,
            &refs,
            Some(operation.operation_id.clone()),
            serde_json::json!({
                "provider_name": prepared_request.provider_name.clone(),
                "model": prepared_request.model.clone(),
                "endpoint": prepared_request.endpoint.clone(),
            }),
        )?);
        events.push(self.event(
            "provider.gateway_response_received",
            &operation.trace_id,
            &operation.submitted_at,
            &refs,
            Some(operation.operation_id.clone()),
            serde_json::to_value(&provider_payload)?,
        )?);
        events.push(self.event(
            "provider.response_normalized",
            &operation.trace_id,
            &operation.submitted_at,
            &refs,
            Some(operation.operation_id.clone()),
            serde_json::to_value(&provider_payload)?,
        )?);
        events.push(self.event(
            "provider.completed",
            &operation.trace_id,
            &operation.submitted_at,
            &refs,
            Some(operation.operation_id.clone()),
            serde_json::to_value(&provider_payload)?,
        )?);
        events.push(self.event(
            "progress.updated",
            &operation.trace_id,
            &operation.submitted_at,
            &refs,
            Some(operation.operation_id.clone()),
            serde_json::to_value(&progress)?,
        )?);
        events.push(self.event(
            "execution_note.appended",
            &operation.trace_id,
            &operation.submitted_at,
            &refs,
            Some(operation.operation_id.clone()),
            serde_json::to_value(&note)?,
        )?);
        events.push(self.event(
            "digest.finalized",
            &operation.trace_id,
            &operation.submitted_at,
            &refs,
            Some(operation.operation_id.clone()),
            serde_json::to_value(&digest)?,
        )?);
        events.push(self.event(
            "operation.completed",
            &operation.trace_id,
            &operation.submitted_at,
            &refs,
            Some(operation.operation_id.clone()),
            serde_json::json!({
                "status": "ok",
                "answer": provider_response.output_text.clone(),
            }),
        )?);

        Ok(ClosureRun {
            operation,
            prepared_request,
            provider_response,
            context_snapshot,
            progress,
            note,
            digest,
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