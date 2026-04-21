use super::*;

#[test]
fn ordinary_user_input_runs_as_parallel_inference_while_waiting_external() {
    let home = temp_runtime_home();
    ensure_runtime_home_layout(&home).expect("runtime home should init");
    let system = map_system_config(&sample_user_toml()).expect("system config");
    let handler =
        CliDebugActionHandler::new(sample_user_toml(), system).expect("handler should build");
    let session_dir = home.join("sessions/2026/04/session-parallel-waiting");
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
  "session_id":"session-parallel-waiting",
  "task_id":"task-parallel-waiting",
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
    let checkpoint = "{
  \"checkpoint_id\":\"checkpoint-op-waiting-r02\",
  \"session_id\":\"session-parallel-waiting\",
  \"task_id\":\"task-parallel-waiting\",
  \"trace_id\":\"trace-op-waiting\",
  \"source_operation_id\":\"op-waiting\",
  \"source_turn_id\":\"turn-op-waiting\",
  \"source_step_id\":\"step-op-waiting-04-tool_dispatch\",
  \"checkpoint_kind\":\"wait_reminder_resume\",
  \"status\":\"open\",
  \"source_round_index\":1,
  \"next_round_index\":2,
  \"resume_input\":\"Continue waiting flow after reminder.\",
  \"summary\":\"resume round 2 after wait.remind from round 1\",
  \"created_at\":\"2026-04-20T10:00:00+08:00\",
  \"consumed_at\":null,
  \"consumed_by_operation_id\":null
}";
    write_file(
        &session_dir.join("control/execution_checkpoint.json"),
        checkpoint.as_bytes(),
    )
    .expect("checkpoint");
    write_file(
        &session_dir.join("control/recent_execution_checkpoints.json"),
        format!("[{checkpoint}]").as_bytes(),
    )
    .expect("recent checkpoints");
    write_file(
        &home.join("runtime/current/last_run.json"),
        br#"{
  "session_id":"session-parallel-waiting",
  "task_id":"task-parallel-waiting",
  "session_messages_path":"sessions/2026/04/session-parallel-waiting/conversation/messages.json",
  "session_recent_contexts_path":"sessions/2026/04/session-parallel-waiting/context/recent_contexts.json",
  "session_recent_digests_path":"sessions/2026/04/session-parallel-waiting/digests/recent_digests.json",
  "session_recent_reasoning_path":"sessions/2026/04/session-parallel-waiting/reasoning/recent_reasoning_views.json",
  "session_recent_tool_records_path":"sessions/2026/04/session-parallel-waiting/tools/recent_tool_records.json"
}"#,
    )
    .expect("last_run");

    let response = handler
        .send_message_internal_with_provider(
            &home,
            ChatSendRequest {
                message: "while waiting, answer this in parallel".into(),
                input_kind: None,
                attachments: Vec::new(),
            },
            &static_provider(&handler.system),
        )
        .expect("parallel input should execute immediately");
    assert_eq!(response.response_kind, "assistant_message");
    assert!(
        response
            .answer
            .contains("while waiting, answer this in parallel")
    );

    let state_after =
        fs::read_to_string(session_dir.join("control/execution_state.json")).expect("state after");
    assert!(state_after.contains("\"status\": \"waiting_external\""));
    assert!(state_after.contains("\"resume_checkpoint_ready\": true"));
    let checkpoint_after =
        fs::read_to_string(session_dir.join("control/execution_checkpoint.json"))
            .expect("checkpoint after");
    assert!(
        checkpoint_after.contains("\"status\":\"open\"")
            || checkpoint_after.contains("\"status\": \"open\"")
    );
    assert!(
        checkpoint_after.contains("\"consumed_at\":null")
            || checkpoint_after.contains("\"consumed_at\": null")
    );
    let pending_after =
        fs::read_to_string(session_dir.join("queue/pending_inputs.json")).expect("pending after");
    assert_eq!(pending_after.trim(), "[]");
    let messages =
        fs::read_to_string(session_dir.join("conversation/messages.json")).expect("messages");
    assert!(messages.contains("while waiting, answer this in parallel"));
    let latest_tick = fs::read_to_string(session_dir.join("control/scheduler/latest_tick.json"))
        .expect("latest tick");
    assert!(latest_tick.contains("\"source\": \"parallel_user_input\""));
    assert!(latest_tick.contains("\"drove_count\": 1"));
    assert!(latest_tick.contains("\"final_action_kind\": \"wait_external\""));
    let latest_decision = fs::read_to_string(session_dir.join("control/scheduler/latest.json"))
        .expect("latest decision");
    assert!(latest_decision.contains("\"action_kind\": \"wait_external\""));
    let recent_decisions =
        fs::read_to_string(session_dir.join("control/scheduler/recent_decisions.json"))
            .expect("recent decisions");
    assert!(
        recent_decisions.contains("\"action_kind\":\"run_next_parallel\"")
            || recent_decisions.contains("\"action_kind\": \"run_next_parallel\"")
    );
}
