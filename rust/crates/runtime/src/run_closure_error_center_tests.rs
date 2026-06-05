use super::*;
use fin_contracts::{
    EntityRefs, InferenceOperationPayload, MinimalContextView, OperationEnvelope, ProviderPath,
    ProviderStrategy, ProviderTarget, RoleProfileRef,
};

fn invalid_payload() -> OperationEnvelope<InferenceOperationPayload> {
    let mut envelope = OperationEnvelope::new(
        String::from("op-test-001"),
        String::from("start_inference"),
        String::from("2026-06-05T00:00:00Z"),
        String::from("test"),
        String::from("trace-test-001"),
        InferenceOperationPayload {
            input: String::from("hello"),
            role: RoleProfileRef::new("system").expect("role"),
            provider_path: ProviderPath::new(vec![ProviderTarget::new("test", "m").expect("t")])
                .expect("path"),
            provider_strategy: ProviderStrategy::Priority,
            protocol_version: String::new(),
            stream: false,
            context: MinimalContextView::default(),
        },
    );
    envelope.refs = EntityRefs::default();
    envelope
}

struct NoopProvider;

impl fin_provider::InferenceProvider for NoopProvider {
    fn descriptor(&self) -> &fin_provider::ProviderDescriptor {
        unimplemented!()
    }
    fn execute_prepared(
        &self,
        _request: &fin_provider::PreparedRequest,
    ) -> Result<fin_provider::ProviderResponse, fin_provider::ProviderError> {
        unimplemented!()
    }
}

#[test]
fn run_closure_routes_failure_to_error_pipeline_and_emits_events() {
    let mut runtime = M1Runtime::new("test");
    let operation = invalid_payload();
    let result: Result<ClosureRun, RuntimeError> =
        runtime.run_closure(operation, &NoopProvider);
    assert!(result.is_err(), "validate failure must surface");
    assert!(
        !runtime.last_error_events.is_empty(),
        "last_error_events must be populated when run_closure fails"
    );
    let kinds: Vec<String> = runtime
        .last_error_events
        .iter()
        .map(|e| e.event_type.clone())
        .collect();
    assert!(
        kinds.iter().any(|k| k == "error.detected"),
        "error.detected must be emitted; got: {kinds:?}"
    );
    assert!(
        kinds.iter().any(|k| k == "error.user_visible_prepared"),
        "error.user_visible_prepared must be emitted; got: {kinds:?}"
    );
}
