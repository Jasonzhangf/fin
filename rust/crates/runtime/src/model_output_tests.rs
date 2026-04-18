use crate::{
    ControlFeedbackBuilder, InferenceOperationBuilder, InferenceRequest, M1Runtime,
    ModelOutputParser, WorkerRuntime,
};
use fin_config::{
    ConfigMapper, ProviderCredential, ProviderProtocol, ResolvedProviderConfig, UserConfig,
    UserProviderConfig,
};
use fin_contracts::{
    EntityRefs, InferenceOperationPayload, MinimalContextView, ProviderPath, ProviderStrategy,
    ProviderTarget, RoleProfileRef,
};
use fin_provider::{
    InferenceProvider, PreparedRequest, ProviderDescriptor, ProviderRequest, ProviderResponse,
};
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
    };
    let system = ConfigMapper::map_user_to_system(&user).expect("mapping should succeed");
    WorkerRuntime::from_system(&system, "agent-1", "worker-1", "runtime", None)
        .expect("worker runtime")
}

fn payload_with_task() -> InferenceOperationPayload {
    InferenceOperationPayload {
        input: "please continue the current work".into(),
        role: RoleProfileRef::new("default").expect("role"),
        provider_path: ProviderPath::new(vec![
            ProviderTarget::new("openai", "gpt-5").expect("target"),
        ])
        .expect("path"),
        provider_strategy: ProviderStrategy::Priority,
        protocol_version: "fin.m1".into(),
        stream: false,
        context: MinimalContextView {
            control: Some(fin_contracts::ContextControlBlock {
                task_id: Some("task-1".into()),
                topic_thread_id: Some("topic-1".into()),
                ..Default::default()
            }),
            summary: Some("previous task context".into()),
            ..Default::default()
        },
    }
}

#[test]
fn model_output_parser_extracts_response_and_control_feedback() {
    let payload = payload_with_task();
    let request = PreparedRequest {
        provider_name: "openai".into(),
        protocol: ProviderProtocol::OpenAiCompatible,
        endpoint: "https://api.example.com/v1/chat/completions".into(),
        model: "gpt-5".into(),
        input: payload.input.clone(),
        rendered_input: "compiled".into(),
        user_agent: None,
        sanitized_headers: BTreeMap::new(),
    };
    let response = ProviderResponse {
        provider_name: "openai".into(),
        model: "gpt-5".into(),
        output_text: "<fin_user_response>好的，我继续当前任务。</fin_user_response>\n<fin_control_feedback>{\"origin\":\"\",\"is_continuation\":true,\"is_simple_query\":false,\"candidate_task_id\":\"task-1\",\"candidate_topic_thread_id\":\"topic-1\",\"continuity_confidence\":97,\"topic_shift_confidence\":6,\"simple_query_confidence\":4,\"previous_topic_summary\":\"\",\"current_topic_summary\":\"继续当前任务\",\"note_candidate\":\"继续推进当前任务\",\"digest_candidate\":\"任务连续\",\"reason\":\"same task\"}</fin_control_feedback>".into(),
        response_id: Some("resp-1".into()),
        stop_reason: Some("end_turn".into()),
        status: 200,
    };

    let parsed = ModelOutputParser::default().parse(&payload, &request, &response);
    assert_eq!(parsed.user_response, "好的，我继续当前任务。");
    assert!(parsed.contract_detected);
    assert!(!parsed.control_feedback_salvaged);
    assert!(parsed.tool_calls.is_empty());
    let feedback = parsed.control_feedback.expect("feedback should parse");
    assert_eq!(feedback.origin, "model_output_contract_v1");
    assert_eq!(feedback.candidate_task_id.as_deref(), Some("task-1"));
    assert_eq!(feedback.continuity_confidence, 97);
}

#[test]
fn control_feedback_builder_falls_back_when_no_structured_output_exists() {
    let payload = payload_with_task();
    let request = PreparedRequest {
        provider_name: "openai".into(),
        protocol: ProviderProtocol::OpenAiCompatible,
        endpoint: "https://api.example.com/v1/chat/completions".into(),
        model: "gpt-5".into(),
        input: payload.input.clone(),
        rendered_input: "compiled".into(),
        user_agent: None,
        sanitized_headers: BTreeMap::new(),
    };
    let response = ProviderResponse {
        provider_name: "openai".into(),
        model: "gpt-5".into(),
        output_text: "plain answer".into(),
        response_id: Some("resp-2".into()),
        stop_reason: Some("end_turn".into()),
        status: 200,
    };

    let parsed = ModelOutputParser::default().parse(&payload, &request, &response);
    let fallback = ControlFeedbackBuilder::default().build(&payload, &request, &response);
    let merged = ControlFeedbackBuilder::default()
        .merge_with_fallback(parsed.control_feedback.clone(), fallback.clone());
    assert_eq!(parsed.user_response, "plain answer");
    assert!(!parsed.control_feedback_salvaged);
    assert!(parsed.tool_calls.is_empty());
    assert_eq!(merged, fallback);
}

#[test]
fn model_output_parser_rejects_unrecognized_control_feedback_shape() {
    let payload = payload_with_task();
    let request = PreparedRequest {
        provider_name: "openai".into(),
        protocol: ProviderProtocol::OpenAiCompatible,
        endpoint: "https://api.example.com/v1/chat/completions".into(),
        model: "gpt-5".into(),
        input: payload.input.clone(),
        rendered_input: "compiled".into(),
        user_agent: None,
        sanitized_headers: BTreeMap::new(),
    };
    let response = ProviderResponse {
        provider_name: "openai".into(),
        model: "gpt-5".into(),
        output_text: "<fin_user_response>OK</fin_user_response>\n<fin_control_feedback>{\"project_scope\":\"/tmp/fin\",\"next_verify_step\":\"none\"}</fin_control_feedback>".into(),
        response_id: Some("resp-3".into()),
        stop_reason: Some("end_turn".into()),
        status: 200,
    };

    let parsed = ModelOutputParser::default().parse(&payload, &request, &response);
    assert_eq!(parsed.user_response, "OK");
    assert!(parsed.contract_detected);
    assert!(!parsed.control_feedback_salvaged);
    assert!(parsed.control_feedback.is_none());
}

#[test]
fn model_output_parser_salvages_whitelisted_feedback_fields_with_mask() {
    let payload = payload_with_task();
    let request = PreparedRequest {
        provider_name: "openai".into(),
        protocol: ProviderProtocol::OpenAiCompatible,
        endpoint: "https://api.example.com/v1/chat/completions".into(),
        model: "gpt-5".into(),
        input: payload.input.clone(),
        rendered_input: "compiled".into(),
        user_agent: None,
        sanitized_headers: BTreeMap::new(),
    };
    let response = ProviderResponse {
        provider_name: "openai".into(),
        model: "gpt-5".into(),
        output_text: "<fin_user_response>OK</fin_user_response>\n<fin_control_feedback>{\"is_continuation\":true,\"is_simple_query\":\"false\",\"continuity_confidence\":1.0,\"topic_shift_confidence\":\"0.18\",\"simple_query_confidence\":\"24\",\"current_topic_summary\":\"masked summary\",\"note_candidate\":\"masked note\",\"digest_candidate\":\"masked digest\",\"reason\":\"masked reason\",\"project_scope\":\"/tmp/ignored\"}</fin_control_feedback>".into(),
        response_id: Some("resp-4".into()),
        stop_reason: Some("end_turn".into()),
        status: 200,
    };

    let parsed = ModelOutputParser::default().parse(&payload, &request, &response);
    assert_eq!(parsed.user_response, "OK");
    assert!(parsed.contract_detected);
    assert!(parsed.control_feedback_salvaged);
    assert!(parsed.tool_calls.is_empty());
    let feedback = parsed
        .control_feedback
        .expect("feedback should be salvaged");
    assert_eq!(feedback.origin, "model_output_contract_masked");
    assert!(feedback.is_continuation);
    assert!(!feedback.is_simple_query);
    assert_eq!(feedback.continuity_confidence, 100);
    assert_eq!(feedback.topic_shift_confidence, 18);
    assert_eq!(feedback.simple_query_confidence, 24);
    assert_eq!(
        feedback.current_topic_summary.as_deref(),
        Some("masked summary")
    );
    assert_eq!(feedback.reason, "masked reason");
}

#[test]
fn model_output_parser_extracts_tool_calls_block() {
    let payload = payload_with_task();
    let request = PreparedRequest {
        provider_name: "openai".into(),
        protocol: ProviderProtocol::OpenAiCompatible,
        endpoint: "https://api.example.com/v1/chat/completions".into(),
        model: "gpt-5".into(),
        input: payload.input.clone(),
        rendered_input: "compiled".into(),
        user_agent: None,
        sanitized_headers: BTreeMap::new(),
    };
    let response = ProviderResponse {
        provider_name: "openai".into(),
        model: "gpt-5".into(),
        output_text: "<fin_user_response>waiting for logs</fin_user_response>\n<fin_control_feedback>{\"origin\":\"model_output_contract_v1\",\"is_continuation\":true,\"is_simple_query\":false,\"candidate_task_id\":null,\"candidate_topic_thread_id\":null,\"continuity_confidence\":84,\"topic_shift_confidence\":16,\"simple_query_confidence\":9,\"previous_topic_summary\":\"logs\",\"current_topic_summary\":\"logs\",\"note_candidate\":\"need wait\",\"digest_candidate\":\"wait scheduled\",\"reason\":\"need async wait\"}</fin_control_feedback>\n<fin_tool_calls>[{\"tool_name\":\"wait.remind\",\"arguments\":{\"wait_minutes\":5,\"reminder\":\"check ci logs\"}}]</fin_tool_calls>".into(),
        response_id: Some("resp-tool-1".into()),
        stop_reason: Some("end_turn".into()),
        status: 200,
    };

    let parsed = ModelOutputParser::default().parse(&payload, &request, &response);
    assert_eq!(parsed.user_response, "waiting for logs");
    assert!(parsed.contract_detected);
    assert_eq!(parsed.tool_calls.len(), 1);
    assert_eq!(parsed.tool_calls[0].tool_name, "wait.remind");
    assert_eq!(
        parsed.tool_calls[0]
            .arguments
            .get("wait_minutes")
            .and_then(serde_json::Value::as_u64),
        Some(5)
    );
}

#[derive(Debug, Clone)]
struct StructuredProvider {
    descriptor: ProviderDescriptor,
}

#[derive(Debug, Clone)]
struct WaitToolProvider {
    descriptor: ProviderDescriptor,
}

#[derive(Debug, Clone)]
struct ReasoningStopProvider {
    descriptor: ProviderDescriptor,
}

impl ReasoningStopProvider {
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

impl WaitToolProvider {
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

impl StructuredProvider {
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

impl InferenceProvider for StructuredProvider {
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
            output_text: "<fin_user_response>structured answer</fin_user_response>\n<fin_control_feedback>{\"origin\":\"model_output_contract_v1\",\"is_continuation\":true,\"is_simple_query\":false,\"candidate_task_id\":\"task-structured\",\"candidate_topic_thread_id\":\"topic-structured\",\"continuity_confidence\":95,\"topic_shift_confidence\":5,\"simple_query_confidence\":7,\"previous_topic_summary\":\"task\",\"current_topic_summary\":\"task\",\"note_candidate\":\"structured note\",\"digest_candidate\":\"structured digest\",\"reason\":\"explicit control block\"}</fin_control_feedback>".into(),
            response_id: Some("structured-response".into()),
            stop_reason: Some("end_turn".into()),
            status: 200,
        })
    }
}

impl InferenceProvider for WaitToolProvider {
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

impl InferenceProvider for ReasoningStopProvider {
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
            output_text: "<fin_user_response>任务完成。</fin_user_response>\n<fin_control_feedback>{\"origin\":\"model_output_contract_v1\",\"is_continuation\":false,\"is_simple_query\":false,\"candidate_task_id\":\"task-stop\",\"candidate_topic_thread_id\":\"topic-stop\",\"continuity_confidence\":35,\"topic_shift_confidence\":65,\"simple_query_confidence\":10,\"previous_topic_summary\":\"build\",\"current_topic_summary\":\"report\",\"note_candidate\":\"finished\",\"digest_candidate\":\"finished\",\"reason\":\"done\"}</fin_control_feedback>\n<fin_tool_calls>[{\"tool_name\":\"reasoning.stop\",\"arguments\":{\"summary\":\"all required checks done\"}}]</fin_tool_calls>".into(),
            response_id: Some("stop-response".into()),
            stop_reason: Some("end_turn".into()),
            status: 200,
        })
    }
}

#[test]
fn runtime_closure_uses_structured_user_response_for_session_visible_output() {
    let mut runtime = M1Runtime::default();
    let worker = worker_runtime();
    let operation = InferenceOperationBuilder
        .build(
            &worker,
            InferenceRequest {
                operation_id: "op-structured".into(),
                trace_id: "trace-structured".into(),
                submitted_at: "2026-04-18T10:00:00+08:00".into(),
                refs: EntityRefs {
                    task_id: Some("task-structured".into()),
                    ..EntityRefs::default()
                },
                input: "continue".into(),
                context: MinimalContextView::default(),
            },
        )
        .expect("operation");

    let run = runtime
        .run_closure(operation, &StructuredProvider::new())
        .expect("closure");
    assert_eq!(run.assistant_response_text, "structured answer");
    assert_eq!(
        run.note.summary,
        "provider openai returned: structured answer"
    );
    assert_eq!(run.digest.continuity_tail[1], "structured answer");
    assert!(
        run.events
            .iter()
            .any(|event| event.event_type == "model.output_parsed")
    );
}

#[test]
fn runtime_closure_records_wait_reminder_tool_and_event() {
    let mut runtime = M1Runtime::default();
    let worker = worker_runtime();
    let operation = InferenceOperationBuilder
        .build(
            &worker,
            InferenceRequest {
                operation_id: "op-wait-tool".into(),
                trace_id: "trace-wait-tool".into(),
                submitted_at: "2026-04-18T10:10:00+08:00".into(),
                refs: EntityRefs {
                    session_id: Some("session-wait".into()),
                    task_id: Some("task-wait".into()),
                    ..EntityRefs::default()
                },
                input: "等两分钟再看日志".into(),
                context: MinimalContextView::default(),
            },
        )
        .expect("operation");

    let run = runtime
        .run_closure(operation, &WaitToolProvider::new())
        .expect("closure");
    assert_eq!(run.assistant_response_text, "先等待日志完成。");
    assert!(
        run.tool_records
            .iter()
            .any(|record| record.tool_name == "wait.remind" && record.status == "completed")
    );
    assert!(
        run.events
            .iter()
            .any(|event| event.event_type == "system.reminder_scheduled")
    );
    assert!(
        run.events
            .iter()
            .any(|event| event.event_type == "reasoning.auto_tool_roundtrip_completed")
    );
}

#[test]
fn runtime_closure_uses_reasoning_stop_as_stop_signal() {
    let mut runtime = M1Runtime::default();
    let worker = worker_runtime();
    let operation = InferenceOperationBuilder
        .build(
            &worker,
            InferenceRequest {
                operation_id: "op-stop-tool".into(),
                trace_id: "trace-stop-tool".into(),
                submitted_at: "2026-04-18T10:20:00+08:00".into(),
                refs: EntityRefs {
                    session_id: Some("session-stop".into()),
                    task_id: Some("task-stop".into()),
                    ..EntityRefs::default()
                },
                input: "收口当前任务".into(),
                context: MinimalContextView::default(),
            },
        )
        .expect("operation");

    let run = runtime
        .run_closure(operation, &ReasoningStopProvider::new())
        .expect("closure");
    assert!(
        run.tool_records
            .iter()
            .any(|record| record.tool_name == "reasoning.stop" && record.status == "completed")
    );
    assert!(
        run.events
            .iter()
            .any(|event| event.event_type == "reasoning.stopped")
    );
    let operation_completed = run
        .events
        .iter()
        .find(|event| event.event_type == "operation.completed")
        .expect("operation.completed");
    assert_eq!(
        operation_completed
            .payload
            .get("status")
            .and_then(serde_json::Value::as_str),
        Some("stopped")
    );
    assert_eq!(
        operation_completed
            .payload
            .get("stop_source")
            .and_then(serde_json::Value::as_str),
        Some("reasoning.stop")
    );
}
