use fin_shared::require_non_empty;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct AgentId(pub String);

impl AgentId {
    pub fn new(value: impl Into<String>) -> Result<Self, fin_shared::SharedError> {
        let value = value.into();
        require_non_empty("agent_id", &value)?;
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct RoleId(pub String);

impl RoleId {
    pub fn new(value: impl Into<String>) -> Result<Self, fin_shared::SharedError> {
        let value = value.into();
        require_non_empty("role_id", &value)?;
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RoleProfileRef {
    pub role_id: RoleId,
}

impl RoleProfileRef {
    pub fn new(role_id: impl Into<String>) -> Result<Self, fin_shared::SharedError> {
        Ok(Self {
            role_id: RoleId::new(role_id)?,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderTarget {
    pub provider_name: String,
    pub model: String,
}

impl ProviderTarget {
    pub fn new(
        provider_name: impl Into<String>,
        model: impl Into<String>,
    ) -> Result<Self, fin_shared::SharedError> {
        let provider_name = provider_name.into();
        let model = model.into();
        require_non_empty("provider_name", &provider_name)?;
        require_non_empty("model", &model)?;
        Ok(Self {
            provider_name,
            model,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderPath {
    pub targets: Vec<ProviderTarget>,
}

impl ProviderPath {
    pub fn new(targets: Vec<ProviderTarget>) -> Result<Self, fin_shared::SharedError> {
        if targets.is_empty() {
            return Err(fin_shared::SharedError::EmptyValue {
                field: "provider_path.targets",
            });
        }
        Ok(Self { targets })
    }

    pub fn primary_target(&self) -> &ProviderTarget {
        &self.targets[0]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderStrategy {
    Priority,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MinimalContextView {
    pub continuity_tail: Vec<String>,
    pub summary: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InferenceOperationPayload {
    pub input: String,
    pub role: RoleProfileRef,
    pub provider_path: ProviderPath,
    pub provider_strategy: ProviderStrategy,
    pub protocol_version: String,
    pub stream: bool,
    pub context: MinimalContextView,
}

impl InferenceOperationPayload {
    pub fn validate(&self) -> Result<(), fin_shared::SharedError> {
        require_non_empty("input", &self.input)?;
        require_non_empty("protocol_version", &self.protocol_version)?;
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderEventPayload {
    pub provider_name: String,
    pub model: String,
    pub endpoint: String,
    pub output_text: Option<String>,
    pub response_id: Option<String>,
    pub stop_reason: Option<String>,
    pub status: Option<u16>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct EntityRefs {
    pub session_id: Option<String>,
    pub task_id: Option<String>,
    pub topic_thread_id: Option<String>,
    pub dispatch_id: Option<String>,
    pub worker_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OperationEnvelope<T> {
    pub operation_id: String,
    pub operation_type: String,
    pub submitted_at: String,
    pub source: String,
    pub trace_id: String,
    pub correlation_id: Option<String>,
    pub causation_id: Option<String>,
    #[serde(flatten)]
    pub refs: EntityRefs,
    pub idempotency_key: Option<String>,
    pub timeout_ms: Option<u64>,
    pub payload: T,
}

impl<T> OperationEnvelope<T> {
    pub fn new(
        operation_id: impl Into<String>,
        operation_type: impl Into<String>,
        submitted_at: impl Into<String>,
        source: impl Into<String>,
        trace_id: impl Into<String>,
        payload: T,
    ) -> Self {
        Self {
            operation_id: operation_id.into(),
            operation_type: operation_type.into(),
            submitted_at: submitted_at.into(),
            source: source.into(),
            trace_id: trace_id.into(),
            correlation_id: None,
            causation_id: None,
            refs: EntityRefs::default(),
            idempotency_key: None,
            timeout_ms: None,
            payload,
        }
    }

    pub fn validate(&self) -> Result<(), fin_shared::SharedError> {
        require_non_empty("operation_id", &self.operation_id)?;
        require_non_empty("operation_type", &self.operation_type)?;
        require_non_empty("submitted_at", &self.submitted_at)?;
        require_non_empty("source", &self.source)?;
        require_non_empty("trace_id", &self.trace_id)?;
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    Info,
    Warn,
    Error,
    Critical,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DebugVisibility {
    Normal,
    Important,
    Verbose,
    Internal,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EventEnvelope<T> {
    pub event_id: String,
    pub event_type: String,
    pub occurred_at: String,
    pub source: String,
    pub trace_id: String,
    pub correlation_id: Option<String>,
    pub causation_id: Option<String>,
    #[serde(flatten)]
    pub refs: EntityRefs,
    pub operation_id: Option<String>,
    pub sequence: u64,
    pub severity: Severity,
    pub debug_visibility: DebugVisibility,
    pub payload: T,
}

impl<T> EventEnvelope<T> {
    pub fn new(
        event_id: impl Into<String>,
        event_type: impl Into<String>,
        occurred_at: impl Into<String>,
        source: impl Into<String>,
        trace_id: impl Into<String>,
        sequence: u64,
        payload: T,
    ) -> Self {
        Self {
            event_id: event_id.into(),
            event_type: event_type.into(),
            occurred_at: occurred_at.into(),
            source: source.into(),
            trace_id: trace_id.into(),
            correlation_id: None,
            causation_id: None,
            refs: EntityRefs::default(),
            operation_id: None,
            sequence,
            severity: Severity::Info,
            debug_visibility: DebugVisibility::Normal,
            payload,
        }
    }

    pub fn validate(&self) -> Result<(), fin_shared::SharedError> {
        require_non_empty("event_id", &self.event_id)?;
        require_non_empty("event_type", &self.event_type)?;
        require_non_empty("occurred_at", &self.occurred_at)?;
        require_non_empty("source", &self.source)?;
        require_non_empty("trace_id", &self.trace_id)?;
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolSnapshot {
    pub tool_name: String,
    pub status: String,
    pub summary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProgressBlock {
    pub progress_id: String,
    #[serde(flatten)]
    pub refs: EntityRefs,
    pub phase: String,
    pub blocker: Option<String>,
    pub next_step: Option<String>,
    pub health_hint: Option<String>,
    pub tool_snapshots: Vec<ToolSnapshot>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionNote {
    pub note_id: String,
    #[serde(flatten)]
    pub refs: EntityRefs,
    pub summary: String,
    pub decision: Option<String>,
    pub lesson: Option<String>,
    pub blocker: Option<String>,
    pub next_step: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DigestRecord {
    pub digest_id: String,
    pub closure_id: String,
    #[serde(flatten)]
    pub refs: EntityRefs,
    pub summary: String,
    pub continuity_tail: Vec<String>,
    pub note_refs: Vec<String>,
    pub artifact_candidates: Vec<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectionView {
    pub session_id: Option<String>,
    pub task_id: Option<String>,
    pub topic_thread_id: Option<String>,
    pub current_phase: Option<String>,
    pub latest_progress_id: Option<String>,
    pub latest_note_id: Option<String>,
    pub latest_digest_id: Option<String>,
    pub latest_provider_activity: Option<String>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    Created,
    Dispatched,
    Accepted,
    Running,
    Claimed,
    Verified,
    Closed,
}

pub type Envelope<T> = EventEnvelope<T>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn role_and_agent_ids_require_non_empty_values() {
        assert!(AgentId::new("agent-1").is_ok());
        assert!(RoleProfileRef::new("project_leader").is_ok());
        assert!(AgentId::new("").is_err());
        assert!(RoleId::new("").is_err());
    }

    #[test]
    fn provider_path_requires_at_least_one_target() {
        let target = ProviderTarget::new("openai", "gpt-5").expect("target should build");
        let path = ProviderPath::new(vec![target.clone()]).expect("path should build");

        assert_eq!(path.primary_target(), &target);
        assert!(ProviderPath::new(vec![]).is_err());
    }

    #[test]
    fn inference_operation_payload_requires_input_and_protocol_version() {
        let payload = InferenceOperationPayload {
            input: "hello".into(),
            role: RoleProfileRef::new("coder").expect("role"),
            provider_path: ProviderPath::new(vec![
                ProviderTarget::new("ali-coding-plan", "qwen3.6-plus").expect("target"),
            ])
            .expect("path"),
            provider_strategy: ProviderStrategy::Priority,
            protocol_version: "fin.m1".into(),
            stream: false,
            context: MinimalContextView::default(),
        };
        assert!(payload.validate().is_ok());

        let invalid = InferenceOperationPayload {
            input: "".into(),
            ..payload
        };
        assert!(invalid.validate().is_err());
    }

    #[test]
    fn operation_envelope_constructor_sets_required_fields() {
        let op = OperationEnvelope::new(
            "op-1",
            "start_inference",
            "2026-04-17T00:00:00Z",
            "runtime",
            "trace-1",
            serde_json::json!({"input":"hello"}),
        );

        assert_eq!(op.operation_id, "op-1");
        assert_eq!(op.operation_type, "start_inference");
        assert!(op.validate().is_ok());
    }

    #[test]
    fn event_envelope_constructor_sets_defaults() {
        let event = EventEnvelope::new(
            "evt-1",
            "inference.started",
            "2026-04-17T00:00:01Z",
            "runtime",
            "trace-1",
            7,
            serde_json::json!({"model":"gpt"}),
        );

        assert_eq!(event.event_id, "evt-1");
        assert_eq!(event.sequence, 7);
        assert_eq!(event.severity, Severity::Info);
        assert_eq!(event.debug_visibility, DebugVisibility::Normal);
        assert!(event.validate().is_ok());
    }

    #[test]
    fn projection_view_serializes_stably() {
        let projection = ProjectionView {
            session_id: Some("session-1".into()),
            task_id: Some("task-1".into()),
            topic_thread_id: Some("topic-1".into()),
            current_phase: Some("running".into()),
            latest_progress_id: Some("progress-1".into()),
            latest_note_id: Some("note-1".into()),
            latest_digest_id: Some("digest-1".into()),
            latest_provider_activity: Some("provider.request_started".into()),
            warnings: vec!["timeout_near".into()],
        };

        let json = serde_json::to_value(&projection).expect("projection should serialize");
        assert_eq!(json["task_id"], "task-1");
        assert_eq!(json["warnings"][0], "timeout_near");
    }
}
