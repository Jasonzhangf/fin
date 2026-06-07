use crate::pipeline::error::ErrorErr05UserVisible;
use crate::{EntityRefs, InferenceOperationPayload, OperationEnvelope, RuntimeError};
use fin_contracts::EventEnvelope;
use serde_json::Value;

use super::closure_runtime::M1Runtime;

pub(super) fn build_runtime_error_events(
    operation: &OperationEnvelope<InferenceOperationPayload>,
    user_visible: &ErrorErr05UserVisible,
    err: &RuntimeError,
    mut emit: impl FnMut(
        &str,
        &str,
        &str,
        &EntityRefs,
        Option<String>,
        Value,
    ) -> Result<EventEnvelope<Value>, RuntimeError>,
) -> Vec<EventEnvelope<Value>> {
    let mut events = Vec::new();
    let refs = operation.refs.clone();
    let trace_id = operation.trace_id.clone();
    let operation_id = operation.operation_id.clone();
    let submitted_at = operation.submitted_at.clone();
    let detected = &user_visible.recorded.classified.classified.detected;
    let source_class_json =
        serde_json::to_value(&user_visible.recorded.classified.classified.source_class)
            .ok()
            .and_then(|v| v.as_str().map(|s| s.to_string()))
            .unwrap_or_else(|| "Runtime".to_string());
    let decision_json = serde_json::to_value(&user_visible.recorded.classified.decision).ok();
    let payload_detected = serde_json::json!({
        "source_node": detected.source_node,
        "fact": detected.fact,
        "captured_at": detected.captured_at,
        "source_class": source_class_json,
        "decision": decision_json,
        "error_message": err.to_string(),
        "ledger_path": user_visible.recorded.ledger_path,
        "recorded_event_id": user_visible.recorded.recorded_event_id,
    });
    if let Ok(ev) = emit(
        "error.detected",
        &trace_id,
        &submitted_at,
        &refs,
        Some(operation_id.clone()),
        payload_detected,
    ) {
        events.push(ev);
    }
    let payload_user = serde_json::json!({
        "user_message": user_visible.user_message,
        "safe_for_channel": user_visible.safe_for_channel,
    });
    if let Ok(ev) = emit(
        "error.user_visible_prepared",
        &trace_id,
        &submitted_at,
        &refs,
        Some(operation_id),
        payload_user,
    ) {
        events.push(ev);
    }
    events
}

impl M1Runtime {
    pub(super) fn emit_error_events(
        &mut self,
        operation: &OperationEnvelope<InferenceOperationPayload>,
        user_visible: &ErrorErr05UserVisible,
        err: &RuntimeError,
    ) {
        let events = build_runtime_error_events(
            operation,
            user_visible,
            err,
            |event_type, trace_id, occurred_at, refs, operation_id, payload| {
                self.event(
                    event_type,
                    trace_id,
                    occurred_at,
                    refs,
                    operation_id,
                    payload,
                )
            },
        );
        self.last_error_events.extend(events);
    }
}
