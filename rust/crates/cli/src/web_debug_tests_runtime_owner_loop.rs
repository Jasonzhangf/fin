use super::*;
use fin_provider::{PreparedRequest, ProviderDescriptor, ProviderRequest, ProviderResponse};

#[derive(Debug, Clone)]
struct OwnerLoopReviewProvider {
    descriptor: ProviderDescriptor,
}

impl OwnerLoopReviewProvider {
    fn new(system: &SystemConfig) -> Self {
        Self {
            descriptor: ProviderDescriptor::from_resolved(
                system.default_provider_config().expect("default provider"),
            ),
        }
    }
}

impl fin_provider::InferenceProvider for OwnerLoopReviewProvider {
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
            .starts_with("Framework owner-loop directive: review submitted managed tasks now.")
        {
            "<fin_user_response>已完成 owner review。</fin_user_response>\n<fin_control_feedback>{\"origin\":\"model_output_contract_v1\",\"is_continuation\":true,\"is_simple_query\":false,\"candidate_task_id\":\"task-owner-review\",\"candidate_topic_thread_id\":\"topic-owner-review\",\"continuity_confidence\":96,\"topic_shift_confidence\":4,\"simple_query_confidence\":3,\"previous_topic_summary\":\"owner review\",\"current_topic_summary\":\"owner review\",\"note_candidate\":\"owner review completed\",\"digest_candidate\":\"owner review completed\",\"reason\":\"framework owner loop review done\"}</fin_control_feedback>\n<fin_tool_calls>[{\"tool_name\":\"project.task.review\",\"arguments\":{\"task_id\":\"task-owner-review\",\"decision\":\"approve\",\"review_summary\":\"owner accepted the submitted work\"}},{\"tool_name\":\"reasoning.stop\",\"arguments\":{\"summary\":\"owner review completed\"}}]</fin_tool_calls>"
        } else {
            "<fin_user_response>unexpected input</fin_user_response>\n<fin_control_feedback>{\"origin\":\"model_output_contract_v1\",\"is_continuation\":false,\"is_simple_query\":true,\"candidate_task_id\":null,\"candidate_topic_thread_id\":null,\"continuity_confidence\":20,\"topic_shift_confidence\":80,\"simple_query_confidence\":90,\"previous_topic_summary\":\"unknown\",\"current_topic_summary\":\"unknown\",\"reason\":\"unexpected owner loop input\"}</fin_control_feedback>\n<fin_tool_calls>[{\"tool_name\":\"reasoning.stop\",\"arguments\":{\"summary\":\"unexpected input\"}}]</fin_tool_calls>"
        };
        Ok(ProviderResponse {
            provider_name: request.provider_name.clone(),
            model: request.model.clone(),
            output_text: output_text.into(),
            response_id: Some("owner-loop-review-response".into()),
            stop_reason: Some("end_turn".into()),
            status: 200,
            tool_calls: Vec::new(),
        })
    }
}

#[derive(Debug, Clone)]
struct OwnerLoopDispatchProvider {
    descriptor: ProviderDescriptor,
}

impl OwnerLoopDispatchProvider {
    fn new(system: &SystemConfig) -> Self {
        Self {
            descriptor: ProviderDescriptor::from_resolved(
                system.default_provider_config().expect("default provider"),
            ),
        }
    }
}

impl fin_provider::InferenceProvider for OwnerLoopDispatchProvider {
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
            .starts_with("Framework owner-loop directive: dispatch ready managed tasks now.")
        {
            "<fin_user_response>已完成 owner dispatch。</fin_user_response>\n<fin_control_feedback>{\"origin\":\"model_output_contract_v1\",\"is_continuation\":true,\"is_simple_query\":false,\"candidate_task_id\":\"task-owner-dispatch\",\"candidate_topic_thread_id\":\"topic-owner-dispatch\",\"continuity_confidence\":95,\"topic_shift_confidence\":5,\"simple_query_confidence\":4,\"previous_topic_summary\":\"owner dispatch\",\"current_topic_summary\":\"owner dispatch\",\"note_candidate\":\"owner dispatch completed\",\"digest_candidate\":\"owner dispatch completed\",\"reason\":\"framework owner loop dispatch done\"}</fin_control_feedback>\n<fin_tool_calls>[{\"tool_name\":\"agent.assign\",\"arguments\":{\"target_worker_id\":\"worker-builder\",\"task_summary\":\"implement ready task and report receipts\"}},{\"tool_name\":\"project.task.claim\",\"arguments\":{\"task_id\":\"task-owner-dispatch\",\"worker_id\":\"worker-builder\",\"status\":\"claimed\"}},{\"tool_name\":\"reasoning.stop\",\"arguments\":{\"summary\":\"owner dispatch completed\"}}]</fin_tool_calls>"
        } else {
            "<fin_user_response>unexpected input</fin_user_response>\n<fin_control_feedback>{\"origin\":\"model_output_contract_v1\",\"is_continuation\":false,\"is_simple_query\":true,\"candidate_task_id\":null,\"candidate_topic_thread_id\":null,\"continuity_confidence\":20,\"topic_shift_confidence\":80,\"simple_query_confidence\":90,\"previous_topic_summary\":\"unknown\",\"current_topic_summary\":\"unknown\",\"reason\":\"unexpected owner loop input\"}</fin_control_feedback>\n<fin_tool_calls>[{\"tool_name\":\"reasoning.stop\",\"arguments\":{\"summary\":\"unexpected input\"}}]</fin_tool_calls>"
        };
        Ok(ProviderResponse {
            provider_name: request.provider_name.clone(),
            model: request.model.clone(),
            output_text: output_text.into(),
            response_id: Some("owner-loop-dispatch-response".into()),
            stop_reason: Some("end_turn".into()),
            status: 200,
            tool_calls: Vec::new(),
        })
    }
}

#[test]
fn tick_command_executes_owner_loop_review_and_updates_task_truth() {
    let home = temp_runtime_home();
    ensure_runtime_home_layout(&home).expect("runtime home should init");
    let system = map_system_config(&sample_user_toml()).expect("system config");
    let handler =
        CliDebugActionHandler::new(sample_user_toml(), system).expect("handler should build");
    let review_owner =
        fin_runtime::create_named_local_worker(&handler.system, &home, None, "cli", Some("system"))
            .expect("owner worker");
    let session_dir = home.join("sessions/2026/04/session-owner-loop-review");
    fs::create_dir_all(session_dir.join("conversation")).expect("conversation dir");
    fs::create_dir_all(session_dir.join("control")).expect("control dir");
    fs::create_dir_all(session_dir.join("control/owner_loop")).expect("owner loop dir");
    fs::create_dir_all(session_dir.join("queue")).expect("queue dir");
    fs::create_dir_all(session_dir.join("tasks/routing")).expect("routing dir");
    fs::create_dir_all(session_dir.join("tasks/registry")).expect("registry dir");
    fs::create_dir_all(session_dir.join("digests")).expect("digests dir");
    fs::create_dir_all(session_dir.join("context")).expect("context dir");
    fs::create_dir_all(session_dir.join("reasoning")).expect("reasoning dir");
    fs::create_dir_all(session_dir.join("tools")).expect("tools dir");
    fs::create_dir_all(session_dir.join("events")).expect("events dir");
    write_file(&session_dir.join("conversation/messages.json"), b"[]").expect("messages");
    write_file(&session_dir.join("digests/recent_digests.json"), b"[]").expect("digests");
    write_file(&session_dir.join("context/recent_contexts.json"), b"[]").expect("contexts");
    write_file(
        &session_dir.join("reasoning/recent_reasoning_views.json"),
        b"[]",
    )
    .expect("reasoning");
    write_file(&session_dir.join("tools/recent_tool_records.json"), b"[]").expect("tools");
    write_file(&session_dir.join("events/stream.jsonl"), b"").expect("events");
    write_file(
        &session_dir.join("control/execution_state.json"),
        br#"{
  "state_id":"exec-state-owner-loop-review",
  "session_id":"session-owner-loop-review",
  "task_id":"task-owner-review",
  "status":"idle",
  "active_turn_id":"turn-op-owner-loop-review",
  "active_step_id":"step-op-owner-loop-review-05-finalize",
  "resume_from_step_id":"step-op-owner-loop-review-05-finalize",
  "pending_input_count":0,
  "accepts_user_input":true,
  "reason":"owner loop ready",
  "updated_at":"2026-04-21T12:00:00+08:00"
}"#,
    )
    .expect("state");
    write_file(&session_dir.join("queue/pending_inputs.json"), b"[]").expect("pending");
    write_file(
        &session_dir.join("tasks/routing/latest_action.json"),
        br#"{"action_id":"routing-action-owner-loop-review","decision_id":"routing-owner-loop-review","operation_id":"op-owner-loop-review","trace_id":"trace-owner-loop-review","session_id":"session-owner-loop-review","task_id":"task-owner-review","created_at":"2026-04-21T12:00:00+08:00","action_kind":"continue_current_task","source_disposition":"continue_current_task","apply_immediately":true,"prompt_user":false,"prompt_text":null,"suggested_task_id":"task-owner-review","suggested_topic_thread_id":null,"confidence":97,"reason":"same task"}"#,
    )
    .expect("routing action");
    write_file(
        &session_dir.join("tasks/registry/task-owner-review.json"),
        format!(
            r#"{{
  "task_id":"task-owner-review",
  "session_id":"session-owner-loop-review",
  "title":"owner review task",
  "summary":"submitted task waiting for owner review",
  "status":"submitted",
  "review_owner_worker_id":"{}",
  "claimed_by_worker_id":"worker-project-builder",
  "submitted_by_worker_id":"worker-project-builder",
  "latest_submission_summary":"worker submitted finished change",
  "created_at":"2026-04-21T11:55:00+08:00",
  "updated_at":"2026-04-21T11:59:00+08:00"
}}"#,
            review_owner.worker_id
        )
        .as_bytes(),
    )
    .expect("task registry");
    write_file(
        &home.join("runtime/current/last_run.json"),
        br#"{
  "session_id":"session-owner-loop-review",
  "task_id":"task-owner-review",
  "session_messages_path":"sessions/2026/04/session-owner-loop-review/conversation/messages.json",
  "session_recent_contexts_path":"sessions/2026/04/session-owner-loop-review/context/recent_contexts.json",
  "session_recent_digests_path":"sessions/2026/04/session-owner-loop-review/digests/recent_digests.json",
  "session_recent_reasoning_path":"sessions/2026/04/session-owner-loop-review/reasoning/recent_reasoning_views.json",
  "session_recent_tool_records_path":"sessions/2026/04/session-owner-loop-review/tools/recent_tool_records.json"
}"#,
    )
    .expect("last_run");

    let response = handler
        .send_message_internal_with_provider(
            &home,
            ChatSendRequest {
                message: "/tick".into(),
                input_kind: None,
                attachments: Vec::new(),
            },
            &OwnerLoopReviewProvider::new(&handler.system),
        )
        .expect("tick should execute owner loop review");

    assert_eq!(response.response_kind, "assistant_message");
    assert!(response.answer.contains("已完成 owner review"));
    let task_json = fs::read_to_string(session_dir.join("tasks/registry/task-owner-review.json"))
        .expect("task json");
    assert!(task_json.contains("\"status\": \"done\""));
    assert!(task_json.contains("\"latest_review_decision\": \"approve\""));
    assert!(task_json.contains("\"reviewed_by_worker_id\":"));
    let messages =
        fs::read_to_string(session_dir.join("conversation/messages.json")).expect("messages");
    assert!(messages.contains("已完成 owner review"));
    assert!(!messages.contains("Framework owner-loop directive"));
    let latest_owner_loop =
        fs::read_to_string(session_dir.join("control/owner_loop/latest.json")).expect("owner");
    assert!(latest_owner_loop.contains("\"action_kind\":"));
    let recent_owner_loop =
        fs::read_to_string(session_dir.join("control/owner_loop/recent_actions.json"))
            .expect("recent owner");
    assert!(recent_owner_loop.contains("\"action_kind\": \"review_submitted_task\""));
    let latest_tick = fs::read_to_string(session_dir.join("control/scheduler/latest_tick.json"))
        .expect("latest tick");
    assert!(latest_tick.contains("\"source\": \"manual_tick\""));
    assert!(latest_tick.contains("\"drove_count\": 1"));
    let latest_supervisor = fs::read_to_string(session_dir.join("control/supervisor/latest.json"))
        .expect("latest supervisor");
    assert!(latest_supervisor.contains("\"source\": \"manual_tick\""));
    assert!(latest_supervisor.contains("\"drove_count\": 1"));
    let latest_tools =
        fs::read_to_string(session_dir.join("tools/recent_tool_records.json")).expect("tools");
    assert!(latest_tools.contains("\"tool_name\": \"project.task.review\""));
    let events =
        fs::read_to_string(session_dir.join("events/stream.jsonl")).expect("events stream");
    assert!(events.contains("scheduler.tick_owner_loop_action_recorded"));
    assert!(events.contains("project.task.review_completed"));
}

#[test]
fn tick_command_executes_owner_loop_dispatch_and_updates_task_and_assignment_truth() {
    let home = temp_runtime_home();
    ensure_runtime_home_layout(&home).expect("runtime home should init");
    let system = map_system_config(&sample_user_toml()).expect("system config");
    let handler =
        CliDebugActionHandler::new(sample_user_toml(), system).expect("handler should build");
    let dispatch_owner =
        fin_runtime::create_named_local_worker(&handler.system, &home, None, "cli", Some("system"))
            .expect("owner worker");
    let session_dir = home.join("sessions/2026/04/session-owner-loop-dispatch");
    fs::create_dir_all(session_dir.join("conversation")).expect("conversation dir");
    fs::create_dir_all(session_dir.join("control")).expect("control dir");
    fs::create_dir_all(session_dir.join("control/owner_loop")).expect("owner loop dir");
    fs::create_dir_all(session_dir.join("queue")).expect("queue dir");
    fs::create_dir_all(session_dir.join("tasks/routing")).expect("routing dir");
    fs::create_dir_all(session_dir.join("tasks/registry")).expect("registry dir");
    fs::create_dir_all(session_dir.join("digests")).expect("digests dir");
    fs::create_dir_all(session_dir.join("context")).expect("context dir");
    fs::create_dir_all(session_dir.join("reasoning")).expect("reasoning dir");
    fs::create_dir_all(session_dir.join("tools")).expect("tools dir");
    fs::create_dir_all(session_dir.join("events")).expect("events dir");
    write_file(&session_dir.join("conversation/messages.json"), b"[]").expect("messages");
    write_file(&session_dir.join("digests/recent_digests.json"), b"[]").expect("digests");
    write_file(&session_dir.join("context/recent_contexts.json"), b"[]").expect("contexts");
    write_file(
        &session_dir.join("reasoning/recent_reasoning_views.json"),
        b"[]",
    )
    .expect("reasoning");
    write_file(&session_dir.join("tools/recent_tool_records.json"), b"[]").expect("tools");
    write_file(&session_dir.join("events/stream.jsonl"), b"").expect("events");
    write_file(
        &session_dir.join("control/execution_state.json"),
        br#"{
  "state_id":"exec-state-owner-loop-dispatch",
  "session_id":"session-owner-loop-dispatch",
  "task_id":"task-owner-dispatch",
  "status":"idle",
  "active_turn_id":"turn-op-owner-loop-dispatch",
  "active_step_id":"step-op-owner-loop-dispatch-05-finalize",
  "resume_from_step_id":"step-op-owner-loop-dispatch-05-finalize",
  "pending_input_count":0,
  "accepts_user_input":true,
  "reason":"owner loop ready",
  "updated_at":"2026-04-21T12:10:00+08:00"
}"#,
    )
    .expect("state");
    write_file(&session_dir.join("queue/pending_inputs.json"), b"[]").expect("pending");
    write_file(
        &session_dir.join("tasks/routing/latest_action.json"),
        br#"{"action_id":"routing-action-owner-loop-dispatch","decision_id":"routing-owner-loop-dispatch","operation_id":"op-owner-loop-dispatch","trace_id":"trace-owner-loop-dispatch","session_id":"session-owner-loop-dispatch","task_id":"task-owner-dispatch","created_at":"2026-04-21T12:10:00+08:00","action_kind":"continue_current_task","source_disposition":"continue_current_task","apply_immediately":true,"prompt_user":false,"prompt_text":null,"suggested_task_id":"task-owner-dispatch","suggested_topic_thread_id":null,"confidence":97,"reason":"same task"}"#,
    )
    .expect("routing action");
    write_file(
        &session_dir.join("tasks/registry/task-owner-dispatch.json"),
        format!(
            r#"{{
  "task_id":"task-owner-dispatch",
  "session_id":"session-owner-loop-dispatch",
  "title":"owner dispatch task",
  "summary":"ready task waiting for owner dispatch",
  "status":"ready",
  "review_owner_worker_id":"{}",
  "created_at":"2026-04-21T12:05:00+08:00",
  "updated_at":"2026-04-21T12:09:00+08:00"
}}"#,
            dispatch_owner.worker_id
        )
        .as_bytes(),
    )
    .expect("task registry");
    write_file(
        &home.join("runtime/current/last_run.json"),
        br#"{
  "session_id":"session-owner-loop-dispatch",
  "task_id":"task-owner-dispatch",
  "session_messages_path":"sessions/2026/04/session-owner-loop-dispatch/conversation/messages.json",
  "session_recent_contexts_path":"sessions/2026/04/session-owner-loop-dispatch/context/recent_contexts.json",
  "session_recent_digests_path":"sessions/2026/04/session-owner-loop-dispatch/digests/recent_digests.json",
  "session_recent_reasoning_path":"sessions/2026/04/session-owner-loop-dispatch/reasoning/recent_reasoning_views.json",
  "session_recent_tool_records_path":"sessions/2026/04/session-owner-loop-dispatch/tools/recent_tool_records.json"
}"#,
    )
    .expect("last_run");

    let response = handler
        .send_message_internal_with_provider(
            &home,
            ChatSendRequest {
                message: "/tick".into(),
                input_kind: None,
                attachments: Vec::new(),
            },
            &OwnerLoopDispatchProvider::new(&handler.system),
        )
        .expect("tick should execute owner loop dispatch");

    assert_eq!(response.response_kind, "assistant_message");
    assert!(response.answer.contains("已完成 owner dispatch"));
    let task_json = fs::read_to_string(session_dir.join("tasks/registry/task-owner-dispatch.json"))
        .expect("task json");
    assert!(task_json.contains("\"status\": \"claimed\""));
    assert!(task_json.contains("\"claimed_by_worker_id\": \"worker-builder\""));
    let assignment_queue =
        fs::read_to_string(home.join("runtime/assignments/pending.json")).expect("assignments");
    assert!(assignment_queue.contains("\"target_worker_id\": \"worker-builder\""));
    assert!(assignment_queue.contains("\"peer_id\": \"local-worker-builder\""));
    assert!(
        assignment_queue
            .contains(format!("\"owner_worker_id\": \"{}\"", dispatch_owner.worker_id).as_str())
    );
    let messages =
        fs::read_to_string(session_dir.join("conversation/messages.json")).expect("messages");
    assert!(messages.contains("已完成 owner dispatch"));
    assert!(!messages.contains("Framework owner-loop directive"));
    let recent_owner_loop =
        fs::read_to_string(session_dir.join("control/owner_loop/recent_actions.json"))
            .expect("recent owner");
    assert!(recent_owner_loop.contains("\"action_kind\": \"dispatch_ready_task\""));
    let latest_tick = fs::read_to_string(session_dir.join("control/scheduler/latest_tick.json"))
        .expect("latest tick");
    assert!(latest_tick.contains("\"source\": \"manual_tick\""));
    assert!(latest_tick.contains("\"drove_count\": 1"));
    let latest_tools =
        fs::read_to_string(session_dir.join("tools/recent_tool_records.json")).expect("tools");
    assert!(latest_tools.contains("\"tool_name\": \"agent.assign\""));
    assert!(latest_tools.contains("\"tool_name\": \"project.task.claim\""));
    let events =
        fs::read_to_string(session_dir.join("events/stream.jsonl")).expect("events stream");
    assert!(events.contains("scheduler.tick_owner_loop_action_recorded"));
    assert!(events.contains("agent.assignment_requested"));
    assert!(events.contains("project.task.claim_completed"));
}
