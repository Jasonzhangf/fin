use fin_contracts::{
    AgentId, DigestRecord, EntityRefs, EventEnvelope, ExecutionNote, OperationEnvelope,
    ProgressBlock, ProviderPath, ProviderStrategy, RoleProfileRef, ToolSnapshot,
};
use fin_provider::{PreparedRequest, ProviderDescriptor, ProviderRequest};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum RuntimeError {
    #[error(transparent)]
    Config(#[from] fin_config::ConfigError),
    #[error(transparent)]
    InvalidOperation(#[from] fin_shared::SharedError),
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

#[derive(Debug, Clone, PartialEq)]
pub struct ClosureRun {
    pub prepared_request: PreparedRequest,
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

impl M1Runtime {
    pub fn new(source: impl Into<String>) -> Self {
        Self {
            source: source.into(),
            sequence: 0,
        }
    }

    pub fn run_closure(
        &mut self,
        operation: OperationEnvelope<Value>,
        provider: &ProviderDescriptor,
    ) -> Result<ClosureRun, RuntimeError> {
        operation.validate()?;
        let refs = operation.refs.clone();
        let prepared_request = provider.prepare_request(&ProviderRequest {
            input: extract_input(&operation.payload),
            override_model: None,
        });

        let progress = ProgressBlock {
            progress_id: format!("progress-{}", operation.operation_id),
            refs: refs.clone(),
            phase: "inference_running".into(),
            blocker: None,
            next_step: Some("finalize_digest".into()),
            health_hint: Some("healthy".into()),
            tool_snapshots: vec![ToolSnapshot {
                tool_name: "provider.call".into(),
                status: "completed".into(),
                summary: format!(
                    "{} -> {} @ {}",
                    prepared_request.provider_name,
                    prepared_request.model,
                    prepared_request.endpoint
                ),
            }],
        };

        let note = ExecutionNote {
            note_id: format!("note-{}", operation.operation_id),
            refs: refs.clone(),
            summary: format!(
                "single runtime closure executed with provider {}",
                prepared_request.provider_name
            ),
            decision: Some("continue_m1_vertical_slice".into()),
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
                "closure finished with model {} and provider {}",
                prepared_request.model, prepared_request.provider_name
            ),
            continuity_tail: vec![operation.operation_type.clone()],
            note_refs: vec![note.note_id.clone()],
            artifact_candidates: vec![format!(
                "provider:{}:{}",
                prepared_request.provider_name, prepared_request.model
            )],
            created_at: operation.submitted_at.clone(),
        };

        let mut events = Vec::new();
        events.push(self.event(
            "operation.accepted",
            &operation.trace_id,
            &refs,
            Some(operation.operation_id.clone()),
            serde_json::json!({"operation_type": operation.operation_type}),
        )?);
        events.push(self.event(
            "inference.started",
            &operation.trace_id,
            &refs,
            Some(operation.operation_id.clone()),
            serde_json::json!({"provider": prepared_request.provider_name, "model": prepared_request.model}),
        )?);
        events.push(self.event(
            "provider.request_started",
            &operation.trace_id,
            &refs,
            Some(operation.operation_id.clone()),
            serde_json::to_value(&prepared_request)?,
        )?);
        events.push(self.event(
            "provider.response_received",
            &operation.trace_id,
            &refs,
            Some(operation.operation_id.clone()),
            serde_json::json!({
                "provider": prepared_request.provider_name,
                "model": prepared_request.model,
                "status": "simulated_ok"
            }),
        )?);
        events.push(self.event(
            "progress.updated",
            &operation.trace_id,
            &refs,
            Some(operation.operation_id.clone()),
            serde_json::to_value(&progress)?,
        )?);
        events.push(self.event(
            "execution_note.appended",
            &operation.trace_id,
            &refs,
            Some(operation.operation_id.clone()),
            serde_json::to_value(&note)?,
        )?);
        events.push(self.event(
            "digest.finalized",
            &operation.trace_id,
            &refs,
            Some(operation.operation_id.clone()),
            serde_json::to_value(&digest)?,
        )?);
        events.push(self.event(
            "operation.completed",
            &operation.trace_id,
            &refs,
            Some(operation.operation_id),
            serde_json::json!({"status": "ok"}),
        )?);

        Ok(ClosureRun {
            prepared_request,
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
        refs: &EntityRefs,
        operation_id: Option<String>,
        payload: Value,
    ) -> Result<EventEnvelope<Value>, RuntimeError> {
        self.sequence += 1;
        let mut event = EventEnvelope::new(
            format!("evt-{}", self.sequence),
            event_type,
            format!("seq-{}", self.sequence),
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

fn extract_input(payload: &Value) -> String {
    payload
        .get("input")
        .and_then(Value::as_str)
        .unwrap_or("<empty-input>")
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use fin_config::{
        ConfigMapper, ProviderCredential, ProviderProtocol, ResolvedProviderConfig, UserConfig,
        UserProviderConfig,
    };
    use fin_contracts::OperationEnvelope;
    use fin_provider::ProviderDescriptor;
    use std::collections::BTreeMap;

    fn provider() -> ProviderDescriptor {
        ProviderDescriptor::from_resolved(&ResolvedProviderConfig {
            name: "openai".into(),
            protocol: ProviderProtocol::OpenAiCompatible,
            base_url: "https://api.example.com/v1".into(),
            model: "gpt-5".into(),
            credential: ProviderCredential::ApiKeyEnv {
                env_var: "OPENAI_API_KEY".into(),
            },
        })
    }

    #[test]
    fn run_closure_emits_expected_event_chain() {
        let mut runtime = M1Runtime::default();
        let mut op = OperationEnvelope::new(
            "op-1",
            "start_inference",
            "2026-04-17T00:00:00Z",
            "runtime",
            "trace-1",
            serde_json::json!({"input":"hello"}),
        );
        op.refs.task_id = Some("task-1".into());
        op.refs.session_id = Some("session-1".into());

        let run = runtime
            .run_closure(op, &provider())
            .expect("closure should run");
        let kinds: Vec<_> = run.events.iter().map(|e| e.event_type.as_str()).collect();
        assert_eq!(
            kinds,
            vec![
                "operation.accepted",
                "inference.started",
                "provider.request_started",
                "provider.response_received",
                "progress.updated",
                "execution_note.appended",
                "digest.finalized",
                "operation.completed",
            ]
        );
        assert_eq!(run.prepared_request.model, "gpt-5");
        assert!(
            run.progress
                .tool_snapshots
                .first()
                .expect("tool snapshot")
                .summary
                .contains("openai")
        );
        assert!(run.events.iter().all(|event| event.trace_id == "trace-1"));
    }

    #[test]
    fn runtime_policy_snapshot_builds_from_default_role() {
        let user = UserConfig {
            default_provider: "openai".into(),
            providers: BTreeMap::from([(
                "openai".into(),
                UserProviderConfig {
                    protocol: ProviderProtocol::OpenAiCompatible,
                    base_url: "https://api.example.com/v1".into(),
                    model: "gpt-5".into(),
                    api_key: None,
                    api_key_env: Some("OPENAI_API_KEY".into()),
                },
            )]),
        };
        let system = ConfigMapper::map_user_to_system(&user).expect("mapping should succeed");

        let snapshot =
            RuntimePolicySnapshot::from_system(&system, None).expect("snapshot should build");
        assert_eq!(snapshot.role.role_id.as_str(), "default");
        assert_eq!(snapshot.protocol_version, "fin.m1");
        assert_eq!(snapshot.provider_strategy, ProviderStrategy::Priority);
        assert_eq!(
            snapshot.provider_path.primary_target().provider_name,
            "openai"
        );

        let encoded = serde_json::to_string(&snapshot).expect("snapshot should serialize");
        let decoded: RuntimePolicySnapshot =
            serde_json::from_str(&encoded).expect("snapshot should deserialize");
        assert_eq!(decoded, snapshot);
    }

    #[test]
    fn worker_runtime_inherits_policy_snapshot() {
        let user = UserConfig {
            default_provider: "openai".into(),
            providers: BTreeMap::from([(
                "openai".into(),
                UserProviderConfig {
                    protocol: ProviderProtocol::OpenAiCompatible,
                    base_url: "https://api.example.com/v1".into(),
                    model: "gpt-5".into(),
                    api_key: None,
                    api_key_env: Some("OPENAI_API_KEY".into()),
                },
            )]),
        };
        let system = ConfigMapper::map_user_to_system(&user).expect("mapping should succeed");

        let runtime = WorkerRuntime::from_system(
            &system,
            "agent-project-leader",
            "worker-1",
            "runtime",
            None,
        )
        .expect("worker runtime should build");

        assert_eq!(runtime.agent_id.as_str(), "agent-project-leader");
        assert_eq!(runtime.worker_id, "worker-1");
        assert_eq!(runtime.policy.provider_path.primary_target().model, "gpt-5");
    }
}
