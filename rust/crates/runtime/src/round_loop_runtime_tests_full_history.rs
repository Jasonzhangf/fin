use super::*;
use fin_config::{
    ConfigMapper, ProviderCredential, ProviderProtocol, ResolvedProviderConfig, UserConfig,
    UserProviderConfig,
};
use fin_contracts::ProjectContextBlock;
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
struct TwoRoundToolProvider {
    descriptor: ProviderDescriptor,
    requests: Arc<Mutex<Vec<PreparedRequest>>>,
    first_round_output: String,
}

impl TwoRoundToolProvider {
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

impl InferenceProvider for TwoRoundToolProvider {
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
        let output_text = if request
            .input
            .starts_with("Continue the same turn with the latest tool results.")
        {
            "<fin_user_response>工具结果已确认，停止。</fin_user_response>\n<fin_control_feedback>{\"origin\":\"model_output_contract_v1\",\"is_continuation\":true,\"is_simple_query\":false,\"candidate_task_id\":\"task-two-round-tool\",\"candidate_topic_thread_id\":\"topic-two-round-tool\",\"continuity_confidence\":95,\"topic_shift_confidence\":5,\"simple_query_confidence\":4,\"previous_topic_summary\":\"tool followup\",\"current_topic_summary\":\"tool followup\",\"note_candidate\":\"tool followup done\",\"digest_candidate\":\"tool followup done\",\"reason\":\"tool result inspected\"}</fin_control_feedback>\n<fin_tool_calls>[{\"tool_name\":\"reasoning.stop\",\"arguments\":{\"summary\":\"tool-backed followup finished\"}}]</fin_tool_calls>".to_string()
        } else {
            self.first_round_output.clone()
        };
        Ok(ProviderResponse {
            provider_name: request.provider_name.clone(),
            model: request.model.clone(),
            output_text,
            response_id: Some("two-round-tool-provider".into()),
            stop_reason: Some("end_turn".into()),
            status: 200,
            usage: None,
        })
    }
}

#[test]
fn runtime_followup_round_includes_full_exec_receipt_in_current_history() {
    let runtime_home = temp_runtime_home("exec-full-history");
    let cwd = runtime_home.join("workspace");
    fs::create_dir_all(&cwd).expect("workspace");

    let mut runtime = M1Runtime::default();
    let worker = worker_runtime();
    let provider = TwoRoundToolProvider::new(
        "<fin_user_response>先执行命令。</fin_user_response>\n<fin_control_feedback>{\"origin\":\"model_output_contract_v1\",\"is_continuation\":true,\"is_simple_query\":false,\"candidate_task_id\":\"task-two-round-tool\",\"candidate_topic_thread_id\":\"topic-two-round-tool\",\"continuity_confidence\":90,\"topic_shift_confidence\":10,\"simple_query_confidence\":5,\"previous_topic_summary\":\"exec receipt\",\"current_topic_summary\":\"exec receipt\",\"note_candidate\":\"run exec first\",\"digest_candidate\":\"run exec first\",\"reason\":\"need exec evidence\"}</fin_control_feedback>\n<fin_tool_calls>[{\"tool_name\":\"exec_command\",\"arguments\":{\"cmd\":\"printf 'alpha\\\\nbeta\\\\n'\",\"cwd\":\"WORKDIR\"}}]</fin_tool_calls>"
            .replace("WORKDIR", &cwd.display().to_string()),
    );
    let operation = InferenceOperationBuilder
        .build(
            &worker,
            InferenceRequest {
                operation_id: "op-exec-full-history".into(),
                trace_id: "trace-exec-full-history".into(),
                submitted_at: "2026-04-21T13:00:00+08:00".into(),
                refs: EntityRefs {
                    session_id: Some("session-exec-full-history".into()),
                    task_id: Some("task-exec-full-history".into()),
                    ..EntityRefs::default()
                },
                input: "先执行命令再收口".into(),
                context: full_history_context(&runtime_home, &cwd),
            },
        )
        .expect("operation");

    runtime
        .run_closure(operation, &provider)
        .expect("closure should run");
    let requests = provider.captured_requests();
    assert_eq!(requests.len(), 2);
    let second = &requests[1];
    assert!(
        second
            .rendered_input
            .contains("Current tool execution history:")
    );
    assert!(second.rendered_input.contains("tool=exec_command"));
    assert!(second.rendered_input.contains("authoritative_receipt:"));
    assert!(
        second
            .rendered_input
            .contains("\"cmd\": \"printf 'alpha\\\\nbeta\\\\n'\"")
    );
    assert!(second.rendered_input.contains("\"stdout\":"));
    assert!(second.rendered_input.contains("alpha\\\\nbeta"));
}

#[test]
fn runtime_followup_round_includes_full_patch_receipt_arguments() {
    let runtime_home = temp_runtime_home("patch-full-history");
    let cwd = runtime_home.join("workspace");
    fs::create_dir_all(&cwd).expect("workspace");

    let target = cwd.join("sample.txt");
    fs::write(&target, "before\n").expect("seed file");

    let mut runtime = M1Runtime::default();
    let worker = worker_runtime();
    let provider = TwoRoundToolProvider::new(
        "<fin_user_response>先改文件。</fin_user_response>\n<fin_control_feedback>{\"origin\":\"model_output_contract_v1\",\"is_continuation\":true,\"is_simple_query\":false,\"candidate_task_id\":\"task-two-round-tool\",\"candidate_topic_thread_id\":\"topic-two-round-tool\",\"continuity_confidence\":90,\"topic_shift_confidence\":10,\"simple_query_confidence\":5,\"previous_topic_summary\":\"patch receipt\",\"current_topic_summary\":\"patch receipt\",\"note_candidate\":\"run patch first\",\"digest_candidate\":\"run patch first\",\"reason\":\"need patch evidence\"}</fin_control_feedback>\n<fin_tool_calls>[{\"tool_name\":\"apply_patch\",\"arguments\":{\"mode\":\"replace\",\"path\":\"sample.txt\",\"old_string\":\"before\\n\",\"new_string\":\"after\\n\"}}]</fin_tool_calls>",
    );
    let operation = InferenceOperationBuilder
        .build(
            &worker,
            InferenceRequest {
                operation_id: "op-patch-full-history".into(),
                trace_id: "trace-patch-full-history".into(),
                submitted_at: "2026-04-21T13:05:00+08:00".into(),
                refs: EntityRefs {
                    session_id: Some("session-patch-full-history".into()),
                    task_id: Some("task-patch-full-history".into()),
                    ..EntityRefs::default()
                },
                input: "先改文件再收口".into(),
                context: full_history_context(&runtime_home, &cwd),
            },
        )
        .expect("operation");

    runtime
        .run_closure(operation, &provider)
        .expect("closure should run");
    let requests = provider.captured_requests();
    assert_eq!(requests.len(), 2);
    let second = &requests[1];
    assert!(second.rendered_input.contains("tool=apply_patch"));
    assert!(second.rendered_input.contains("authoritative_receipt:"));
    assert!(second.rendered_input.contains("\"arguments\": {"));
    assert!(second.rendered_input.contains("\"path\": \"sample.txt\""));
    assert!(
        second
            .rendered_input
            .contains("\"old_string\": \"before\\n\"")
    );
    assert!(
        second
            .rendered_input
            .contains("\"new_string\": \"after\\n\"")
    );
}
