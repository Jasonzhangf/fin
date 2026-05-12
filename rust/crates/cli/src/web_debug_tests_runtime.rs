use super::*;
#[test]
fn pause_and_resume_run_commands_toggle_execution_state() {
    let home = temp_runtime_home();
    ensure_runtime_home_layout(&home).expect("runtime home should init");
    let system = map_system_config(&sample_user_toml()).expect("system config");
    let handler =
        CliDebugActionHandler::new(sample_user_toml(), system).expect("handler should build");

    handler
        .send_message_internal(
            &home,
            ChatSendRequest {
                message: "/new".into(),
                input_kind: None,
                attachments: Vec::new(),
            },
        )
        .expect("new");

    let paused = handler
        .send_message_internal(
            &home,
            ChatSendRequest {
                message: "/pause testing".into(),
                input_kind: None,
                attachments: Vec::new(),
            },
        )
        .expect("pause");
    assert!(paused.answer.contains("execution paused"));
    let last_run = read_last_run_value(&home).expect("last run after pause");
    let segment_path = last_run
        .get("current_interrupted_segment_path")
        .and_then(serde_json::Value::as_str)
        .expect("segment path");
    let segment_json = fs::read_to_string(home.join(segment_path)).expect("segment should exist");
    assert!(segment_json.contains("\"status\": \"open\""));

    let resumed = handler
        .send_message_internal(
            &home,
            ChatSendRequest {
                message: "/resume-run".into(),
                input_kind: None,
                attachments: Vec::new(),
            },
        )
        .expect("resume run");
    assert!(resumed.answer.contains("execution resumed"));
    let last_run = read_last_run_value(&home).expect("last run");
    let state_path = last_run
        .get("current_execution_state_path")
        .and_then(serde_json::Value::as_str)
        .expect("state path");
    let state_json =
        fs::read_to_string(home.join(state_path)).expect("execution state should exist");
    assert!(state_json.contains("\"status\": \"idle\""));
}

#[test]
fn interrupt_request_runs_immediately_and_preserves_open_segment() {
    let home = temp_runtime_home();
    ensure_runtime_home_layout(&home).expect("runtime home should init");
    let system = map_system_config(&sample_user_toml()).expect("system config");
    let handler =
        CliDebugActionHandler::new(sample_user_toml(), system).expect("handler should build");
    let session_dir = home.join("sessions/2026/05/session-interrupt");
    fs::create_dir_all(session_dir.join("conversation")).expect("conversation dir");
    fs::create_dir_all(session_dir.join("control")).expect("control dir");
    fs::create_dir_all(session_dir.join("queue")).expect("queue dir");
    fs::create_dir_all(session_dir.join("digests")).expect("digests dir");
    fs::create_dir_all(session_dir.join("context")).expect("context dir");
    fs::create_dir_all(session_dir.join("reasoning")).expect("reasoning dir");
    fs::create_dir_all(session_dir.join("tools")).expect("tools dir");
    fs::create_dir_all(session_dir.join("events")).expect("events dir");
    fs::create_dir_all(session_dir.join("interrupts")).expect("interrupts dir");
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
  "state_id":"exec-state-paused",
  "session_id":"session-interrupt",
  "task_id":"task-interrupt",
  "status":"paused",
  "active_turn_id":"turn-op-paused",
  "active_step_id":"step-op-paused-05-tool_dispatch",
  "resume_from_step_id":"step-op-paused-05-tool_dispatch",
  "pending_input_count":0,
  "accepts_user_input":false,
  "reason":"manual pause",
  "updated_at":"2026-04-18T08:10:02+08:00"
}"#,
    )
    .expect("state");
    write_file(
            &session_dir.join("interrupts/recent_segments.json"),
            br#"[{"segment_id":"segment-session-interrupt","session_id":"session-interrupt","task_id":"task-interrupt","interrupted_turn_id":"turn-op-paused","interrupted_step_id":"step-op-paused-05-tool_dispatch","resume_from_step_id":"step-op-paused-05-tool_dispatch","status":"open","reason":"manual pause","created_at":"2026-04-18T08:10:02+08:00","merged_into_turn_id":null,"merged_into_operation_id":null,"merged_at":null}]"#,
        )
        .expect("segments");
    write_file(&session_dir.join("interrupts/recent_merges.json"), b"[]").expect("merges");
    write_file(&session_dir.join("queue/pending_inputs.json"), b"[]").expect("pending");
    write_file(
            &home.join("runtime/current/last_run.json"),
            br#"{
  "session_id":"session-interrupt",
  "task_id":"task-interrupt",
  "session_messages_path":"sessions/2026/05/session-interrupt/conversation/messages.json",
  "session_recent_contexts_path":"sessions/2026/05/session-interrupt/context/recent_contexts.json",
  "session_recent_digests_path":"sessions/2026/05/session-interrupt/digests/recent_digests.json",
  "session_recent_reasoning_path":"sessions/2026/05/session-interrupt/reasoning/recent_reasoning_views.json",
  "session_recent_tool_records_path":"sessions/2026/05/session-interrupt/tools/recent_tool_records.json"
}"#,
        )
        .expect("last_run");

    let response = handler
        .send_message_internal_with_provider(
            &home,
            ChatSendRequest {
                message: "/interrupt fix this now".into(),
                input_kind: None,
                attachments: Vec::new(),
            },
            &static_provider(&handler.system),
        )
        .expect("interrupt should execute immediately");
    assert_eq!(response.response_kind, "assistant_message");
    assert!(response.answer.contains("fix this now"));
    assert!(response.routing_action.is_some());
    let pending_after =
        fs::read_to_string(session_dir.join("queue/pending_inputs.json")).expect("pending");
    assert_eq!(pending_after.trim(), "[]");
    let messages =
        fs::read_to_string(session_dir.join("conversation/messages.json")).expect("messages");
    assert!(messages.contains("fix this now"));
    let merges =
        fs::read_to_string(session_dir.join("interrupts/recent_merges.json")).expect("merges");
    assert_eq!(merges.trim(), "[]");
    let segments =
        fs::read_to_string(session_dir.join("interrupts/recent_segments.json")).expect("segments");
    assert!(segments.contains("\"status\":\"open\"") || segments.contains("\"status\": \"open\""));
    assert!(
        !segments.contains("\"status\":\"merged\"") && !segments.contains("\"status\": \"merged\"")
    );
}

#[test]
fn resume_run_auto_drains_multiple_pending_inputs_until_queue_empty() {
    let home = temp_runtime_home();
    ensure_runtime_home_layout(&home).expect("runtime home should init");
    let system = map_system_config(&sample_user_toml()).expect("system config");
    let handler =
        CliDebugActionHandler::new(sample_user_toml(), system).expect("handler should build");
    let session_dir = home.join("sessions/2026/05/session-resume-drain");
    fs::create_dir_all(session_dir.join("conversation")).expect("conversation dir");
    fs::create_dir_all(session_dir.join("control")).expect("control dir");
    fs::create_dir_all(session_dir.join("queue")).expect("queue dir");
    fs::create_dir_all(session_dir.join("digests")).expect("digests dir");
    fs::create_dir_all(session_dir.join("context")).expect("context dir");
    fs::create_dir_all(session_dir.join("reasoning")).expect("reasoning dir");
    fs::create_dir_all(session_dir.join("tools")).expect("tools dir");
    fs::create_dir_all(session_dir.join("events")).expect("events dir");
    fs::create_dir_all(session_dir.join("interrupts")).expect("interrupts dir");
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
  "state_id":"exec-state-paused",
  "session_id":"session-resume-drain",
  "task_id":"task-resume-drain",
  "status":"paused",
  "active_turn_id":"turn-op-paused",
  "active_step_id":"step-op-paused-05-tool_dispatch",
  "resume_from_step_id":"step-op-paused-05-tool_dispatch",
  "pending_input_count":1,
  "accepts_user_input":false,
  "reason":"manual pause",
  "updated_at":"2026-04-18T08:10:02+08:00"
}"#,
    )
    .expect("state");
    write_file(
            &session_dir.join("interrupts/recent_segments.json"),
            br#"[{"segment_id":"segment-session-resume-drain","session_id":"session-resume-drain","task_id":"task-resume-drain","interrupted_turn_id":"turn-op-paused","interrupted_step_id":"step-op-paused-05-tool_dispatch","resume_from_step_id":"step-op-paused-05-tool_dispatch","status":"open","reason":"manual pause","created_at":"2026-04-18T08:10:02+08:00","merged_into_turn_id":null,"merged_into_operation_id":null,"merged_at":null}]"#,
        )
        .expect("segments");
    write_file(&session_dir.join("interrupts/recent_merges.json"), b"[]").expect("merges");
    write_file(
            &session_dir.join("queue/pending_inputs.json"),
            br#"[{"pending_input_id":"pending-1","session_id":"session-resume-drain","task_id":"task-resume-drain","input_kind":"chat","message":"resume queued task 1","status":"pending","enqueue_reason":"manual pause","enqueued_at":"2026-04-18T08:11:00+08:00"},{"pending_input_id":"pending-2","session_id":"session-resume-drain","task_id":"task-resume-drain","input_kind":"chat","message":"resume queued task 2","status":"pending","enqueue_reason":"manual pause","enqueued_at":"2026-04-18T08:12:00+08:00"},{"pending_input_id":"pending-3","session_id":"session-resume-drain","task_id":"task-resume-drain","input_kind":"chat","message":"resume queued task 3","status":"pending","enqueue_reason":"manual pause","enqueued_at":"2026-04-18T08:13:00+08:00"}]"#,
        )
        .expect("pending");
    write_file(
            &home.join("runtime/current/last_run.json"),
            br#"{
  "session_id":"session-resume-drain",
  "task_id":"task-resume-drain",
  "session_messages_path":"sessions/2026/05/session-resume-drain/conversation/messages.json",
  "session_recent_contexts_path":"sessions/2026/05/session-resume-drain/context/recent_contexts.json",
  "session_recent_digests_path":"sessions/2026/05/session-resume-drain/digests/recent_digests.json",
  "session_recent_reasoning_path":"sessions/2026/05/session-resume-drain/reasoning/recent_reasoning_views.json",
  "session_recent_tool_records_path":"sessions/2026/05/session-resume-drain/tools/recent_tool_records.json"
}"#,
        )
        .expect("last_run");

    let response = handler
        .send_message_internal_with_provider(
            &home,
            ChatSendRequest {
                message: "/resume-run".into(),
                input_kind: None,
                attachments: Vec::new(),
            },
            &static_provider(&handler.system),
        )
        .expect("resume run should execute");
    assert_eq!(response.response_kind, "assistant_message");
    assert!(response.answer.contains("resume queued task 3"));
    let pending_after =
        fs::read_to_string(session_dir.join("queue/pending_inputs.json")).expect("pending");
    assert_eq!(pending_after.trim(), "[]");
    let messages =
        fs::read_to_string(session_dir.join("conversation/messages.json")).expect("messages");
    assert!(messages.contains("resume queued task 1"));
    assert!(messages.contains("resume queued task 2"));
    assert!(messages.contains("resume queued task 3"));
    let merges =
        fs::read_to_string(session_dir.join("interrupts/recent_merges.json")).expect("merges");
    assert!(merges.contains("\"strategy\": \"resume_as_new_closure\""));
    assert_eq!(merges.matches("\"merge_id\"").count(), 1);
    let segments =
        fs::read_to_string(session_dir.join("interrupts/recent_segments.json")).expect("segments");
    assert!(segments.contains("\"status\": \"merged\""));
}

#[test]
fn tick_command_drives_pending_queue_when_scheduler_allows() {
    let home = temp_runtime_home();
    ensure_runtime_home_layout(&home).expect("runtime home should init");
    let system = map_system_config(&sample_user_toml()).expect("system config");
    let handler =
        CliDebugActionHandler::new(sample_user_toml(), system).expect("handler should build");
    let session_dir = home.join("sessions/2026/05/session-tick");
    fs::create_dir_all(session_dir.join("conversation")).expect("conversation dir");
    fs::create_dir_all(session_dir.join("control")).expect("control dir");
    fs::create_dir_all(session_dir.join("queue")).expect("queue dir");
    fs::create_dir_all(session_dir.join("tasks/routing")).expect("routing dir");
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
  "state_id":"exec-state-idle",
  "session_id":"session-tick",
  "task_id":"task-tick",
  "status":"idle",
  "active_turn_id":"turn-op-idle",
  "active_step_id":"step-op-idle-05-finalize",
  "resume_from_step_id":"step-op-idle-05-finalize",
  "pending_input_count":1,
  "accepts_user_input":true,
  "reason":"ready",
  "updated_at":"2026-04-19T22:40:00+08:00"
}"#,
    )
    .expect("state");
    write_file(
            &session_dir.join("queue/pending_inputs.json"),
            br#"[{"pending_input_id":"pending-1","session_id":"session-tick","task_id":"task-tick","input_kind":"chat","source":"channel.qqbot","message":"tick queued task","attachments":[{"name":"tick-proof.png","kind":"image/png"}],"status":"pending","enqueue_reason":"test","enqueued_at":"2026-04-19T22:40:01+08:00"}]"#,
        )
        .expect("pending");
    write_file(
            &session_dir.join("tasks/routing/latest_action.json"),
            br#"{"action_id":"routing-action-op-tick","decision_id":"routing-op-tick","operation_id":"op-tick","trace_id":"trace-tick","session_id":"session-tick","task_id":"task-tick","created_at":"2026-04-19T22:40:00+08:00","action_kind":"continue_current_task","source_disposition":"continue_current_task","apply_immediately":true,"prompt_user":false,"prompt_text":null,"suggested_task_id":"task-tick","suggested_topic_thread_id":null,"confidence":94,"reason":"same task"}"#,
        )
        .expect("routing action");
    write_file(
            &home.join("runtime/current/last_run.json"),
            br#"{
  "session_id":"session-tick",
  "task_id":"task-tick",
  "session_messages_path":"sessions/2026/05/session-tick/conversation/messages.json",
  "session_recent_contexts_path":"sessions/2026/05/session-tick/context/recent_contexts.json",
  "session_recent_digests_path":"sessions/2026/05/session-tick/digests/recent_digests.json",
  "session_recent_reasoning_path":"sessions/2026/05/session-tick/reasoning/recent_reasoning_views.json",
  "session_recent_tool_records_path":"sessions/2026/05/session-tick/tools/recent_tool_records.json"
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
            &static_provider(&handler.system),
        )
        .expect("tick should execute");
    assert_eq!(response.response_kind, "assistant_message");
    assert!(response.answer.contains("tick queued task"));
    let pending_after =
        fs::read_to_string(session_dir.join("queue/pending_inputs.json")).expect("pending");
    assert_eq!(pending_after.trim(), "[]");
    let latest_tick = fs::read_to_string(session_dir.join("control/scheduler/latest_tick.json"))
        .expect("latest tick");
    assert!(latest_tick.contains("\"source\": \"manual_tick\""));
    assert!(latest_tick.contains("\"drove_count\": 1"));
    let latest_supervisor = fs::read_to_string(session_dir.join("control/supervisor/latest.json"))
        .expect("latest supervisor");
    assert!(latest_supervisor.contains("\"source\": \"manual_tick\""));
    assert!(latest_supervisor.contains("\"tick_count\": 1"));
    let current_context =
        fs::read_to_string(home.join("runtime/current/current_context.json")).expect("context");
    assert!(current_context.contains("\"source\": \"channel.qqbot\""));
    assert!(current_context.contains("\"role_id\": \"system\""));
    assert!(current_context.contains("tick-proof.png"));
    let last_run =
        fs::read_to_string(home.join("runtime/current/last_run.json")).expect("last_run");
    assert!(last_run.contains("current_scheduler_tick_path"));
    assert!(last_run.contains("current_supervisor_cycle_path"));
    let events =
        fs::read_to_string(session_dir.join("events/stream.jsonl")).expect("events stream");
    assert!(events.contains("scheduler.tick_started"));
    assert!(events.contains("scheduler.tick_decision_recorded"));
    assert!(events.contains("scheduler.tick_drove_pending"));
    assert!(events.contains("scheduler.tick_completed"));
    assert!(events.contains("supervisor.cycle_started"));
    assert!(events.contains("supervisor.cycle_completed"));
}

#[path = "web_debug_tests_runtime_followups.rs"]
mod web_debug_tests_runtime_followups;
#[path = "web_debug_tests_runtime_parallel.rs"]
mod web_debug_tests_runtime_parallel;
#[path = "web_debug_tests_runtime_routing.rs"]
mod web_debug_tests_runtime_routing;
