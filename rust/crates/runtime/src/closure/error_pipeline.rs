use crate::pipeline::error::{
    ErrorErr01DetectedBuilder, ErrorErr02SourceClassifiedBuilder,
    ErrorErr03RuntimeClassifiedBuilder, ErrorErr04SessionRecordedBuilder,
    ErrorErr05UserVisibleBuilder, classify_runtime_error,
};
use crate::{InferenceOperationPayload, OperationEnvelope, RuntimeError};

pub fn map_runtime_error_through_error_pipeline(
    operation: &OperationEnvelope<InferenceOperationPayload>,
    err: &RuntimeError,
) -> crate::pipeline::error::ErrorErr05UserVisible {
    let detected = ErrorErr01DetectedBuilder
        .build(
            "M1Runtime::run_closure",
            err.to_string(),
            operation.submitted_at.clone(),
        )
        .expect("error fact non-empty");
    let (source_class, decision) = classify_runtime_error(err);
    let classified = ErrorErr02SourceClassifiedBuilder.build(detected, source_class);
    let runtime_classified = ErrorErr03RuntimeClassifiedBuilder.build(classified, decision);
    let recorded = ErrorErr04SessionRecordedBuilder
        .build(
            runtime_classified,
            format!("err-{}", operation.operation_id),
            format!("sessions/ledger/{}.jsonl", operation.operation_id),
        )
        .expect("error record ids non-empty");
    ErrorErr05UserVisibleBuilder
        .build(recorded, "runtime failure: see session ledger for trace")
        .expect("user message non-empty")
}
