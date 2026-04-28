use super::*;
use fin_provider::{PreparedRequest, ProviderDescriptor, ProviderRequest, ProviderResponse};

#[derive(Debug, Clone)]
struct ManagedLoopProvider {
    descriptor: ProviderDescriptor,
}

impl ManagedLoopProvider {
    fn new(system: &SystemConfig) -> Self {
        Self {
            descriptor: ProviderDescriptor::from_resolved(
                system.default_provider_config().expect("default provider"),
            ),
        }
    }
}

impl fin_provider::InferenceProvider for ManagedLoopProvider {
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
            .starts_with("Framework planning kickoff: the task is now formalized.")
        {
            "<fin_user_response>已进入 managed path，并创建执行任务。</fin_user_response>\n<fin_control_feedback>{\"origin\":\"model_output_contract_v1\",\"is_continuation\":true,\"is_simple_query\":false,\"candidate_task_id\":\"task-closed-loop\",\"candidate_topic_thread_id\":\"topic-closed-loop\",\"continuity_confidence\":97,\"topic_shift_confidence\":3,\"simple_query_confidence\":2,\"previous_topic_summary\":\"formalized task\",\"current_topic_summary\":\"managed planning\",\"note_candidate\":\"managed planning created task board\",\"digest_candidate\":\"managed planning created task board\",\"reason\":\"complex work requires managed path\"}</fin_control_feedback>\n<fin_tool_calls>[{\"tool_name\":\"project.task.create\",\"arguments\":{\"task_id\":\"task-closed-loop\",\"title\":\"close managed loop\",\"summary\":\"dispatch worker then submit then review\",\"status\":\"ready\"}},{\"tool_name\":\"reasoning.stop\",\"arguments\":{\"summary\":\"managed planning completed\"}}]</fin_tool_calls>"
        } else if request
            .input
            .starts_with("Framework owner-loop directive: dispatch ready managed tasks now.")
        {
            "<fin_user_response>已完成 owner dispatch。</fin_user_response>\n<fin_control_feedback>{\"origin\":\"model_output_contract_v1\",\"is_continuation\":true,\"is_simple_query\":false,\"candidate_task_id\":\"task-closed-loop\",\"candidate_topic_thread_id\":\"topic-closed-loop\",\"continuity_confidence\":95,\"topic_shift_confidence\":5,\"simple_query_confidence\":4,\"previous_topic_summary\":\"owner dispatch\",\"current_topic_summary\":\"owner dispatch\",\"note_candidate\":\"owner dispatch completed\",\"digest_candidate\":\"owner dispatch completed\",\"reason\":\"dispatch ready task\"}</fin_control_feedback>\n<fin_tool_calls>[{\"tool_name\":\"agent.assign\",\"arguments\":{\"task_id\":\"task-closed-loop\",\"target_worker_id\":\"worker-builder\",\"task_summary\":\"implement managed task and attach receipts\"}},{\"tool_name\":\"project.task.claim\",\"arguments\":{\"task_id\":\"task-closed-loop\",\"worker_id\":\"worker-builder\",\"status\":\"claimed\"}},{\"tool_name\":\"reasoning.stop\",\"arguments\":{\"summary\":\"owner dispatch completed\"}}]</fin_tool_calls>"
        } else if request
            .input
            .starts_with("Framework assignment: execute task now.")
        {
            "<fin_user_response>worker 已提交执行结果。</fin_user_response>\n<fin_control_feedback>{\"origin\":\"model_output_contract_v1\",\"is_continuation\":true,\"is_simple_query\":false,\"candidate_task_id\":\"task-closed-loop\",\"candidate_topic_thread_id\":\"topic-closed-loop\",\"continuity_confidence\":94,\"topic_shift_confidence\":6,\"simple_query_confidence\":4,\"previous_topic_summary\":\"worker execution\",\"current_topic_summary\":\"worker execution\",\"note_candidate\":\"worker submitted result\",\"digest_candidate\":\"worker submitted result\",\"reason\":\"worker finished task\"}</fin_control_feedback>\n<fin_tool_calls>[{\"tool_name\":\"project.task.submit\",\"arguments\":{\"task_id\":\"task-closed-loop\",\"result_summary\":\"worker implemented managed slice and attached receipts\"}},{\"tool_name\":\"reasoning.stop\",\"arguments\":{\"summary\":\"worker assignment completed\"}}]</fin_tool_calls>"
        } else if request
            .input
            .starts_with("Framework owner-loop directive: review submitted managed tasks now.")
        {
            "<fin_user_response>已完成 owner review。</fin_user_response>\n<fin_control_feedback>{\"origin\":\"model_output_contract_v1\",\"is_continuation\":true,\"is_simple_query\":false,\"candidate_task_id\":\"task-closed-loop\",\"candidate_topic_thread_id\":\"topic-closed-loop\",\"continuity_confidence\":96,\"topic_shift_confidence\":4,\"simple_query_confidence\":3,\"previous_topic_summary\":\"owner review\",\"current_topic_summary\":\"owner review\",\"note_candidate\":\"owner review completed\",\"digest_candidate\":\"owner review completed\",\"reason\":\"owner accepted submitted work\"}</fin_control_feedback>\n<fin_tool_calls>[{\"tool_name\":\"project.task.review\",\"arguments\":{\"task_id\":\"task-closed-loop\",\"decision\":\"approve\",\"review_summary\":\"owner accepted managed loop output\"}},{\"tool_name\":\"reasoning.stop\",\"arguments\":{\"summary\":\"owner review completed\"}}]</fin_tool_calls>"
        } else {
            "<fin_user_response>unexpected input</fin_user_response>\n<fin_control_feedback>{\"origin\":\"model_output_contract_v1\",\"is_continuation\":false,\"is_simple_query\":true,\"candidate_task_id\":null,\"candidate_topic_thread_id\":null,\"continuity_confidence\":20,\"topic_shift_confidence\":80,\"simple_query_confidence\":90,\"previous_topic_summary\":\"unknown\",\"current_topic_summary\":\"unknown\",\"reason\":\"unexpected managed loop input\"}</fin_control_feedback>\n<fin_tool_calls>[{\"tool_name\":\"reasoning.stop\",\"arguments\":{\"summary\":\"unexpected input\"}}]</fin_tool_calls>".into()
        };
        Ok(ProviderResponse {
            provider_name: request.provider_name.clone(),
            model: request.model.clone(),
            output_text: output_text.into(),
            response_id: Some("managed-loop-response".into()),
            stop_reason: Some("end_turn".into()),
            status: 200,
            tool_calls: Vec::new(),
        })
    }
}

#[test]
fn managed_closed_loop_e2e_reaches_review_done_with_full_framework_chain() {
    let home = temp_runtime_home();
    ensure_runtime_home_layout(&home).expect("runtime home should init");
    let system = map_system_config(&sample_user_toml()).expect("system config");
    let handler =
        CliDebugActionHandler::new(sample_user_toml(), system).expect("handler should build");
    let provider = ManagedLoopProvider::new(&handler.system);

    let first = handler
        .send_message_internal_with_provider(
            &home,
            ChatSendRequest {
                message:
                    "please create a formal project task for closing one managed loop end to end"
                        .into(),
                input_kind: None,
                attachments: Vec::new(),
            },
            &static_provider(&handler.system),
        )
        .expect("tentative turn should run");
    let session_id = first.binding.session_id.clone().expect("session id");
    let session_dir = crate::session_binding::find_session_dir(&home, &session_id)
        .map(|(_, _, dir)| dir)
        .expect("session dir should exist");

    let formalized = handler
        .send_message_internal_with_provider(
            &home,
            ChatSendRequest {
                message: "/formalize".into(),
                input_kind: None,
                attachments: Vec::new(),
            },
            &provider,
        )
        .expect("formalize should run");
    assert_eq!(formalized.response_kind, "system_notice");

    let dispatch = handler
        .send_message_internal_with_provider(
            &home,
            ChatSendRequest {
                message: "/tick".into(),
                input_kind: None,
                attachments: Vec::new(),
            },
            &provider,
        )
        .expect("tick should dispatch ready task");
    assert!(dispatch.answer.contains("已完成 owner dispatch"));

    let status = handler
        .send_message_internal_with_provider(
            &home,
            ChatSendRequest {
                message: "/status managed loop?".into(),
                input_kind: Some("status_probe".into()),
                attachments: Vec::new(),
            },
            &provider,
        )
        .expect("status probe should drive assignment resume first");
    assert_eq!(status.response_kind, "status_probe");
    assert!(
        status.answer.contains(
            "assignment_runtime_resume=pending=1 attempted=1 drove=1 completed=1 failed=0"
        )
    );

    let review = handler
        .send_message_internal_with_provider(
            &home,
            ChatSendRequest {
                message: "/tick".into(),
                input_kind: None,
                attachments: Vec::new(),
            },
            &provider,
        )
        .expect("tick should run owner review");
    assert!(review.answer.contains("已完成 owner review"));

    let task_json = fs::read_to_string(session_dir.join("tasks/registry/task-closed-loop.json"))
        .expect("task json");
    assert!(task_json.contains("\"status\": \"done\""));
    assert!(task_json.contains("\"latest_review_decision\": \"approve\""));
    assert!(task_json.contains("worker implemented managed slice and attached receipts"));

    let messages =
        fs::read_to_string(session_dir.join("conversation/messages.json")).expect("messages");
    assert!(messages.contains("已进入 managed path，并创建执行任务"));
    assert!(messages.contains("已完成 owner dispatch"));
    assert!(messages.contains("worker 已提交执行结果"));
    assert!(messages.contains("已完成 owner review"));
    assert!(!messages.contains("Framework planning kickoff:"));
    assert!(!messages.contains("Framework assignment: execute task now."));
    assert!(!messages.contains("Framework owner-loop directive:"));

    let tools =
        fs::read_to_string(session_dir.join("tools/recent_tool_records.json")).expect("tools");
    assert!(tools.contains("\"tool_name\": \"project.task.create\""));
    assert!(tools.contains("\"tool_name\": \"agent.assign\""));
    assert!(tools.contains("\"tool_name\": \"project.task.claim\""));
    assert!(tools.contains("\"tool_name\": \"project.task.submit\""));
    assert!(tools.contains("\"tool_name\": \"project.task.review\""));

    let events = fs::read_to_string(session_dir.join("events/stream.jsonl")).expect("events");
    assert!(events.contains("session.formalized"));
    assert!(events.contains("framework.task_kickoff_enqueued"));
    assert!(events.contains("project.task.created"));
    assert!(events.contains("agent.assignment_requested"));
    assert!(events.contains("project.task.claim_completed"));
    assert!(events.contains("project.task.submit_completed"));
    assert!(events.contains("project.task.review_completed"));
    assert!(events.contains("scheduler.tick_owner_loop_action_recorded"));
    assert!(events.contains("supervisor.cycle_completed"));
}
