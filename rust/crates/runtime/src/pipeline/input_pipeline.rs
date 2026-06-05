use crate::*;
use fin_contracts::{EntityRefs, MinimalContextView, OperationEnvelope};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InputIn01ChannelRaw {
    pub operation_id: String,
    pub trace_id: String,
    pub submitted_at: String,
    pub source: String,
    pub refs: EntityRefs,
    pub raw_input: String,
    pub raw_context: MinimalContextView,
    pub raw_attachments: Vec<RawAttachment>,
    pub channel_metadata: ChannelMetadata,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct RawAttachment {
    pub attachment_id: String,
    pub kind: String,
    pub summary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ChannelMetadata {
    pub channel: String,
    pub origin: String,
    pub received_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InputIn02Normalized {
    pub raw: InputIn01ChannelRaw,
    pub normalized_input: String,
    pub normalized_attachments: Vec<RawAttachment>,
    pub source_label: String,
    pub received_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InputIn03Operation {
    pub normalized: InputIn02Normalized,
    pub envelope: OperationEnvelope<InferenceOperationPayload>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InputIn04SessionBound {
    pub operation_node: InputIn03Operation,
    pub operation_id: String,
    pub trace_id: String,
    pub submitted_at: String,
    pub refs: EntityRefs,
    pub input: String,
    pub context: MinimalContextView,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InputIn05ReasoningSeed {
    pub session_bound: InputIn04SessionBound,
    pub operation: OperationEnvelope<InferenceOperationPayload>,
}

#[derive(Debug, Clone, Default)]
pub struct InputIn02NormalizedBuilder;

impl InputIn02NormalizedBuilder {
    pub fn build(&self, raw: InputIn01ChannelRaw) -> Result<InputIn02Normalized, RuntimeError> {
        fin_shared::require_non_empty("operation_id", &raw.operation_id)?;
        fin_shared::require_non_empty("source", &raw.source)?;
        let normalized_input = raw.raw_input.trim().to_string();
        if normalized_input.is_empty() && raw.raw_attachments.is_empty() {
            return Err(RuntimeError::State(
                "InputIn02NormalizedBuilder: raw_input and raw_attachments both empty".into(),
            ));
        }
        let source_label = if raw.channel_metadata.channel.trim().is_empty() {
            raw.source.clone()
        } else {
            format!(
                "{}::{}",
                raw.channel_metadata.channel, raw.channel_metadata.origin
            )
        };
        Ok(InputIn02Normalized {
            normalized_input: if normalized_input.is_empty() {
                raw.raw_input.clone()
            } else {
                normalized_input
            },
            normalized_attachments: raw.raw_attachments.clone(),
            source_label,
            received_at: raw.channel_metadata.received_at.clone(),
            raw,
        })
    }
}

#[derive(Debug, Clone, Default)]
pub struct InputIn03OperationBuilder;

impl InputIn03OperationBuilder {
    pub fn build(
        &self,
        normalized: InputIn02Normalized,
        worker: &WorkerRuntime,
    ) -> Result<InputIn03Operation, RuntimeError> {
        let payload = InferenceOperationPayload {
            input: normalized.normalized_input.clone(),
            role: worker.policy.role.clone(),
            provider_path: worker.policy.provider_path.clone(),
            provider_strategy: worker.policy.provider_strategy,
            protocol_version: worker.policy.protocol_version.clone(),
            stream: worker.policy.stream,
            context: normalized.raw.raw_context.clone(),
        };
        payload.validate()?;

        let mut envelope = OperationEnvelope::new(
            normalized.raw.operation_id.clone(),
            "start_inference",
            normalized.raw.submitted_at.clone(),
            worker.source.clone(),
            normalized.raw.trace_id.clone(),
            payload,
        );
        envelope.refs = normalized.raw.refs.clone();
        envelope.timeout_ms = Some(worker.policy.timeout_ms);
        Ok(InputIn03Operation {
            normalized,
            envelope,
        })
    }
}

#[derive(Debug, Clone, Default)]
pub struct InputIn04SessionBoundBuilder;

impl InputIn04SessionBoundBuilder {
    pub fn build(
        &self,
        operation_node: InputIn03Operation,
    ) -> Result<InputIn04SessionBound, RuntimeError> {
        let operation_id = operation_node.envelope.operation_id.clone();
        let trace_id = operation_node.envelope.trace_id.clone();
        let submitted_at = operation_node.envelope.submitted_at.clone();
        let refs = operation_node.envelope.refs.clone();
        let input = operation_node.envelope.payload.input.clone();
        let context = operation_node.envelope.payload.context.clone();
        fin_shared::require_non_empty("operation_id", &operation_id)?;
        fin_shared::require_non_empty("submitted_at", &submitted_at)?;
        Ok(InputIn04SessionBound {
            operation_node,
            operation_id,
            trace_id,
            submitted_at,
            refs,
            input,
            context,
        })
    }
}

#[derive(Debug, Clone, Default)]
pub struct InputIn05ReasoningSeedBuilder;

impl InputIn05ReasoningSeedBuilder {
    pub fn build(
        &self,
        session_bound: InputIn04SessionBound,
    ) -> Result<InputIn05ReasoningSeed, RuntimeError> {
        let _ = session_bound
            .operation_node
            .envelope
            .refs
            .session_id
            .as_deref()
            .unwrap_or("session-m1");
        let operation = session_bound.operation_node.envelope.clone();
        Ok(InputIn05ReasoningSeed {
            session_bound,
            operation,
        })
    }
}
