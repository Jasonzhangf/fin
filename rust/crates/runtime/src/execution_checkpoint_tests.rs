use super::*;
use fin_config::{
    ConfigMapper, ProviderCredential, ProviderProtocol, ResolvedProviderConfig, UserConfig,
    UserProviderConfig,
};
use fin_provider::{PreparedRequest, ProviderDescriptor, ProviderRequest, ProviderResponse};
use std::collections::BTreeMap;

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
struct WaitCheckpointProvider {
    descriptor: ProviderDescriptor,
}

impl WaitCheckpointProvider {
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
        }
    }
}

impl InferenceProvider for WaitCheckpointProvider {
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
        Ok(ProviderResponse {
            provider_name: request.provider_name.clone(),
            model: request.model.clone(),
            output_text: "<fin_user_response>先等待日志完成。</fin_user_response>\n<fin_control_feedback>{\"origin\":\"model_output_contract_v1\",\"is_continuation\":true,\"is_simple_query\":false,\"candidate_task_id\":\"task-wait\",\"candidate_topic_thread_id\":\"topic-wait\",\"continuity_confidence\":90,\"topic_shift_confidence\":10,\"simple_query_confidence\":8,\"previous_topic_summary\":\"ci\",\"current_topic_summary\":\"ci\",\"note_candidate\":\"schedule wait\",\"digest_candidate\":\"wait tool\",\"reason\":\"waiting external result\"}</fin_control_feedback>\n<fin_tool_calls>[{\"tool_name\":\"wait.remind\",\"arguments\":{\"wait_minutes\":2,\"reminder\":\"检查 CI 日志\"}}]</fin_tool_calls>".into(),
            response_id: Some("wait-response".into()),
            stop_reason: Some("end_turn".into()),
            status: 200,
        })
    }
}

#[test]
fn runtime_wait_closure_records_open_resume_checkpoint() {
    let mut runtime = M1Runtime::default();
    let worker = worker_runtime();
    let operation = InferenceOperationBuilder
        .build(
            &worker,
            InferenceRequest {
                operation_id: "op-wait-checkpoint".into(),
                trace_id: "trace-wait-checkpoint".into(),
                submitted_at: "2026-04-20T13:00:00+08:00".into(),
                refs: EntityRefs {
                    session_id: Some("session-wait-checkpoint".into()),
                    task_id: Some("task-wait-checkpoint".into()),
                    ..EntityRefs::default()
                },
                input: "等两分钟再看日志".into(),
                context: MinimalContextView::default(),
            },
        )
        .expect("operation");

    let run = runtime
        .run_closure(operation, &WaitCheckpointProvider::new())
        .expect("closure");

    let checkpoint = run.resume_checkpoint.expect("resume checkpoint");
    assert_eq!(checkpoint.status, "open");
    assert_eq!(checkpoint.checkpoint_kind, "wait_reminder_resume");
    assert_eq!(checkpoint.source_round_index, 1);
    assert_eq!(checkpoint.next_round_index, 2);
    assert!(
        checkpoint
            .resume_input
            .contains("Continue the same turn with the latest tool results.")
    );
    assert!(
        run.events
            .iter()
            .any(|event| event.event_type == "execution.checkpoint_recorded")
    );
}
