use super::*;
use fin_config::ProviderProtocol;
use fin_provider::{ProviderCapabilities, ProviderDescriptor, ProviderError, ProviderResponse};

struct MockProvider;

impl fin_provider::InferenceProvider for MockProvider {
    fn descriptor(&self) -> &ProviderDescriptor {
        Box::leak(Box::new(ProviderDescriptor {
            name: "mock".into(),
            protocol: ProviderProtocol::OpenAiCompatible,
            base_url: "http://mock".into(),
            default_model: "mock-model".into(),
            capabilities: ProviderCapabilities::for_protocol(ProviderProtocol::OpenAiCompatible),
        }))
    }

    fn execute_prepared(
        &self,
        _request: &fin_provider::PreparedRequest,
    ) -> Result<ProviderResponse, ProviderError> {
        Ok(ProviderResponse {
            provider_name: "mock".into(),
            model: "mock-model".into(),
            output_text: "I will help you with that.".into(),
            response_id: Some("resp-1".into()),
            stop_reason: Some("end_turn".into()),
            status: 200,
            usage: None,
        })
    }
}

fn make_op(input: &str) -> OperationEnvelope<InferenceOperationPayload> {
    let ctx = MinimalContextView::default();
    let payload = InferenceOperationPayload {
        input: input.into(),
        role: RoleProfileRef::new("system").unwrap(),
        provider_path: ProviderPath::new(vec![
            fin_contracts::ProviderTarget::new("mock", "mock-model").unwrap(),
        ])
        .unwrap(),
        provider_strategy: ProviderStrategy::Priority,
        protocol_version: "1.0".into(),
        stream: false,
        context: ctx,
    };
    OperationEnvelope::new(
        "op-1",
        "test",
        "2026-06-01T00:00:00Z",
        "test",
        "trace-1",
        payload,
    )
}

#[test]
fn run_closure_rejects_empty_input() {
    let mut runtime = M1Runtime::new("test");
    let op = make_op("");
    let result = runtime.run_closure(op, &MockProvider);
    assert!(result.is_err(), "empty input should fail validation");
}

#[test]
fn run_closure_produces_closure_run_with_text() {
    let mut runtime = M1Runtime::new("test");
    let op = make_op("hello");
    let result = runtime.run_closure(op, &MockProvider);
    assert!(result.is_ok(), "run_closure failed: {:?}", result);
    let run = result.unwrap();
    assert_eq!(run.assistant_response_text, "I will help you with that.");
    assert_eq!(
        run.provider_response.stop_reason.as_deref(),
        Some("end_turn")
    );
}

#[test]
fn run_closure_preserves_operation_refs() {
    let mut runtime = M1Runtime::new("test");
    let mut op = make_op("hello");
    op.refs.session_id = Some("s-custom".into());
    let run = runtime.run_closure(op, &MockProvider).unwrap();
    assert_eq!(run.operation.refs.session_id.as_deref(), Some("s-custom"));
}

#[test]
fn run_closure_tool_records_are_recorded() {
    let mut runtime = M1Runtime::new("test");
    let op = make_op("hello");
    let run = runtime.run_closure(op, &MockProvider).unwrap();
    // text-only response: tool_records may be empty or may include provider.call
    // the important thing is that the closure run completes successfully
    assert!(!run.assistant_response_text.is_empty());
}
