use super::*;

use fin_config::{
    ConfigMapper, ProviderCredential, ProviderProtocol, ResolvedProviderConfig, UserConfig,
    UserProviderConfig,
};
use fin_provider::{ProviderDescriptor, StaticProviderClient};
use std::collections::BTreeMap;

fn provider() -> StaticProviderClient {
    StaticProviderClient::new(ProviderDescriptor::from_resolved(&ResolvedProviderConfig {
        name: "openai".into(),
        protocol: ProviderProtocol::OpenAiCompatible,
        base_url: "https://api.example.com/v1".into(),
        model: "gpt-5".into(),
        credential: ProviderCredential::ApiKeyEnv {
            env_var: "OPENAI_API_KEY".into(),
        },
        user_agent: None,
        headers: BTreeMap::new(),
    }))
}

fn worker_runtime() -> WorkerRuntime {
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
                user_agent: None,
                headers: BTreeMap::new(),
            },
        )]),
    };
    let system = ConfigMapper::map_user_to_system(&user).expect("mapping should succeed");
    WorkerRuntime::from_system(&system, "agent-1", "worker-1", "runtime", None)
        .expect("worker runtime")
}

#[test]
fn run_closure_emits_expected_event_chain() {
    let mut runtime = M1Runtime::default();
    let worker = worker_runtime();
    let op = InferenceOperationBuilder
        .build(
            &worker,
            InferenceRequest {
                operation_id: "op-1".into(),
                trace_id: "trace-1".into(),
                submitted_at: "2026-04-17T00:00:00Z".into(),
                refs: EntityRefs {
                    session_id: Some("session-1".into()),
                    task_id: Some("task-1".into()),
                    ..EntityRefs::default()
                },
                input: "hello".into(),
                context: MinimalContextView::default(),
            },
        )
        .expect("build operation");

    let run = runtime
        .run_closure(op, &provider())
        .expect("closure should run");
    let kinds: Vec<_> = run.events.iter().map(|e| e.event_type.as_str()).collect();
    assert_eq!(
        kinds,
        vec![
            "operation.accepted",
            "inference.started",
            "provider.operation_accepted",
            "provider.gateway_request_sent",
            "provider.gateway_response_received",
            "provider.response_normalized",
            "provider.completed",
            "progress.updated",
            "execution_note.appended",
            "digest.finalized",
            "operation.completed",
        ]
    );
    assert_eq!(run.prepared_request.model, "gpt-5");
    assert_eq!(run.context_snapshot.operation_id, "op-1");
    assert_eq!(run.context_snapshot.trace_id, "trace-1");
    assert!(run.provider_response.output_text.contains("hello"));
    assert!(run.events.iter().all(|event| event.trace_id == "trace-1"));
    let provider_event = run
        .events
        .iter()
        .find(|event| event.event_type == "provider.completed")
        .expect("provider.completed event should exist");
    let payload: ProviderEventPayload =
        serde_json::from_value(provider_event.payload.clone()).expect("payload should decode");
    assert!(payload.debug.is_some());
}

#[test]
fn inference_builder_carries_runtime_policy_into_operation() {
    let worker = worker_runtime();
    let operation = InferenceOperationBuilder
        .build(
            &worker,
            InferenceRequest {
                operation_id: "op-2".into(),
                trace_id: "trace-2".into(),
                submitted_at: "2026-04-17T00:00:00Z".into(),
                refs: EntityRefs::default(),
                input: "say ok".into(),
                context: MinimalContextView {
                    continuity_tail: vec!["previous".into()],
                    summary: Some("recent continuity".into()),
                },
            },
        )
        .expect("operation");

    assert_eq!(operation.payload.role.role_id.as_str(), "default");
    assert_eq!(
        operation
            .payload
            .provider_path
            .primary_target()
            .provider_name,
        "openai"
    );
    assert_eq!(operation.timeout_ms, Some(60_000));
    assert_eq!(
        operation.payload.context.summary.as_deref(),
        Some("recent continuity")
    );
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
                user_agent: None,
                headers: BTreeMap::new(),
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
                user_agent: None,
                headers: BTreeMap::new(),
            },
        )]),
    };
    let system = ConfigMapper::map_user_to_system(&user).expect("mapping should succeed");

    let runtime =
        WorkerRuntime::from_system(&system, "agent-project-leader", "worker-1", "runtime", None)
            .expect("worker runtime should build");

    assert_eq!(runtime.agent_id.as_str(), "agent-project-leader");
    assert_eq!(runtime.worker_id, "worker-1");
    assert_eq!(runtime.policy.provider_path.primary_target().model, "gpt-5");
}

#[test]
fn run_closure_renders_context_into_provider_input() {
    let mut runtime = M1Runtime::default();
    let worker = worker_runtime();
    let op = InferenceOperationBuilder
        .build(
            &worker,
            InferenceRequest {
                operation_id: "op-context".into(),
                trace_id: "trace-context".into(),
                submitted_at: "2026-04-17T00:00:00Z".into(),
                refs: EntityRefs::default(),
                input: "answer current turn".into(),
                context: MinimalContextView {
                    continuity_tail: vec!["first".into(), "second".into()],
                    summary: Some("carry previous state".into()),
                },
            },
        )
        .expect("build operation");

    let run = runtime
        .run_closure(op, &provider())
        .expect("closure should run");
    assert!(
        run.prepared_request
            .rendered_input
            .contains("Context summary:")
    );
    assert!(
        run.prepared_request
            .rendered_input
            .contains("carry previous state")
    );
    assert!(
        run.prepared_request
            .rendered_input
            .contains("Continuity tail:")
    );
    assert!(run.prepared_request.rendered_input.contains(
        "Current user input:
answer current turn"
    ));
}