use super::*;
use fin_config::{
    ConfigMapper, ProviderCredential, ProviderProtocol, ResolvedProviderConfig, UserConfig,
    UserProviderConfig,
};
use fin_provider::{PreparedRequest, ProviderDescriptor, ProviderRequest, ProviderResponse};
use std::{
    collections::BTreeMap,
    fs,
    path::PathBuf,
    sync::{Arc, Mutex},
    time::{SystemTime, UNIX_EPOCH},
};

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
        runtime: fin_config::UserRuntimeConfig::default(),
    };
    let system = ConfigMapper::map_user_to_system(&user).expect("mapping should succeed");
    WorkerRuntime::from_system(&system, "agent-1", "worker-1", "runtime", None)
        .expect("worker runtime")
}

fn temp_runtime_home(prefix: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock should work")
        .as_nanos();
    std::env::temp_dir().join(format!(
        "fin-runtime-{prefix}-{}-{nanos}",
        std::process::id()
    ))
}

#[derive(Debug, Clone)]
struct ContractRetryProvider {
    descriptor: ProviderDescriptor,
    requests: Arc<Mutex<Vec<PreparedRequest>>>,
}

impl ContractRetryProvider {
    fn new() -> Self {
        Self {
            descriptor: ProviderDescriptor::from_resolved(&ResolvedProviderConfig {
                name: "openai".into(),
                protocol: ProviderProtocol::OpenAiCompatible,
                base_url: "https://api.example.com/v1".into(),
                model: "gpt-5".into(),
                credential: ProviderCredential::ApiKeyEnv {
                    env_var: "OPENAI_API_KEY".into(),
                },
                user_agent: None,
                headers: BTreeMap::new(),
            }),
            requests: Arc::new(Mutex::new(Vec::new())),
        }
    }

    fn captured_requests(&self) -> Vec<PreparedRequest> {
        self.requests.lock().expect("requests lock").clone()
    }
}

impl InferenceProvider for ContractRetryProvider {
    fn descriptor(&self) -> &ProviderDescriptor {
        &self.descriptor
    }

    fn prepare_request(&self, request: &ProviderRequest) -> PreparedRequest {
        self.descriptor.prepare_request(request)
    }

    fn execute_prepared(
        &self,
        request: &PreparedRequest,
    ) -> Result<ProviderResponse, fin_provider::ProviderError> {
        let mut requests = self.requests.lock().expect("requests lock");
        requests.push(request.clone());
        let output_text = if requests.len() == 1 {
            "<fin_user_response>done</fin_user_response>\n<fin_control_feedback>{\"origin\":\"model_output_contract_v1\",\"is_continuation\":false,\"is_simple_query\":true,\"candidate_task_id\":null,\"candidate_topic_thread_id\":null,\"continuity_confidence\":20,\"topic_shift_confidence\":80,\"simple_query_confidence\":95,\"previous_topic_summary\":\"prev\",\"current_topic_summary\":\"current\",\"note_candidate\":\"done\",\"digest_candidate\":\"done\",\"reason\":\"done\"}</fin_control_feedback>\n<fin_tool_calls>\n[{\"tool_name\":\"reasoning.stop\",\"arguments\":{\"summary\":\"truncated".to_string()
        } else {
            "<fin_user_response>done</fin_user_response>\n<fin_control_feedback>{\"origin\":\"model_output_contract_v1\",\"is_continuation\":false,\"is_simple_query\":true,\"candidate_task_id\":null,\"candidate_topic_thread_id\":null,\"continuity_confidence\":20,\"topic_shift_confidence\":80,\"simple_query_confidence\":95,\"previous_topic_summary\":\"prev\",\"current_topic_summary\":\"current\",\"note_candidate\":\"done\",\"digest_candidate\":\"done\",\"reason\":\"done\"}</fin_control_feedback>\n<fin_tool_calls>[{\"tool_name\":\"reasoning.stop\",\"arguments\":{\"summary\":\"fixed after retry\"}}]</fin_tool_calls>".to_string()
        };
        Ok(ProviderResponse {
            provider_name: request.provider_name.clone(),
            model: request.model.clone(),
            output_text,
            response_id: Some("contract-retry-response".into()),
            stop_reason: Some("end_turn".into()),
            status: 200,
        })
    }
}

#[derive(Debug, Clone)]
struct ContractRetryLimitProvider {
    descriptor: ProviderDescriptor,
    requests: Arc<Mutex<Vec<PreparedRequest>>>,
}

impl ContractRetryLimitProvider {
    fn new() -> Self {
        Self {
            descriptor: ProviderDescriptor::from_resolved(&ResolvedProviderConfig {
                name: "openai".into(),
                protocol: ProviderProtocol::OpenAiCompatible,
                base_url: "https://api.example.com/v1".into(),
                model: "gpt-5".into(),
                credential: ProviderCredential::ApiKeyEnv {
                    env_var: "OPENAI_API_KEY".into(),
                },
                user_agent: None,
                headers: BTreeMap::new(),
            }),
            requests: Arc::new(Mutex::new(Vec::new())),
        }
    }

    fn captured_requests(&self) -> Vec<PreparedRequest> {
        self.requests.lock().expect("requests lock").clone()
    }
}

impl InferenceProvider for ContractRetryLimitProvider {
    fn descriptor(&self) -> &ProviderDescriptor {
        &self.descriptor
    }

    fn prepare_request(&self, request: &ProviderRequest) -> PreparedRequest {
        self.descriptor.prepare_request(request)
    }

    fn execute_prepared(
        &self,
        request: &PreparedRequest,
    ) -> Result<ProviderResponse, fin_provider::ProviderError> {
        self.requests
            .lock()
            .expect("requests lock")
            .push(request.clone());
        Ok(ProviderResponse {
            provider_name: request.provider_name.clone(),
            model: request.model.clone(),
            output_text: "<fin_user_response>done</fin_user_response>\n<fin_control_feedback>{\"origin\":\"model_output_contract_v1\",\"is_continuation\":false,\"is_simple_query\":true,\"candidate_task_id\":null,\"candidate_topic_thread_id\":null,\"continuity_confidence\":20,\"topic_shift_confidence\":80,\"simple_query_confidence\":95,\"previous_topic_summary\":\"prev\",\"current_topic_summary\":\"current\",\"note_candidate\":\"done\",\"digest_candidate\":\"done\",\"reason\":\"done\"}</fin_control_feedback>\n<fin_tool_calls>\n[{\"tool_name\":\"reasoning.stop\",\"arguments\":{\"summary\":\"truncated".into(),
            response_id: Some("contract-retry-limit-response".into()),
            stop_reason: Some("end_turn".into()),
            status: 200,
        })
    }
}

#[test]
fn runtime_retries_invalid_output_contract_and_recovers() {
    let mut runtime = M1Runtime::default();
    let worker = worker_runtime();
    let provider = ContractRetryProvider::new();
    let operation = InferenceOperationBuilder
        .build(
            &worker,
            InferenceRequest {
                operation_id: "op-contract-retry".into(),
                trace_id: "trace-contract-retry".into(),
                submitted_at: "2026-04-21T21:00:00+08:00".into(),
                refs: EntityRefs {
                    session_id: Some("session-contract-retry".into()),
                    task_id: Some("task-contract-retry".into()),
                    ..EntityRefs::default()
                },
                input: "finish the turn".into(),
                context: MinimalContextView::default(),
            },
        )
        .expect("operation");

    let run = runtime
        .run_closure(operation, &provider)
        .expect("closure should run");
    let requests = provider.captured_requests();
    assert_eq!(requests.len(), 2);
    assert!(
        requests[1]
            .input
            .contains("did not satisfy the fin structured output contract")
    );
    assert!(
        requests[1]
            .input
            .contains("detected <fin_tool_calls> but it is not executable")
    );
    assert!(
        run.events
            .iter()
            .any(|event| event.event_type == "model.output_contract_retry_requested")
    );
    assert!(
        run.events
            .iter()
            .any(|event| event.event_type == "model.output_contract_retry_succeeded")
    );
    assert_eq!(run.provider_request_records.len(), 2);
    assert_eq!(run.provider_response_records.len(), 2);
    assert_eq!(
        run.provider_request_records
            .iter()
            .map(|record| (
                record.round_index,
                record.attempt_index,
                record.request_id.as_str()
            ))
            .collect::<Vec<_>>(),
        vec![
            (1, 1, "provider-request-op-contract-retry-r01-a01"),
            (1, 2, "provider-request-op-contract-retry-r01-a02"),
        ]
    );
    assert_eq!(
        run.provider_response_records
            .iter()
            .map(|record| (
                record.round_index,
                record.attempt_index,
                record.response_record_id.as_str()
            ))
            .collect::<Vec<_>>(),
        vec![
            (1, 1, "provider-response-op-contract-retry-r01-a01"),
            (1, 2, "provider-response-op-contract-retry-r01-a02"),
        ]
    );
    assert_eq!(run.round_records.len(), 1);
    assert_eq!(
        run.round_records[0].request_id,
        "provider-request-op-contract-retry-r01-a02"
    );
    assert!(run.step_records.iter().any(|step| {
        step.step_kind == "model_parse"
            && step.summary.contains("round 1 attempt 1")
            && step.summary.contains("accepted=false")
    }));
    assert!(run.step_records.iter().any(|step| {
        step.step_kind == "model_parse"
            && step.summary.contains("round 1 attempt 2")
            && step.summary.contains("accepted=true")
    }));
    assert!(
        run.tool_records
            .iter()
            .any(|record| record.tool_name == "reasoning.stop" && record.status == "completed")
    );
}

#[test]
fn runtime_stops_after_contract_retry_limit() {
    let mut runtime = M1Runtime::default();
    let worker = worker_runtime();
    let provider = ContractRetryLimitProvider::new();
    let operation = InferenceOperationBuilder
        .build(
            &worker,
            InferenceRequest {
                operation_id: "op-contract-retry-limit".into(),
                trace_id: "trace-contract-retry-limit".into(),
                submitted_at: "2026-04-21T21:10:00+08:00".into(),
                refs: EntityRefs {
                    session_id: Some("session-contract-retry-limit".into()),
                    task_id: Some("task-contract-retry-limit".into()),
                    ..EntityRefs::default()
                },
                input: "finish the turn".into(),
                context: MinimalContextView::default(),
            },
        )
        .expect("operation");

    let run = runtime
        .run_closure(operation, &provider)
        .expect("closure should run");
    let requests = provider.captured_requests();
    assert_eq!(requests.len(), 4);
    assert!(
        run.events
            .iter()
            .any(|event| event.event_type == "model.output_contract_retry_limit_reached")
    );
    assert_eq!(run.provider_request_records.len(), 4);
    assert_eq!(run.provider_response_records.len(), 4);
    assert_eq!(
        run.provider_request_records
            .iter()
            .map(|record| (record.round_index, record.attempt_index))
            .collect::<Vec<_>>(),
        vec![(1, 1), (1, 2), (1, 3), (1, 4)]
    );
    assert_eq!(
        run.round_records[0].request_id,
        "provider-request-op-contract-retry-limit-r01-a04"
    );
    assert!(
        !run.tool_records
            .iter()
            .any(|record| record.tool_name == "reasoning.stop" && record.status == "completed")
    );
}

#[test]
fn session_materializer_persists_retry_attempt_provider_truth() {
    let mut runtime = M1Runtime::default();
    let worker = worker_runtime();
    let provider = ContractRetryProvider::new();
    let operation = InferenceOperationBuilder
        .build(
            &worker,
            InferenceRequest {
                operation_id: "op-contract-retry-materialize".into(),
                trace_id: "trace-contract-retry-materialize".into(),
                submitted_at: "2026-04-21T21:20:00+08:00".into(),
                refs: EntityRefs {
                    session_id: Some("session-contract-retry-materialize".into()),
                    task_id: Some("task-contract-retry-materialize".into()),
                    ..EntityRefs::default()
                },
                input: "finish the turn".into(),
                context: MinimalContextView::default(),
            },
        )
        .expect("operation");

    let run = runtime
        .run_closure(operation, &provider)
        .expect("closure should run");
    let runtime_home = temp_runtime_home("retry-materialize");
    fs::create_dir_all(runtime_home.join("runtime/current"))
        .expect("runtime current dir should exist");
    let receipt = SessionMaterializer
        .persist(
            &runtime_home,
            &run,
            &fin_config::RuntimeRetentionConfig::default(),
        )
        .expect("materialization should succeed");

    let current_requests: Vec<ProviderRequestRecord> = serde_json::from_str(
        &fs::read_to_string(runtime_home.join("runtime/current/current_provider_requests.json"))
            .expect("current requests should exist"),
    )
    .expect("current requests should decode");
    let current_responses: Vec<ProviderResponseRecord> = serde_json::from_str(
        &fs::read_to_string(runtime_home.join("runtime/current/current_provider_responses.json"))
            .expect("current responses should exist"),
    )
    .expect("current responses should decode");
    let session_requests: Vec<ProviderRequestRecord> = serde_json::from_str(
        &fs::read_to_string(
            receipt
                .session_dir
                .join("provider/recent_provider_requests.json"),
        )
        .expect("session requests should exist"),
    )
    .expect("session requests should decode");
    let session_responses: Vec<ProviderResponseRecord> = serde_json::from_str(
        &fs::read_to_string(
            receipt
                .session_dir
                .join("provider/recent_provider_responses.json"),
        )
        .expect("session responses should exist"),
    )
    .expect("session responses should decode");

    for records in [&current_requests, &session_requests] {
        assert_eq!(
            records
                .iter()
                .map(|record| (record.round_index, record.attempt_index))
                .collect::<Vec<_>>(),
            vec![(1, 1), (1, 2)]
        );
    }
    for records in [&current_responses, &session_responses] {
        assert_eq!(
            records
                .iter()
                .map(|record| (record.round_index, record.attempt_index))
                .collect::<Vec<_>>(),
            vec![(1, 1), (1, 2)]
        );
    }

    fs::remove_dir_all(&runtime_home).expect("temp runtime home should be cleaned");
}
