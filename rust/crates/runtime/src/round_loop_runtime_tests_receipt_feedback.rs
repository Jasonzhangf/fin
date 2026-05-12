use super::*;
use fin_config::{
    ConfigMapper, ProviderCredential, ProviderProtocol, ResolvedProviderConfig, UserConfig,
    UserProviderConfig,
};
use fin_contracts::{EntityRefs, MinimalContextView, ProjectContextBlock};
use fin_provider::{PreparedRequest, ProviderDescriptor, ProviderResponse};
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

fn full_history_context(runtime_home: &PathBuf, cwd: &PathBuf) -> MinimalContextView {
    MinimalContextView {
        project: Some(ProjectContextBlock {
            runtime_home: Some(runtime_home.display().to_string()),
            cwd: Some(cwd.display().to_string()),
            project_root: Some(cwd.display().to_string()),
            ..Default::default()
        }),
        ..Default::default()
    }
}

#[derive(Debug, Clone)]
struct TwoRoundFailureProvider {
    descriptor: ProviderDescriptor,
    requests: Arc<Mutex<Vec<PreparedRequest>>>,
    first_round_output: String,
}

impl TwoRoundFailureProvider {
    fn new(first_round_output: impl Into<String>) -> Self {
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
            first_round_output: first_round_output.into(),
        }
    }

    fn captured_requests(&self) -> Vec<PreparedRequest> {
        self.requests.lock().expect("requests lock").clone()
    }
}

impl InferenceProvider for TwoRoundFailureProvider {
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
        let output_text = if request.input.starts_with("Continue the same turn.") {
            "<fin_user_response>已看到失败回执，停止。</fin_user_response>\n<fin_control_feedback>{\"origin\":\"model_output_contract_v1\",\"is_continuation\":true,\"is_simple_query\":false,\"task_completed\":true,\"is_simple_chat\":false,\"blocked\":false,\"needs_user_involve\":false,\"candidate_task_id\":\"task-failure-feedback\",\"candidate_topic_thread_id\":\"topic-failure-feedback\",\"continuity_confidence\":95,\"topic_shift_confidence\":5,\"simple_query_confidence\":4,\"previous_topic_summary\":\"failed exec receipt\",\"current_topic_summary\":\"failed exec receipt\",\"completion_evidence\":[\"failure receipt rendered into current tool history\",\"model saw retry guidance\"],\"final_conclusions\":[\"tool failure was visible to the model\",\"turn may close\"],\"blocked_reason\":null,\"what_needs_to_be_done_by_user\":null,\"note_candidate\":\"failure receipt visible\",\"digest_candidate\":\"failure receipt visible\",\"reason\":\"failure receipt inspected\"}</fin_control_feedback>\n<fin_tool_calls>[{\"tool_name\":\"reasoning.stop\",\"arguments\":{\"summary\":\"failure receipt inspected\"}}]</fin_tool_calls>".to_string()
        } else {
            self.first_round_output.clone()
        };
        Ok(ProviderResponse {
            provider_name: request.provider_name.clone(),
            model: request.model.clone(),
            output_text,
            response_id: Some("two-round-failure-provider".into()),
            stop_reason: Some(
                if request.input.starts_with("Continue the same turn.") {
                    "end_turn"
                } else {
                    "tool_use"
                }
                .into(),
            ),
            status: 200,
            tool_calls: Vec::new(),
        })
    }
}

#[test]
fn runtime_followup_round_renders_failure_feedback_from_exec_receipt() {
    let runtime_home = temp_runtime_home("failure-feedback");
    let cwd = runtime_home.join("workspace");
    fs::create_dir_all(&cwd).expect("workspace");

    let mut runtime = M1Runtime::default();
    let worker = worker_runtime();
    let provider = TwoRoundFailureProvider::new(
        "<fin_user_response>先执行一个会失败的命令。</fin_user_response>\n<fin_control_feedback>{\"origin\":\"model_output_contract_v1\",\"is_continuation\":true,\"is_simple_query\":false,\"task_completed\":false,\"is_simple_chat\":false,\"blocked\":false,\"needs_user_involve\":false,\"candidate_task_id\":\"task-failure-feedback\",\"candidate_topic_thread_id\":\"topic-failure-feedback\",\"continuity_confidence\":90,\"topic_shift_confidence\":10,\"simple_query_confidence\":4,\"previous_topic_summary\":\"failed exec receipt\",\"current_topic_summary\":\"failed exec receipt\",\"completion_evidence\":[],\"final_conclusions\":[],\"blocked_reason\":null,\"what_needs_to_be_done_by_user\":null,\"note_candidate\":\"run failed exec first\",\"digest_candidate\":\"run failed exec first\",\"reason\":\"need failure receipt evidence\"}</fin_control_feedback>\n<fin_tool_calls>[{\"tool_name\":\"exec_command\",\"arguments\":{\"cmd\":\"exit 7\",\"cwd\":\"WORKDIR\"}}]</fin_tool_calls>"
            .replace("WORKDIR", &cwd.display().to_string()),
    );

    let operation = InferenceOperationBuilder
        .build(
            &worker,
            InferenceRequest {
                operation_id: "op-failure-feedback".into(),
                trace_id: "trace-failure-feedback".into(),
                submitted_at: "2026-04-23T23:50:00+08:00".into(),
                refs: EntityRefs {
                    session_id: Some("session-failure-feedback".into()),
                    task_id: Some("task-failure-feedback".into()),
                    ..EntityRefs::default()
                },
                input: "先执行失败命令再收口".into(),
                context: full_history_context(&runtime_home, &cwd),
            },
        )
        .expect("operation");

    let _run = runtime
        .run_closure(operation, &provider)
        .expect("closure should run");
    let requests = provider.captured_requests();
    assert_eq!(requests.len(), 2);
    let second = &requests[1];
    assert!(second.rendered_input.contains("tool=exec_command"));
    assert!(second.rendered_input.contains("authoritative_receipt:"));
    assert!(second.rendered_input.contains("\"status\": \"failed\""));
    assert!(
        second
            .rendered_input
            .contains("\"failure_kind\": \"command_non_zero_exit\"")
    );
    assert!(second.rendered_input.contains("\"retry_hint\":"));
    assert!(second.rendered_input.contains("stdout/stderr"));
}

#[test]
fn runtime_followup_round_renders_failure_feedback_from_peer_receipt() {
    let runtime_home = temp_runtime_home("peer-failure-feedback");
    let cwd = runtime_home.join("workspace");
    fs::create_dir_all(&cwd).expect("workspace");

    let mut runtime = M1Runtime::default();
    let worker = worker_runtime();
    let provider = TwoRoundFailureProvider::new(
        "<fin_user_response>先检查一个不存在的 peer。</fin_user_response>\n<fin_control_feedback>{\"origin\":\"model_output_contract_v1\",\"is_continuation\":true,\"is_simple_query\":false,\"task_completed\":false,\"is_simple_chat\":false,\"blocked\":false,\"needs_user_involve\":false,\"candidate_task_id\":\"task-peer-failure-feedback\",\"candidate_topic_thread_id\":\"topic-peer-failure-feedback\",\"continuity_confidence\":90,\"topic_shift_confidence\":10,\"simple_query_confidence\":4,\"previous_topic_summary\":\"failed peer receipt\",\"current_topic_summary\":\"failed peer receipt\",\"completion_evidence\":[],\"final_conclusions\":[],\"blocked_reason\":null,\"what_needs_to_be_done_by_user\":null,\"note_candidate\":\"run failed peer describe first\",\"digest_candidate\":\"run failed peer describe first\",\"reason\":\"need peer failure receipt evidence\"}</fin_control_feedback>\n<fin_tool_calls>[{\"tool_name\":\"peer.describe\",\"arguments\":{\"peer_id\":\"peer-missing\"}}]</fin_tool_calls>",
    );

    let mut context = full_history_context(&runtime_home, &cwd);
    context.peer = Some(fin_contracts::PeerContextBlock {
        peers: vec![fin_contracts::PeerDescriptorSummary {
            peer_id: "peer-worker-a".into(),
            label: "Worker A".into(),
            peer_kind: "project_worker".into(),
            presence_state: "idle".into(),
            health_state: Some("healthy".into()),
            capability_ids: vec!["mailbox.poll".into()],
            supports_session_binding: true,
            supports_agentic_execution: true,
        }],
        ..Default::default()
    });

    let operation = InferenceOperationBuilder
        .build(
            &worker,
            InferenceRequest {
                operation_id: "op-peer-failure-feedback".into(),
                trace_id: "trace-peer-failure-feedback".into(),
                submitted_at: "2026-04-24T00:25:00+08:00".into(),
                refs: EntityRefs {
                    session_id: Some("session-peer-failure-feedback".into()),
                    task_id: Some("task-peer-failure-feedback".into()),
                    ..EntityRefs::default()
                },
                input: "先检查不存在的 peer 再收口".into(),
                context,
            },
        )
        .expect("operation");

    let _run = runtime
        .run_closure(operation, &provider)
        .expect("closure should run");
    let requests = provider.captured_requests();
    assert_eq!(requests.len(), 2);
    let second = &requests[1];
    assert!(second.rendered_input.contains("tool=peer.describe"));
    assert!(second.rendered_input.contains("authoritative_receipt:"));
    assert!(
        second
            .rendered_input
            .contains("\"failure_kind\": \"missing_context_target\"")
    );
    assert!(second.rendered_input.contains("\"retry_hint\":"));
    assert!(second.rendered_input.contains("peer.list"));
}

#[test]
fn runtime_followup_round_renders_failure_feedback_from_presence_receipt() {
    let runtime_home = temp_runtime_home("presence-failure-feedback");
    let cwd = runtime_home.join("workspace");
    fs::create_dir_all(&cwd).expect("workspace");

    let mut runtime = M1Runtime::default();
    let worker = worker_runtime();
    let provider = TwoRoundFailureProvider::new(
        "<fin_user_response>先检查 agent presence。</fin_user_response>\n<fin_control_feedback>{\"origin\":\"model_output_contract_v1\",\"is_continuation\":true,\"is_simple_query\":false,\"task_completed\":false,\"is_simple_chat\":false,\"blocked\":false,\"needs_user_involve\":false,\"candidate_task_id\":\"task-presence-failure-feedback\",\"candidate_topic_thread_id\":\"topic-presence-failure-feedback\",\"continuity_confidence\":90,\"topic_shift_confidence\":10,\"simple_query_confidence\":4,\"previous_topic_summary\":\"failed presence receipt\",\"current_topic_summary\":\"failed presence receipt\",\"completion_evidence\":[],\"final_conclusions\":[],\"blocked_reason\":null,\"what_needs_to_be_done_by_user\":null,\"note_candidate\":\"run failed presence query first\",\"digest_candidate\":\"run failed presence query first\",\"reason\":\"need presence failure receipt evidence\"}</fin_control_feedback>\n<fin_tool_calls>[{\"tool_name\":\"agent.presence.list\",\"arguments\":{\"limit\":5}}]</fin_tool_calls>",
    );

    let operation = InferenceOperationBuilder
        .build(
            &worker,
            InferenceRequest {
                operation_id: "op-presence-failure-feedback".into(),
                trace_id: "trace-presence-failure-feedback".into(),
                submitted_at: "2026-04-24T00:35:00+08:00".into(),
                refs: EntityRefs {
                    session_id: Some("session-presence-failure-feedback".into()),
                    task_id: Some("task-presence-failure-feedback".into()),
                    ..EntityRefs::default()
                },
                input: "先检查 agent presence 再收口".into(),
                context: full_history_context(&runtime_home, &cwd),
            },
        )
        .expect("operation");

    let _run = runtime
        .run_closure(operation, &provider)
        .expect("closure should run");
    let requests = provider.captured_requests();
    assert_eq!(requests.len(), 2);
    let second = &requests[1];
    assert!(second.rendered_input.contains("tool=agent.presence.list"));
    assert!(second.rendered_input.contains("authoritative_receipt:"));
    assert!(
        second
            .rendered_input
            .contains("\"failure_kind\": \"missing_runtime_snapshot\"")
    );
    assert!(second.rendered_input.contains("\"retry_hint\":"));
    assert!(
        second
            .rendered_input
            .contains("current_agent_presence_registry.json")
    );
}

#[test]
fn runtime_followup_round_renders_failure_feedback_from_task_status_receipt() {
    let runtime_home = temp_runtime_home("task-status-failure-feedback");
    let cwd = runtime_home.join("workspace");
    fs::create_dir_all(&cwd).expect("workspace");

    let mut runtime = M1Runtime::default();
    let worker = worker_runtime();
    let provider = TwoRoundFailureProvider::new(
        "<fin_user_response>先检查一个不存在的任务。</fin_user_response>\n<fin_control_feedback>{\"origin\":\"model_output_contract_v1\",\"is_continuation\":true,\"is_simple_query\":false,\"task_completed\":false,\"is_simple_chat\":false,\"blocked\":false,\"needs_user_involve\":false,\"candidate_task_id\":\"task-task-status-failure-feedback\",\"candidate_topic_thread_id\":\"topic-task-status-failure-feedback\",\"continuity_confidence\":90,\"topic_shift_confidence\":10,\"simple_query_confidence\":4,\"previous_topic_summary\":\"failed task status receipt\",\"current_topic_summary\":\"failed task status receipt\",\"completion_evidence\":[],\"final_conclusions\":[],\"blocked_reason\":null,\"what_needs_to_be_done_by_user\":null,\"note_candidate\":\"run failed task status first\",\"digest_candidate\":\"run failed task status first\",\"reason\":\"need task status failure receipt evidence\"}</fin_control_feedback>\n<fin_tool_calls>[{\"tool_name\":\"project.task.status\",\"arguments\":{\"task_id\":\"task-missing@session-missing\"}}]</fin_tool_calls>",
    );

    let operation = InferenceOperationBuilder
        .build(
            &worker,
            InferenceRequest {
                operation_id: "op-task-status-failure-feedback".into(),
                trace_id: "trace-task-status-failure-feedback".into(),
                submitted_at: "2026-04-24T00:45:00+08:00".into(),
                refs: EntityRefs {
                    session_id: Some("session-task-status-failure-feedback".into()),
                    task_id: Some("task-task-status-failure-feedback".into()),
                    ..EntityRefs::default()
                },
                input: "先检查不存在的 task 再收口".into(),
                context: full_history_context(&runtime_home, &cwd),
            },
        )
        .expect("operation");

    let _run = runtime
        .run_closure(operation, &provider)
        .expect("closure should run");
    let requests = provider.captured_requests();
    assert_eq!(requests.len(), 2);
    let second = &requests[1];
    assert!(second.rendered_input.contains("tool=project.task.status"));
    assert!(second.rendered_input.contains("authoritative_receipt:"));
    assert!(
        second
            .rendered_input
            .contains("\"failure_kind\": \"missing_runtime_target\"")
    );
    assert!(second.rendered_input.contains("\"retry_hint\":"));
    assert!(second.rendered_input.contains("valid existing target id"));
}

#[test]
fn runtime_followup_round_renders_failure_feedback_from_capability_receipt() {
    let runtime_home = temp_runtime_home("capability-failure-feedback");
    let cwd = runtime_home.join("workspace");
    fs::create_dir_all(&cwd).expect("workspace");

    let mut runtime = M1Runtime::default();
    let worker = worker_runtime();
    let provider = TwoRoundFailureProvider::new(
        "<fin_user_response>先调用一个不存在的 capability。</fin_user_response>\n<fin_control_feedback>{\"origin\":\"model_output_contract_v1\",\"is_continuation\":true,\"is_simple_query\":false,\"task_completed\":false,\"is_simple_chat\":false,\"blocked\":false,\"needs_user_involve\":false,\"candidate_task_id\":\"task-capability-failure-feedback\",\"candidate_topic_thread_id\":\"topic-capability-failure-feedback\",\"continuity_confidence\":90,\"topic_shift_confidence\":10,\"simple_query_confidence\":4,\"previous_topic_summary\":\"failed capability receipt\",\"current_topic_summary\":\"failed capability receipt\",\"completion_evidence\":[],\"final_conclusions\":[],\"blocked_reason\":null,\"what_needs_to_be_done_by_user\":null,\"note_candidate\":\"run failed capability invoke first\",\"digest_candidate\":\"run failed capability invoke first\",\"reason\":\"need capability failure receipt evidence\"}</fin_control_feedback>\n<fin_tool_calls>[{\"tool_name\":\"capability.invoke\",\"arguments\":{\"capability_id\":\"capability.missing\",\"input\":{\"task\":\"sample\"}}}]</fin_tool_calls>",
    );

    let mut context = full_history_context(&runtime_home, &cwd);
    context.peer = Some(fin_contracts::PeerContextBlock {
        peers: vec![fin_contracts::PeerDescriptorSummary {
            peer_id: "peer-worker-a".into(),
            label: "Worker A".into(),
            peer_kind: "project_worker".into(),
            presence_state: "idle".into(),
            health_state: Some("healthy".into()),
            capability_ids: vec!["capability.exec".into()],
            supports_session_binding: true,
            supports_agentic_execution: true,
        }],
        ..Default::default()
    });

    let operation = InferenceOperationBuilder
        .build(
            &worker,
            InferenceRequest {
                operation_id: "op-capability-failure-feedback".into(),
                trace_id: "trace-capability-failure-feedback".into(),
                submitted_at: "2026-04-24T00:50:00+08:00".into(),
                refs: EntityRefs {
                    session_id: Some("session-capability-failure-feedback".into()),
                    task_id: Some("task-capability-failure-feedback".into()),
                    ..EntityRefs::default()
                },
                input: "先检查不存在的 capability 再收口".into(),
                context,
            },
        )
        .expect("operation");

    let _run = runtime
        .run_closure(operation, &provider)
        .expect("closure should run");
    let requests = provider.captured_requests();
    assert_eq!(requests.len(), 2);
    let second = &requests[1];
    assert!(second.rendered_input.contains("tool=capability.invoke"));
    assert!(second.rendered_input.contains("authoritative_receipt:"));
    assert!(
        second
            .rendered_input
            .contains("\"failure_kind\": \"missing_context_capability\"")
    );
    assert!(second.rendered_input.contains("\"retry_hint\":"));
    assert!(second.rendered_input.contains("peer.list"));
    assert!(second.rendered_input.contains("peer.describe"));
}

#[test]
fn runtime_followup_round_renders_failure_feedback_from_assign_receipt() {
    let runtime_home = temp_runtime_home("assign-failure-feedback");
    let cwd = runtime_home.join("workspace");
    fs::create_dir_all(&cwd).expect("workspace");

    let mut runtime = M1Runtime::default();
    let worker = worker_runtime();
    let provider = TwoRoundFailureProvider::new(
        "<fin_user_response>先尝试派发，但故意漏参数。</fin_user_response>\n<fin_control_feedback>{\"origin\":\"model_output_contract_v1\",\"is_continuation\":true,\"is_simple_query\":false,\"task_completed\":false,\"is_simple_chat\":false,\"blocked\":false,\"needs_user_involve\":false,\"candidate_task_id\":\"task-assign-failure-feedback\",\"candidate_topic_thread_id\":\"topic-assign-failure-feedback\",\"continuity_confidence\":90,\"topic_shift_confidence\":10,\"simple_query_confidence\":4,\"previous_topic_summary\":\"failed assign receipt\",\"current_topic_summary\":\"failed assign receipt\",\"completion_evidence\":[],\"final_conclusions\":[],\"blocked_reason\":null,\"what_needs_to_be_done_by_user\":null,\"note_candidate\":\"run failed assign first\",\"digest_candidate\":\"run failed assign first\",\"reason\":\"need assign failure receipt evidence\"}</fin_control_feedback>\n<fin_tool_calls>[{\"tool_name\":\"agent.assign\",\"arguments\":{\"task_summary\":\"dispatch sample task\"}}]</fin_tool_calls>",
    );

    let operation = InferenceOperationBuilder
        .build(
            &worker,
            InferenceRequest {
                operation_id: "op-assign-failure-feedback".into(),
                trace_id: "trace-assign-failure-feedback".into(),
                submitted_at: "2026-04-24T00:55:00+08:00".into(),
                refs: EntityRefs {
                    session_id: Some("session-assign-failure-feedback".into()),
                    task_id: Some("task-assign-failure-feedback".into()),
                    ..EntityRefs::default()
                },
                input: "先跑一次缺参数的派发再收口".into(),
                context: full_history_context(&runtime_home, &cwd),
            },
        )
        .expect("operation");

    let _run = runtime
        .run_closure(operation, &provider)
        .expect("closure should run");
    let requests = provider.captured_requests();
    assert_eq!(requests.len(), 2);
    let second = &requests[1];
    assert!(second.rendered_input.contains("tool=agent.assign"));
    assert!(second.rendered_input.contains("authoritative_receipt:"));
    assert!(
        second
            .rendered_input
            .contains("\"failure_kind\": \"missing_argument\"")
    );
    assert!(second.rendered_input.contains("\"retry_hint\":"));
    assert!(
        second
            .rendered_input
            .contains("peer_id or target_worker_id")
    );
}
