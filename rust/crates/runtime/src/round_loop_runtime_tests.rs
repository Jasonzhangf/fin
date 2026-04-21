use super::*;
use fin_config::{
    ConfigMapper, ProviderCredential, ProviderProtocol, ResolvedProviderConfig, UserConfig,
    UserProviderConfig,
};
use fin_provider::{PreparedRequest, ProviderDescriptor, ProviderRequest, ProviderResponse};
use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
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

#[derive(Debug, Clone)]
struct InspectingTwoRoundProvider {
    descriptor: ProviderDescriptor,
    requests: Arc<Mutex<Vec<PreparedRequest>>>,
}

impl InspectingTwoRoundProvider {
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

impl InferenceProvider for InspectingTwoRoundProvider {
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
        let output_text = if request
            .input
            .starts_with("Continue the same turn with the latest tool results.")
        {
            "<fin_user_response>工具结果已确认，现在收口。</fin_user_response>\n<fin_control_feedback>{\"origin\":\"model_output_contract_v1\",\"is_continuation\":true,\"is_simple_query\":false,\"candidate_task_id\":\"task-inspect-two-round\",\"candidate_topic_thread_id\":\"topic-inspect-two-round\",\"continuity_confidence\":91,\"topic_shift_confidence\":9,\"simple_query_confidence\":4,\"previous_topic_summary\":\"tool loop\",\"current_topic_summary\":\"tool loop\",\"note_candidate\":\"tool followup done\",\"digest_candidate\":\"tool followup done\",\"reason\":\"tool result inspected\"}</fin_control_feedback>\n<fin_tool_calls>[{\"tool_name\":\"reasoning.stop\",\"arguments\":{\"summary\":\"tool-backed followup finished\"}}]</fin_tool_calls>"
        } else {
            "<fin_user_response>先查看 peer 列表。</fin_user_response>\n<fin_control_feedback>{\"origin\":\"model_output_contract_v1\",\"is_continuation\":true,\"is_simple_query\":false,\"candidate_task_id\":\"task-inspect-two-round\",\"candidate_topic_thread_id\":\"topic-inspect-two-round\",\"continuity_confidence\":88,\"topic_shift_confidence\":12,\"simple_query_confidence\":6,\"previous_topic_summary\":\"tool loop\",\"current_topic_summary\":\"tool loop\",\"note_candidate\":\"need peer list\",\"digest_candidate\":\"need peer list\",\"reason\":\"inspect peers before stopping\"}</fin_control_feedback>\n<fin_tool_calls>[{\"tool_name\":\"peer.list\",\"arguments\":{}}]</fin_tool_calls>"
        };
        Ok(ProviderResponse {
            provider_name: request.provider_name.clone(),
            model: request.model.clone(),
            output_text: output_text.into(),
            response_id: Some("inspect-two-round-response".into()),
            stop_reason: Some("end_turn".into()),
            status: 200,
        })
    }
}

#[test]
fn runtime_followup_round_renders_tool_results_and_dynamic_catalog() {
    let mut runtime = M1Runtime::default();
    let worker = worker_runtime();
    let provider = InspectingTwoRoundProvider::new();
    let operation = InferenceOperationBuilder
        .build(
            &worker,
            InferenceRequest {
                operation_id: "op-inspect-two-round".into(),
                trace_id: "trace-inspect-two-round".into(),
                submitted_at: "2026-04-20T11:00:00+08:00".into(),
                refs: EntityRefs {
                    session_id: Some("session-inspect-two-round".into()),
                    task_id: Some("task-inspect-two-round".into()),
                    ..EntityRefs::default()
                },
                input: "先检查 peer 再结束".into(),
                context: MinimalContextView::default(),
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
            .contains("Executed tool results (authoritative client facts, full current history):")
    );
    assert!(
        second
            .rendered_input
            .contains("tool=peer.list status=completed")
    );
    assert!(
        second
            .rendered_input
            .contains("Current tool execution history:")
    );
    assert!(second.rendered_input.contains("target_ref=context.peer"));
    assert!(
        second
            .rendered_input
            .contains("auto-tool follow-up round 2 with 1 executed tool result(s)")
    );
    assert!(
        second
            .rendered_input
            .contains("authoritative client-side facts")
    );
}
