//! L3 fault injection tests for the architecture cleanup verification matrix.
//!
//! Injects three failure classes (provider HTTP 500, missing credential, unsupported protocol)
//! and verifies the error chain does not silently salvage into a success truth.

use super::*;
use fin_contracts::{
    EntityRefs, InferenceOperationPayload, MinimalContextView, OperationEnvelope, ProviderPath,
    ProviderStrategy, ProviderTarget, RoleProfileRef,
};
use fin_provider::ProviderError;

fn make_op() -> OperationEnvelope<InferenceOperationPayload> {
    let mut env = OperationEnvelope::new(
        String::from("op-fault"),
        String::from("start_inference"),
        String::from("2026-06-06T00:00:00Z"),
        String::from("test"),
        String::from("trace-fault"),
        InferenceOperationPayload {
            input: "hi".to_string(),
            role: RoleProfileRef::new("system").expect("role"),
            provider_path: ProviderPath::new(vec![ProviderTarget::new("test", "m").expect("t")]).expect("path"),
            provider_strategy: ProviderStrategy::Priority,
            protocol_version: String::new(),
            stream: false,
            context: MinimalContextView::default(),
        },
    );
    env.refs = EntityRefs::default();
    env
}

struct FaultyProvider { err: ProviderError }
impl fin_provider::InferenceProvider for FaultyProvider {
    fn descriptor(&self) -> &fin_provider::ProviderDescriptor { static D: fin_provider::ProviderDescriptor = fin_provider::ProviderDescriptor { name: String::new(), protocol: fin_config::ProviderProtocol::OpenAiCompatible, base_url: String::new(), default_model: String::new(), capabilities: fin_provider::ProviderCapabilities { supports_streaming: false, supports_tool_calls: false } }; &D }
    fn execute_prepared(&self, _: &fin_provider::PreparedRequest) -> Result<fin_provider::ProviderResponse, ProviderError> {
        match &self.err {
            ProviderError::HttpStatus { status, body } => Err(ProviderError::HttpStatus { status: *status, body: body.clone() }),
            ProviderError::MissingCredentialEnv { env_var } => Err(ProviderError::MissingCredentialEnv { env_var: env_var.clone() }),
            ProviderError::UnsupportedProtocol { protocol } => Err(ProviderError::UnsupportedProtocol { protocol: protocol.clone() }),
            ProviderError::Request { message } => Err(ProviderError::Request { message: message.clone() }),
            other => panic!("unexpected error variant: {other:?}"),
        }
    }
}

fn check(m: &M1Runtime, label: &str) {
    let kinds: Vec<&str> = m.last_error_events.iter().map(|e| e.event_type.as_str()).collect();
    assert!(!kinds.is_empty(), "[{label}] error events must be non-empty");
    assert!(kinds.contains(&"error.detected"), "[{label}] error.detected missing");
    assert!(kinds.contains(&"error.user_visible_prepared"), "[{label}] error.user_visible_prepared missing");
}

#[test]
fn l3_provider_500_surfaces_as_err() {
    let mut m = M1Runtime::new("l3-500");
    let p = FaultyProvider { err: ProviderError::HttpStatus { status: 500, body: "test".into() } };
    assert!(m.run_closure(make_op(), &p).is_err());
    check(&m, "500");
}

#[test]
fn l3_missing_credential_surfaces_as_err() {
    let mut m = M1Runtime::new("l3-cred");
    let p = FaultyProvider { err: ProviderError::MissingCredentialEnv { env_var: "KEY".into() } };
    assert!(m.run_closure(make_op(), &p).is_err());
    check(&m, "cred");
}

#[test]
fn l3_unsupported_protocol_surfaces_as_err() {
    let mut m = M1Runtime::new("l3-proto");
    let p = FaultyProvider { err: ProviderError::UnsupportedProtocol { protocol: fin_config::ProviderProtocol::OpenAiCompatible } };
    assert!(m.run_closure(make_op(), &p).is_err());
    check(&m, "proto");
}

#[test]
fn l3_request_failure_surfaces_as_err() {
    let mut m = M1Runtime::new("l3-req");
    let p = FaultyProvider { err: ProviderError::Request { message: "connection timeout".into() } };
    assert!(m.run_closure(make_op(), &p).is_err());
    check(&m, "req");
}
