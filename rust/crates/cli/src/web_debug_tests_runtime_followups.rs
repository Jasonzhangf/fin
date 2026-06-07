use super::*;
use fin_provider::{PreparedRequest, ProviderDescriptor, ProviderRequest, ProviderResponse};

#[derive(Debug, Clone)]
struct ResumeCheckpointProvider {
    descriptor: ProviderDescriptor,
}

impl ResumeCheckpointProvider {
    fn new(system: &SystemConfig) -> Self {
        Self {
            descriptor: ProviderDescriptor::from_resolved(
                system.default_provider_config().expect("default provider"),
            ),
        }
    }
}

impl fin_provider::InferenceProvider for ResumeCheckpointProvider {
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
            output_text: "<fin_user_response>提醒已到，继续并完成检查。</fin_user_response>\n<fin_control_feedback>{\"origin\":\"model_output_contract_v1\",\"is_continuation\":true,\"is_simple_query\":false,\"candidate_task_id\":\"task-auto-resume\",\"candidate_topic_thread_id\":\"topic-auto-resume\",\"continuity_confidence\":94,\"topic_shift_confidence\":6,\"simple_query_confidence\":4,\"previous_topic_summary\":\"waiting\",\"current_topic_summary\":\"waiting\",\"note_candidate\":\"resume finished\",\"digest_candidate\":\"resume finished\",\"reason\":\"checkpoint resume completed\"}</fin_control_feedback>\n<fin_tool_calls>[{\"tool_name\":\"reasoning.stop\",\"arguments\":{\"summary\":\"checkpoint resume complete\"}}]</fin_tool_calls>".into(),
            response_id: Some("resume-checkpoint-response".into()),
            stop_reason: Some("end_turn".into()),
            status: 200,
            tool_calls: Vec::new(),
        })
    }
}

#[test]
fn due_reminder_auto_ticks_scheduler_and_drains_pending_queue() {
    let home = temp_runtime_home();
    ensure_runtime_home_layout(&home).expect("runtime home should init");
    let system = map_system_config(&sample_user_toml()).expect("system config");
    let handler =
        CliDebugActionHandler::new(sample_user_toml(), system).expect("handler should build");
    let session_dir = home.join("sessions/2026/06/session-auto-wake");
    fs::create_dir_all(session_dir.join("conversation")).expect("conversation dir");
    fs::create_dir_all(session_dir.join("control")).expect("control dir");
    fs::create_dir_all(session_dir.join("queue")).expect("queue dir");
    fs::create_dir_all(session_dir.join("tasks/routing")).expect("routing dir");
    fs::create_dir_all(session_dir.join("digests")).expect("digests dir");
    fs::create_dir_all(session_dir.join("context")).expect("context dir");
    fs::create_dir_all(session_dir.join("reasoning")).expect("reasoning dir");
    fs::create_dir_all(session_dir.join("tools")).expect("tools dir");
    fs::create_dir_all(session_dir.join("events")).expect("events dir");
    fs::create_dir_all(home.join("runtime/reminders")).expect("reminders dir");
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
  "state_id":"exec-state-waiting",
  "session_id":"session-auto-wake",
  "task_id":"task-auto-wake",
  "status":"waiting_external",
  "active_turn_id":"turn-op-waiting",
  "active_step_id":"step-op-waiting-05-finalize",
  "resume_from_step_id":"step-op-waiting-05-finalize",
  "pending_input_count":1,
  "accepts_user_input":true,
  "reason":"waiting for reminder or external result",
  "updated_at":"2026-04-19T22:50:00+08:00"
}"#,
    )
    .expect("state");
    write_file(
            &session_dir.join("queue/pending_inputs.json"),
            br#"[{"pending_input_id":"pending-1","session_id":"session-auto-wake","task_id":"task-auto-wake","input_kind":"chat","message":"wake queued task","status":"pending","enqueue_reason":"wait","enqueued_at":"2026-04-19T22:50:01+08:00"}]"#,
        )
        .expect("pending");
    write_file(
            &session_dir.join("tasks/routing/latest_action.json"),
            br#"{"action_id":"routing-action-op-wake","decision_id":"routing-op-wake","operation_id":"op-wake","trace_id":"trace-wake","session_id":"session-auto-wake","task_id":"task-auto-wake","created_at":"2026-04-19T22:50:00+08:00","action_kind":"continue_current_task","source_disposition":"continue_current_task","apply_immediately":true,"prompt_user":false,"prompt_text":null,"suggested_task_id":"task-auto-wake","suggested_topic_thread_id":null,"confidence":95,"reason":"same task"}"#,
        )
        .expect("routing action");
    write_file(
            &home.join("runtime/reminders/pending.json"),
            format!(
                r#"[{{"reminder_id":"reminder-1","session_id":"session-auto-wake","task_id":"task-auto-wake","operation_id":"op-reminder","trace_id":"trace-reminder","wait_minutes":1,"reminder":"check wake","wake_role":"system","scheduled_at":"{}","status":"pending","fired_at":null}}]"#,
                (chrono::Local::now() - chrono::Duration::minutes(2))
                    .format("%Y-%m-%dT%H:%M:%S%:z")
            )
            .as_bytes(),
        )
        .expect("reminders");
    write_file(
            &home.join("runtime/current/last_run.json"),
            br#"{
  "session_id":"session-auto-wake",
  "task_id":"task-auto-wake",
  "session_messages_path":"sessions/2026/06/session-auto-wake/conversation/messages.json",
  "session_recent_contexts_path":"sessions/2026/06/session-auto-wake/context/recent_contexts.json",
  "session_recent_digests_path":"sessions/2026/06/session-auto-wake/digests/recent_digests.json",
  "session_recent_reasoning_path":"sessions/2026/06/session-auto-wake/reasoning/recent_reasoning_views.json",
  "session_recent_tool_records_path":"sessions/2026/06/session-auto-wake/tools/recent_tool_records.json"
}"#,
        )
        .expect("last_run");

    let response = handler
        .send_message_internal_with_provider(
            &home,
            ChatSendRequest {
                message: "/status wake?".into(),
                input_kind: Some("status_probe".into()),
                attachments: Vec::new(),
            },
            &static_provider(&handler.system),
        )
        .expect("status after wake");
    assert_eq!(response.response_kind, "status_probe");
    assert!(response.answer.contains("scheduler="));
    assert!(response.answer.contains("tick="));
    assert!(response.answer.contains("supervisor="));
    assert!(response.answer.contains("heartbeat="));
    assert!(response.answer.contains("daemon="));
    assert!(response.answer.contains("recovery="));
    assert!(response.answer.contains("source=reminder_fired"));
    let pending_after =
        fs::read_to_string(session_dir.join("queue/pending_inputs.json")).expect("pending");
    assert_eq!(pending_after.trim(), "[]");
    let messages =
        fs::read_to_string(session_dir.join("conversation/messages.json")).expect("messages");
    assert!(messages.contains("Reminder"));
    assert!(messages.contains("wake queued task"));
    let latest_tick = fs::read_to_string(session_dir.join("control/scheduler/latest_tick.json"))
        .expect("latest tick");
    assert!(latest_tick.contains("\"source\": \"reminder_fired\""));
    assert!(latest_tick.contains("\"drove_count\": 1"));
    let latest_supervisor = fs::read_to_string(session_dir.join("control/supervisor/latest.json"))
        .expect("latest supervisor");
    assert!(latest_supervisor.contains("\"source\": \"reminder_fired\""));
    assert!(latest_supervisor.contains("\"tick_count\": 1"));
    let latest_heartbeat =
        fs::read_to_string(session_dir.join("control/supervisor/latest_heartbeat.json"))
            .expect("latest heartbeat");
    assert!(latest_heartbeat.contains("\"source\": \"web_debug_request\""));
    let latest_daemon = fs::read_to_string(session_dir.join("control/daemon/latest_state.json"))
        .expect("latest daemon");
    assert!(latest_daemon.contains("\"service_kind\": \"web_debug_attached\""));
    let latest_recovery =
        fs::read_to_string(session_dir.join("control/daemon/latest_recovery_action.json"))
            .expect("latest recovery");
    assert!(latest_recovery.contains("\"action_kind\":"));
    let events =
        fs::read_to_string(session_dir.join("events/stream.jsonl")).expect("events stream");
    assert!(events.contains("scheduler.tick_started"));
    assert!(events.contains("scheduler.tick_drove_pending"));
    assert!(events.contains("scheduler.tick_completed"));
    assert!(events.contains("supervisor.cycle_started"));
    assert!(events.contains("supervisor.cycle_completed"));
    assert!(events.contains("supervisor.heartbeat_recorded"));
    assert!(events.contains("daemon.state_recorded"));
    assert!(events.contains("daemon.recovery_action_derived"));
}

#[test]
fn slash_compact_rebuilds_current_context_without_provider_call() {
    let home = temp_runtime_home();
    ensure_runtime_home_layout(&home).expect("runtime home should init");
    let system = map_system_config(&sample_user_toml()).expect("system config");
    let handler =
        CliDebugActionHandler::new(sample_user_toml(), system).expect("handler should build");
    let session_dir = home.join("sessions/2026/06/session-compact");
    fs::create_dir_all(session_dir.join("conversation")).expect("conversation dir");
    fs::create_dir_all(session_dir.join("digests")).expect("digests dir");
    fs::create_dir_all(session_dir.join("context")).expect("context dir");
    fs::create_dir_all(session_dir.join("reasoning")).expect("reasoning dir");
    fs::create_dir_all(session_dir.join("tools")).expect("tools dir");
    write_file(
            &session_dir.join("conversation/messages.json"),
            br#"[{"message_id":"user-1","role":"user","content":"keep context","created_at":"2026-04-18T08:10:00+08:00","session_id":"session-compact","task_id":"task-compact"}]"#,
        )
        .expect("messages");
    write_file(&session_dir.join("digests/recent_digests.json"), b"[]").expect("digests");
    write_file(&session_dir.join("context/recent_contexts.json"), b"[]").expect("contexts");
    write_file(
        &session_dir.join("reasoning/recent_reasoning_views.json"),
        b"[]",
    )
    .expect("reasoning");
    write_file(&session_dir.join("tools/recent_tool_records.json"), b"[]").expect("tools");
    write_file(
        &home.join("runtime/current/last_run.json"),
        br#"{
  "session_id":"session-compact",
  "task_id":"task-compact",
  "session_messages_path":"sessions/2026/06/session-compact/conversation/messages.json",
  "session_recent_contexts_path":"sessions/2026/06/session-compact/context/recent_contexts.json",
  "session_recent_digests_path":"sessions/2026/06/session-compact/digests/recent_digests.json"
}"#,
    )
    .expect("last_run");

    let response = handler
        .send_message_internal(
            &home,
            ChatSendRequest {
                message: "/compact".into(),
                input_kind: None,
                attachments: Vec::new(),
            },
        )
        .expect("compact command should work");
    assert_eq!(response.response_kind, "system_notice");
    assert!(home.join("runtime/current/current_context.json").exists());
    assert!(
        home.join("runtime/current/current_rebuild_index.json")
            .exists()
    );
}

#[test]
fn due_reminder_prefers_execution_checkpoint_resume_without_fake_user_message() {
    let home = temp_runtime_home();
    ensure_runtime_home_layout(&home).expect("runtime home should init");
    let system = map_system_config(&sample_user_toml()).expect("system config");
    let handler =
        CliDebugActionHandler::new(sample_user_toml(), system).expect("handler should build");
    let session_dir = home.join("sessions/2026/06/session-checkpoint-resume");
    for relative in [
        "conversation",
        "control",
        "queue",
        "tasks/routing",
        "digests",
        "context",
        "reasoning",
        "tools",
        "events",
    ] {
        fs::create_dir_all(session_dir.join(relative)).expect("session dir");
    }
    fs::create_dir_all(home.join("runtime/reminders")).expect("reminders dir");
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
    write_file(&session_dir.join("queue/pending_inputs.json"), b"[]").expect("pending");
    write_file(
        &session_dir.join("control/execution_state.json"),
        br#"{
  "state_id":"exec-state-waiting",
  "session_id":"session-checkpoint-resume",
  "task_id":"task-auto-resume",
  "status":"waiting_external",
  "active_turn_id":"turn-op-waiting",
  "active_step_id":"step-op-waiting-05-finalize",
  "resume_from_step_id":"step-op-waiting-04-tool_dispatch",
  "resume_checkpoint_ready":true,
  "resume_checkpoint_id":"checkpoint-op-waiting-r02",
  "pending_input_count":0,
  "accepts_user_input":true,
  "reason":"waiting for reminder or external result; resumable checkpoint ready",
  "updated_at":"2026-04-20T10:00:00+08:00"
}"#,
    )
    .expect("state");
    write_file(
        &session_dir.join("control/execution_checkpoint.json"),
        "{
  \"checkpoint_id\":\"checkpoint-op-waiting-r02\",
  \"session_id\":\"session-checkpoint-resume\",
  \"task_id\":\"task-auto-resume\",
  \"trace_id\":\"trace-op-waiting\",
  \"source_operation_id\":\"op-waiting\",
  \"source_turn_id\":\"turn-op-waiting\",
  \"source_step_id\":\"step-op-waiting-04-tool_dispatch\",
  \"checkpoint_kind\":\"wait_reminder_resume\",
  \"status\":\"open\",
  \"source_round_index\":1,
  \"next_round_index\":2,
  \"resume_input\":\"Continue the same turn with the latest tool results.\\nOriginal request: 等两分钟再看日志\\nLast assistant response: 先等待日志完成。\\nExecuted tool results (authoritative client facts):\\n- tool=wait.remind status=completed target=role=system result=scheduled system self reminder in 2 minute(s)\\nInspect these tool results before deciding whether another tool is needed. If the task is complete, answer directly and emit reasoning.stop.\",
  \"summary\":\"resume round 2 after wait.remind from round 1\",
  \"created_at\":\"2026-04-20T10:00:00+08:00\",
  \"consumed_at\":null,
  \"consumed_by_operation_id\":null
}"
        .as_bytes(),
    )
    .expect("checkpoint");
    write_file(
        &session_dir.join("control/recent_execution_checkpoints.json"),
        "[{\"checkpoint_id\":\"checkpoint-op-waiting-r02\",\"session_id\":\"session-checkpoint-resume\",\"task_id\":\"task-auto-resume\",\"trace_id\":\"trace-op-waiting\",\"source_operation_id\":\"op-waiting\",\"source_turn_id\":\"turn-op-waiting\",\"source_step_id\":\"step-op-waiting-04-tool_dispatch\",\"checkpoint_kind\":\"wait_reminder_resume\",\"status\":\"open\",\"source_round_index\":1,\"next_round_index\":2,\"resume_input\":\"Continue the same turn...\",\"summary\":\"resume round 2 after wait.remind from round 1\",\"created_at\":\"2026-04-20T10:00:00+08:00\",\"consumed_at\":null,\"consumed_by_operation_id\":null}]"
            .as_bytes(),
    )
    .expect("recent checkpoints");
    write_file(
        &session_dir.join("tasks/routing/latest_action.json"),
        br#"{"action_id":"routing-action-op-waiting","decision_id":"routing-op-waiting","operation_id":"op-waiting","trace_id":"trace-op-waiting","session_id":"session-checkpoint-resume","task_id":"task-auto-resume","created_at":"2026-04-20T10:00:00+08:00","action_kind":"continue_current_task","source_disposition":"continue_current_task","apply_immediately":true,"prompt_user":false,"prompt_text":null,"suggested_task_id":"task-auto-resume","suggested_topic_thread_id":null,"confidence":95,"reason":"same task"}"#,
    )
    .expect("routing action");
    write_file(
        &home.join("runtime/reminders/pending.json"),
        format!(
            r#"[{{"reminder_id":"reminder-1","session_id":"session-checkpoint-resume","task_id":"task-auto-resume","operation_id":"op-waiting","trace_id":"trace-op-waiting","wait_minutes":1,"reminder":"check wait result","wake_role":"system","scheduled_at":"{}","status":"pending","fired_at":null}}]"#,
            (chrono::Local::now() - chrono::Duration::minutes(2))
                .format("%Y-%m-%dT%H:%M:%S%:z")
        )
        .as_bytes(),
    )
    .expect("reminders");
    write_file(
        &home.join("runtime/current/last_run.json"),
        br#"{
  "session_id":"session-checkpoint-resume",
  "task_id":"task-auto-resume",
  "session_messages_path":"sessions/2026/06/session-checkpoint-resume/conversation/messages.json",
  "session_recent_contexts_path":"sessions/2026/06/session-checkpoint-resume/context/recent_contexts.json",
  "session_recent_digests_path":"sessions/2026/06/session-checkpoint-resume/digests/recent_digests.json",
  "session_recent_reasoning_path":"sessions/2026/06/session-checkpoint-resume/reasoning/recent_reasoning_views.json",
  "session_recent_tool_records_path":"sessions/2026/06/session-checkpoint-resume/tools/recent_tool_records.json"
}"#,
    )
    .expect("last_run");

    let response = handler
        .send_message_internal_with_provider(
            &home,
            ChatSendRequest {
                message: "/status resume?".into(),
                input_kind: Some("status_probe".into()),
                attachments: Vec::new(),
            },
            &ResumeCheckpointProvider::new(&handler.system),
        )
        .expect("status after checkpoint resume");
    assert_eq!(response.response_kind, "status_probe");
    let checkpoint = fs::read_to_string(session_dir.join("control/execution_checkpoint.json"))
        .expect("checkpoint");
    assert!(checkpoint.contains("\"status\": \"consumed\""));
    let messages =
        fs::read_to_string(session_dir.join("conversation/messages.json")).expect("messages");
    assert!(messages.contains("Reminder"));
    assert!(messages.contains("提醒已到，继续并完成检查"));
    assert!(!messages.contains("Continue the same turn with the latest tool results."));
    let events = fs::read_to_string(session_dir.join("events/stream.jsonl")).expect("events");
    assert!(events.contains("execution.checkpoint_consumed"));
    assert!(events.contains("reasoning.stopped"));
}
