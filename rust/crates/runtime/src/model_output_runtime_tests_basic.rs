use super::*;
use fin_config::{ProviderCredential, ProviderProtocol, ResolvedProviderConfig};
use fin_provider::{PreparedRequest, ProviderDescriptor, ProviderRequest, ProviderResponse};
use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
};

#[derive(Debug, Clone)]
struct CompletedWithEvidenceProvider {
    descriptor: ProviderDescriptor,
}

impl CompletedWithEvidenceProvider {
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

impl InferenceProvider for CompletedWithEvidenceProvider {
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
            output_text: "<fin_user_response>任务已完成，所有检查通过。</fin_user_response>\n<fin_control_feedback>{\"origin\":\"model_output_contract_v1\",\"is_continuation\":false,\"is_simple_query\":false,\"candidate_task_id\":\"task-complete\",\"candidate_topic_thread_id\":\"topic-complete\",\"continuity_confidence\":20,\"topic_shift_confidence\":80,\"simple_query_confidence\":5,\"previous_topic_summary\":\"task\",\"current_topic_summary\":\"done\",\"note_candidate\":\"finished\",\"digest_candidate\":\"finished\",\"reason\":\"done\",\"task_completed\":true,\"completion_evidence\":[\"所有步骤已完成\",\"文件已验证\"],\"final_conclusions\":[\"任务成功收口\"]}</fin_control_feedback>".into(),
            response_id: Some("completed-with-evidence-response".into()),
            stop_reason: Some("end_turn".into()),
            status: 200,
            tool_calls: Vec::new(),
        })
    }
}

#[derive(Debug, Clone)]
struct SimpleChatProvider {
    descriptor: ProviderDescriptor,
}

impl SimpleChatProvider {
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

impl InferenceProvider for SimpleChatProvider {
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
            output_text: "<fin_user_response>你好！有什么可以帮你的？</fin_user_response>\n<fin_control_feedback>{\"origin\":\"model_output_contract_v1\",\"is_continuation\":false,\"is_simple_query\":true,\"candidate_task_id\":null,\"candidate_topic_thread_id\":null,\"continuity_confidence\":0,\"topic_shift_confidence\":0,\"simple_query_confidence\":100,\"previous_topic_summary\":\"\",\"current_topic_summary\":\"greeting\",\"note_candidate\":\"simple chat\",\"digest_candidate\":\"simple chat\",\"reason\":\"simple chat\",\"is_simple_chat\":true}</fin_control_feedback>".into(),
            response_id: Some("simple-chat-response".into()),
            stop_reason: Some("end_turn".into()),
            status: 200,
            tool_calls: Vec::new(),
        })
    }
}

#[derive(Debug, Clone)]
struct BlockedUserActionProvider {
    descriptor: ProviderDescriptor,
}

impl BlockedUserActionProvider {
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

impl InferenceProvider for BlockedUserActionProvider {
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
            output_text: "<fin_user_response>部署脚本已准备好，但需要你先确认 SSH 密钥位置。</fin_user_response>\n<fin_control_feedback>{\"origin\":\"model_output_contract_v1\",\"is_continuation\":true,\"is_simple_query\":false,\"candidate_task_id\":\"task-blocked\",\"candidate_topic_thread_id\":\"topic-blocked\",\"continuity_confidence\":80,\"topic_shift_confidence\":20,\"simple_query_confidence\":5,\"previous_topic_summary\":\"deploy\",\"current_topic_summary\":\"deploy\",\"note_candidate\":\"blocked\",\"digest_candidate\":\"blocked\",\"reason\":\"user action needed\",\"blocked\":true,\"needs_user_involve\":true,\"blocked_reason\":\"需要 SSH 密钥路径才能执行部署\",\"what_needs_to_be_done_by_user\":\"请提供 SSH 密钥的完整路径\"}</fin_control_feedback>\n<fin_tool_calls>[{\"tool_name\":\"reasoning.stop\",\"arguments\":{\"summary\":\"blocked, waiting for user input\"}}]</fin_tool_calls>".into(),
            response_id: Some("blocked-user-action-response".into()),
            stop_reason: Some("end_turn".into()),
            status: 200,
            tool_calls: Vec::new(),
        })
    }
}

#[derive(Debug, Clone)]
struct MissingCompletionFeedbackRetryProvider {
    descriptor: ProviderDescriptor,
    requests: Arc<Mutex<Vec<PreparedRequest>>>,
}

impl MissingCompletionFeedbackRetryProvider {
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

impl InferenceProvider for MissingCompletionFeedbackRetryProvider {
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
            "<fin_user_response>任务完成。</fin_user_response>\n<fin_control_feedback>{\"origin\":\"model_output_contract_v1\",\"is_continuation\":false,\"is_simple_query\":false,\"candidate_task_id\":\"task-missing-feedback\",\"candidate_topic_thread_id\":\"topic-missing-feedback\",\"continuity_confidence\":30,\"topic_shift_confidence\":70,\"simple_query_confidence\":10,\"previous_topic_summary\":\"task\",\"current_topic_summary\":\"report\",\"note_candidate\":\"done\",\"digest_candidate\":\"done\",\"reason\":\"done\",\"task_completed\":true}</fin_control_feedback>\n<fin_tool_calls>[{\"tool_name\":\"reasoning.stop\",\"arguments\":{\"summary\":\"task finished\"}}]</fin_tool_calls>".into()
        } else {
            "<fin_user_response>任务已完成，所有检查通过。</fin_user_response>\n<fin_control_feedback>{\"origin\":\"model_output_contract_v1\",\"is_continuation\":false,\"is_simple_query\":false,\"candidate_task_id\":\"task-missing-feedback\",\"candidate_topic_thread_id\":\"topic-missing-feedback\",\"continuity_confidence\":20,\"topic_shift_confidence\":80,\"simple_query_confidence\":5,\"previous_topic_summary\":\"task\",\"current_topic_summary\":\"done\",\"note_candidate\":\"finished\",\"digest_candidate\":\"finished\",\"reason\":\"done\",\"task_completed\":true,\"completion_evidence\":[\"all checks passed\"],\"final_conclusions\":[\"task complete\"]}</fin_control_feedback>\n<fin_tool_calls>[{\"tool_name\":\"reasoning.stop\",\"arguments\":{\"summary\":\"task finished with full evidence\"}}]</fin_tool_calls>".into()
        };
        Ok(ProviderResponse {
            provider_name: request.provider_name.clone(),
            model: request.model.clone(),
            output_text,
            response_id: Some("missing-completion-feedback-response".into()),
            stop_reason: Some("end_turn".into()),
            status: 200,
            tool_calls: Vec::new(),
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
    assert_eq!(run.provider_request_records.len(), 1);
    assert_eq!(run.provider_response_records.len(), 1);
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
        Some("waiting_external")
    );
    assert_eq!(
        operation_completed
            .payload
            .get("stop_source")
            .and_then(serde_json::Value::as_str),
        Some("wait.remind")
    );
    assert!(
        !run.events
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
        Some("completed_with_evidence")
    );
    assert_eq!(
        operation_completed
            .payload
            .get("stop_source")
            .and_then(serde_json::Value::as_str),
        Some("completed_with_evidence")
    );
    assert_eq!(run.turn_record.status, "completed_with_evidence");
    assert!(
        !run.events
            .iter()
            .any(|event| event.event_type == "control.exit_gate_rejected")
    );
}

#[test]
fn runtime_closure_completes_with_evidence_via_control_feedback() {
    let mut runtime = M1Runtime::default();
    let worker = worker_runtime();
    let operation = InferenceOperationBuilder
        .build(
            &worker,
            InferenceRequest {
                operation_id: "op-completed-with-evidence".into(),
                trace_id: "trace-completed-with-evidence".into(),
                submitted_at: "2026-04-22T10:30:00+08:00".into(),
                refs: EntityRefs {
                    session_id: Some("session-complete".into()),
                    task_id: Some("task-complete".into()),
                    ..EntityRefs::default()
                },
                input: "给我最终完成结论".into(),
                context: MinimalContextView::default(),
            },
        )
        .expect("operation");

    let run = runtime
        .run_closure(operation, &CompletedWithEvidenceProvider::new())
        .expect("closure");
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
        Some("completed_with_evidence")
    );
    assert_eq!(
        operation_completed
            .payload
            .get("stop_source")
            .and_then(serde_json::Value::as_str),
        Some("completed_with_evidence")
    );
    assert_eq!(run.turn_record.status, "completed_with_evidence");
    assert_eq!(run.progress.next_step.as_deref(), Some("render_projection"));
}

#[test]
fn runtime_closure_accepts_simple_chat_exit_channel() {
    let mut runtime = M1Runtime::default();
    let worker = worker_runtime();
    let operation = InferenceOperationBuilder
        .build(
            &worker,
            InferenceRequest {
                operation_id: "op-simple-chat".into(),
                trace_id: "trace-simple-chat".into(),
                submitted_at: "2026-04-22T10:40:00+08:00".into(),
                refs: EntityRefs {
                    session_id: Some("session-simple-chat".into()),
                    ..EntityRefs::default()
                },
                input: "你好".into(),
                context: MinimalContextView::default(),
            },
        )
        .expect("operation");

    let run = runtime
        .run_closure(operation, &SimpleChatProvider::new())
        .expect("closure");
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
        Some("simple_chat_done")
    );
    assert_eq!(run.turn_record.status, "simple_chat_done");
}

#[test]
fn runtime_closure_accepts_blocked_user_action_exit_channel() {
    let mut runtime = M1Runtime::default();
    let worker = worker_runtime();
    let operation = InferenceOperationBuilder
        .build(
            &worker,
            InferenceRequest {
                operation_id: "op-blocked-user-action".into(),
                trace_id: "trace-blocked-user-action".into(),
                submitted_at: "2026-04-22T10:50:00+08:00".into(),
                refs: EntityRefs {
                    session_id: Some("session-blocked".into()),
                    task_id: Some("task-blocked".into()),
                    ..EntityRefs::default()
                },
                input: "继续部署".into(),
                context: MinimalContextView::default(),
            },
        )
        .expect("operation");

    let run = runtime
        .run_closure(operation, &BlockedUserActionProvider::new())
        .expect("closure");
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
        Some("blocked_requires_user_action")
    );
    assert_eq!(run.turn_record.status, "blocked_requires_user_action");
}
