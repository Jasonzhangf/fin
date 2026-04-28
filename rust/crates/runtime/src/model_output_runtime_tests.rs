use super::*;

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
            tool_calls: Vec::new(),
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
            tool_calls: Vec::new(),
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
            tool_calls: Vec::new(),
        })
    }
}

#[derive(Debug, Clone)]
struct TwoRoundProvider {
    descriptor: ProviderDescriptor,
}

impl TwoRoundProvider {
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

impl InferenceProvider for TwoRoundProvider {
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
        let output_text = if request
            .input
            .starts_with("Continue the same turn.")
        {
            "<fin_user_response>工具结果已确认，现在收口。</fin_user_response>\n<fin_control_feedback>{\"origin\":\"model_output_contract_v1\",\"is_continuation\":true,\"is_simple_query\":false,\"candidate_task_id\":\"task-two-round\",\"candidate_topic_thread_id\":\"topic-two-round\",\"continuity_confidence\":90,\"topic_shift_confidence\":10,\"simple_query_confidence\":5,\"previous_topic_summary\":\"tool loop\",\"current_topic_summary\":\"tool loop\",\"note_candidate\":\"tool followup done\",\"digest_candidate\":\"tool followup done\",\"reason\":\"tool result inspected\"}</fin_control_feedback>\n<fin_tool_calls>[{\"tool_name\":\"reasoning.stop\",\"arguments\":{\"summary\":\"tool-backed followup finished\"}}]</fin_tool_calls>"
        } else {
            "<fin_user_response>先查看 peer 列表。</fin_user_response>\n<fin_control_feedback>{\"origin\":\"model_output_contract_v1\",\"is_continuation\":true,\"is_simple_query\":false,\"candidate_task_id\":\"task-two-round\",\"candidate_topic_thread_id\":\"topic-two-round\",\"continuity_confidence\":88,\"topic_shift_confidence\":12,\"simple_query_confidence\":6,\"previous_topic_summary\":\"tool loop\",\"current_topic_summary\":\"tool loop\",\"note_candidate\":\"need peer list\",\"digest_candidate\":\"need peer list\",\"reason\":\"inspect peers before stopping\"}</fin_control_feedback>\n<fin_tool_calls>[{\"tool_name\":\"peer.list\",\"arguments\":{}}]</fin_tool_calls>"
        };
        Ok(ProviderResponse {
            provider_name: request.provider_name.clone(),
            model: request.model.clone(),
            output_text: output_text.into(),
            response_id: Some("two-round-response".into()),
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

#[test]
fn runtime_closure_records_round_level_events_for_multi_round_tool_loop() {
    let mut runtime = M1Runtime::default();
    let worker = worker_runtime();
    let operation = InferenceOperationBuilder
        .build(
            &worker,
            InferenceRequest {
                operation_id: "op-two-round".into(),
                trace_id: "trace-two-round".into(),
                submitted_at: "2026-04-19T12:00:00+08:00".into(),
                refs: EntityRefs {
                    session_id: Some("session-two-round".into()),
                    task_id: Some("task-two-round".into()),
                    ..EntityRefs::default()
                },
                input: "先检查 peer 再结束".into(),
                context: MinimalContextView::default(),
            },
        )
        .expect("operation");

    let run = runtime
        .run_closure(operation, &TwoRoundProvider::new())
        .expect("closure");
    assert_eq!(run.provider_request_records.len(), 2);
    assert_eq!(run.provider_response_records.len(), 2);
    assert_eq!(
        run.events
            .iter()
            .filter(|event| event.event_type == "provider.round_completed")
            .count(),
        2
    );
    assert_eq!(
        run.events
            .iter()
            .filter(|event| event.event_type == "model.output_round_parsed")
            .count(),
        2
    );
    assert_eq!(
        run.events
            .iter()
            .filter(|event| event.event_type == "control.feedback_round_recorded")
            .count(),
        2
    );
    assert_eq!(
        run.events
            .iter()
            .filter(|event| event.event_type == "tool.dispatch_round_completed")
            .count(),
        2
    );
    assert!(
        run.step_records
            .iter()
            .filter(|step| matches!(
                step.step_kind.as_str(),
                "provider_request" | "model_parse" | "control_feedback" | "tool_dispatch"
            ))
            .all(|step| !step.event_ids.is_empty())
    );
    assert!(
        run.step_records
            .windows(2)
            .all(|pair| pair[0].step_index < pair[1].step_index)
    );
    let model_tool_ids = run
        .tool_records
        .iter()
        .filter(|record| record.tool_name != "provider.call")
        .map(|record| record.tool_call_id.as_str())
        .collect::<Vec<_>>();
    let unique_model_tool_ids = model_tool_ids
        .iter()
        .copied()
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(unique_model_tool_ids.len(), model_tool_ids.len());
    assert!(model_tool_ids.iter().any(|id| id.contains("-r01-00")));
    assert!(model_tool_ids.iter().any(|id| id.contains("-r02-00")));
}
