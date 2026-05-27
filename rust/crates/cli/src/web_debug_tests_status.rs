use crate::fs_utils::write_file;
use crate::web_debug_tests_support::{build_handler, temp_runtime_home};
use fin_contracts::{ControlFeedback, ExecutionNote, ProgressBlock};
use fin_debug_server::ChatSendRequest;
use std::fs;

#[test]
fn status_probe_returns_latest_framework_state_without_new_closure() {
    let home = temp_runtime_home();
    let handler = build_handler(&home);
    let session_dir = home.join("sessions/2026/05/session-web-debug");
    fs::create_dir_all(session_dir.join("conversation")).expect("conversation dir");
    fs::create_dir_all(session_dir.join("progress")).expect("progress dir");
    fs::create_dir_all(session_dir.join("notes")).expect("notes dir");
    fs::create_dir_all(session_dir.join("control")).expect("control dir");
    fs::create_dir_all(session_dir.join("digests")).expect("digests dir");
    write_file(
            &session_dir.join("conversation/messages.json"),
            br#"[{"message_id":"user-1","role":"user","content":"build it","created_at":"2026-04-18T08:10:00+08:00","session_id":"session-web-debug","task_id":"task-web-debug"}]"#,
        )
        .expect("messages should write");
    write_file(
            &session_dir.join("digests/recent_digests.json"),
            br#"[{"digest_id":"digest-existing","closure_id":"closure-existing","session_id":"session-web-debug","task_id":"task-web-debug","summary":"existing digest","continuity_tail":[],"note_refs":[],"artifact_candidates":[],"created_at":"2026-04-18T08:10:01+08:00"}]"#,
        )
        .expect("digests should write");
    write_file(
        &session_dir.join("progress/latest.json"),
        serde_json::to_vec_pretty(&ProgressBlock {
            progress_id: "progress-1".into(),
            refs: fin_contracts::EntityRefs {
                session_id: Some("session-web-debug".into()),
                task_id: Some("task-web-debug".into()),
                ..fin_contracts::EntityRefs::default()
            },
            phase: "running".into(),
            blocker: None,
            next_step: Some("finish current closure".into()),
            health_hint: Some("healthy".into()),
            tool_snapshots: vec![],
        })
        .expect("progress json")
        .as_slice(),
    )
    .expect("progress should write");
    write_file(
        &session_dir.join("notes/latest.json"),
        serde_json::to_vec_pretty(&ExecutionNote {
            note_id: "note-1".into(),
            refs: fin_contracts::EntityRefs {
                session_id: Some("session-web-debug".into()),
                task_id: Some("task-web-debug".into()),
                ..fin_contracts::EntityRefs::default()
            },
            summary: "currently applying runtime changes".into(),
            decision: None,
            lesson: None,
            blocker: None,
            next_step: Some("wait for verification".into()),
            control_feedback: None,
            created_at: "2026-04-18T08:10:02+08:00".into(),
        })
        .expect("note json")
        .as_slice(),
    )
    .expect("note should write");
    write_file(
        &session_dir.join("control/latest.json"),
        serde_json::to_vec_pretty(&ControlFeedback {
            origin: "runtime_heuristic".into(),
            is_continuation: true,
            continuity_confidence: 93,
            topic_shift_confidence: 7,
            simple_query_confidence: 10,
            reason: "current task still active".into(),
            ..ControlFeedback::default()
        })
        .expect("control json")
        .as_slice(),
    )
    .expect("control should write");
    write_file(
        &session_dir.join("control/execution_state.json"),
        br#"{
  "state_id":"exec-state-running",
  "session_id":"session-web-debug",
  "task_id":"task-web-debug",
  "status":"running",
  "active_turn_id":"turn-op-live",
  "active_step_id":"step-op-live-03-model_parse",
  "resume_from_step_id":"step-op-live-03-model_parse",
  "pending_input_count":1,
  "accepts_user_input":false,
  "reason":"active closure running",
  "updated_at":"2026-04-18T08:10:02+08:00"
}"#,
    )
    .expect("execution state should write");
    fs::create_dir_all(session_dir.join("tasks/routing")).expect("routing dir");
    write_file(
            &session_dir.join("tasks/routing/latest_action.json"),
            br#"{"action_id":"routing-action-op-live","decision_id":"routing-op-live","operation_id":"op-live","trace_id":"trace-live","session_id":"session-web-debug","task_id":"task-web-debug","created_at":"2026-04-18T08:10:02+08:00","action_kind":"continue_current_task","source_disposition":"continue_current_task","apply_immediately":true,"prompt_user":false,"prompt_text":null,"suggested_task_id":"task-web-debug","suggested_topic_thread_id":null,"confidence":93,"reason":"current task still active"}"#,
        )
        .expect("routing action should write");
    fs::create_dir_all(session_dir.join("queue")).expect("queue dir");
    write_file(
            &session_dir.join("queue/pending_inputs.json"),
            br#"[{"pending_input_id":"pending-1","session_id":"session-web-debug","task_id":"task-web-debug","input_kind":"chat","message":"queued question","status":"pending","enqueue_reason":"paused","enqueued_at":"2026-04-18T08:11:00+08:00"}]"#,
        )
        .expect("pending queue should write");
    write_file(
        &home.join("runtime/current/last_run.json"),
        br#"{
  "session_id":"session-web-debug",
  "task_id":"task-web-debug",
  "digest_id":"digest-existing",
  "session_messages_path":"sessions/2026/05/session-web-debug/conversation/messages.json",
  "session_control_feedback_path":"sessions/2026/05/session-web-debug/control/latest.json",
  "current_execution_state_path":"runtime/current/current_execution_state.json",
  "current_pending_inputs_path":"runtime/current/current_pending_inputs.json"
}"#,
    )
    .expect("last_run should write");
    write_file(
        &home.join("runtime/current/current_agent_registry.json"),
        br#"{"agents":[{"agent_id":"mac-mini.system","agent_name":"system","device_name":"mac-mini","worker_id":"worker-system","role_id":"system","source":"cli"}]}"#,
    )
    .expect("agent registry should write");

    let before_messages =
        fs::read_to_string(session_dir.join("conversation/messages.json")).expect("before");
    let before_digests =
        fs::read_to_string(session_dir.join("digests/recent_digests.json")).expect("before");

    let response = handler
        .send_message_internal(
            &home,
            ChatSendRequest {
                message: "/status current?".into(),
                input_kind: Some("status_probe".into()),
                attachments: Vec::new(),
            },
        )
        .expect("status probe should work");

    assert_eq!(response.response_kind, "status_probe");
    assert_eq!(response.freshness.as_deref(), Some("live"));
    assert_eq!(response.digest_id, "digest-existing");
    assert_eq!(response.events_count, 0);
    assert!(response.answer.contains("status probe (live)"));
    assert!(response.answer.contains("agents=agents=4 ["));
    assert!(response.answer.contains("system-worker-01:idle"));
    assert!(response.answer.contains("project_supervision="));
    assert!(response.answer.contains("project_runtime_resume="));
    assert!(
        response
            .answer
            .contains("project_recovery=project recovery unavailable")
    );
    assert!(response.answer.contains("phase=running"));
    assert!(
        response
            .answer
            .contains("active_step=step-op-live-03-model_parse")
    );
    assert!(response.answer.contains("pending_inputs=1"));
    assert!(
        response
            .answer
            .contains("currently applying runtime changes")
    );
    assert!(response.answer.contains("routing_action="));
    assert_eq!(
        response
            .routing_action
            .as_ref()
            .map(|value| value.action_kind.as_str()),
        Some("continue_current_task")
    );
    assert_eq!(
        response
            .control_feedback
            .as_ref()
            .map(|value| value.continuity_confidence),
        Some(93)
    );
    assert_eq!(
        fs::read_to_string(session_dir.join("conversation/messages.json")).expect("after"),
        before_messages
    );
    assert_eq!(
        fs::read_to_string(session_dir.join("digests/recent_digests.json")).expect("after"),
        before_digests
    );
}

#[test]
fn status_probe_uses_session_dir_truth_without_messages_projection_anchor() {
    let home = temp_runtime_home();
    let handler = build_handler(&home);
    let session_dir = home.join("sessions/2026/05/session-status-ledger");
    fs::create_dir_all(session_dir.join("progress")).expect("progress dir");
    fs::create_dir_all(session_dir.join("notes")).expect("notes dir");
    fs::create_dir_all(session_dir.join("control")).expect("control dir");
    fs::create_dir_all(session_dir.join("tasks/routing")).expect("routing dir");
    fs::create_dir_all(session_dir.join("queue")).expect("queue dir");
    write_file(
        &session_dir.join("progress/latest.json"),
        serde_json::to_vec_pretty(&ProgressBlock {
            progress_id: "progress-2".into(),
            refs: fin_contracts::EntityRefs {
                session_id: Some("session-status-ledger".into()),
                task_id: Some("task-status-ledger".into()),
                ..fin_contracts::EntityRefs::default()
            },
            phase: "running".into(),
            blocker: Some("none".into()),
            next_step: Some("continue supervised task".into()),
            health_hint: Some("healthy".into()),
            tool_snapshots: vec![],
        })
        .expect("progress json")
        .as_slice(),
    )
    .expect("progress should write");
    write_file(
        &session_dir.join("notes/latest.json"),
        serde_json::to_vec_pretty(&ExecutionNote {
            note_id: "note-2".into(),
            refs: fin_contracts::EntityRefs {
                session_id: Some("session-status-ledger".into()),
                task_id: Some("task-status-ledger".into()),
                ..fin_contracts::EntityRefs::default()
            },
            summary: "ledger truth note".into(),
            decision: None,
            lesson: None,
            blocker: None,
            next_step: Some("wait for project completion".into()),
            control_feedback: None,
            created_at: "2026-04-18T09:10:02+08:00".into(),
        })
        .expect("note json")
        .as_slice(),
    )
    .expect("note should write");
    write_file(
        &session_dir.join("control/latest.json"),
        serde_json::to_vec_pretty(&ControlFeedback {
            origin: "runtime_heuristic".into(),
            is_continuation: true,
            continuity_confidence: 88,
            topic_shift_confidence: 12,
            simple_query_confidence: 8,
            reason: "session dir truth still active".into(),
            ..ControlFeedback::default()
        })
        .expect("control json")
        .as_slice(),
    )
    .expect("control should write");
    write_file(
        &session_dir.join("control/execution_state.json"),
        br#"{
  "state_id":"exec-state-running-2",
  "session_id":"session-status-ledger",
  "task_id":"task-status-ledger",
  "status":"running",
  "active_turn_id":"turn-op-status",
  "active_step_id":"step-op-status-02",
  "resume_from_step_id":"step-op-status-02",
  "pending_input_count":0,
  "accepts_user_input":false,
  "reason":"running from session dir truth",
  "updated_at":"2026-04-18T09:10:02+08:00"
}"#,
    )
    .expect("execution state should write");
    write_file(
        &session_dir.join("tasks/routing/latest_action.json"),
        br#"{"action_id":"routing-action-op-status","decision_id":"routing-op-status","operation_id":"op-status","trace_id":"trace-status","session_id":"session-status-ledger","task_id":"task-status-ledger","created_at":"2026-04-18T09:10:02+08:00","action_kind":"continue_current_task","source_disposition":"continue_current_task","apply_immediately":true,"prompt_user":false,"prompt_text":null,"suggested_task_id":"task-status-ledger","suggested_topic_thread_id":null,"confidence":88,"reason":"status probe via session dir"}"#,
    )
    .expect("routing should write");
    write_file(
        &home.join("runtime/current/last_run.json"),
        br#"{
  "session_id":"session-status-ledger",
  "task_id":"task-status-ledger",
  "digest_id":"digest-status-ledger"
}"#,
    )
    .expect("last_run should write");
    write_file(
        &home.join("runtime/current/current_agent_registry.json"),
        br#"{"agents":[{"agent_id":"mac-mini.system","agent_name":"system","device_name":"mac-mini","worker_id":"worker-system","role_id":"system","source":"cli"}]}"#,
    )
    .expect("agent registry should write");

    let response = handler
        .send_message_internal(
            &home,
            ChatSendRequest {
                message: "/status check".into(),
                input_kind: Some("status_probe".into()),
                attachments: Vec::new(),
            },
        )
        .expect("status probe should work");

    assert_eq!(response.response_kind, "status_probe");
    assert_eq!(response.freshness.as_deref(), Some("live"));
    assert!(response.answer.contains("session=session-status-ledger"));
    assert!(response.answer.contains("phase=running"));
    assert!(response.answer.contains("ledger truth note"));
    assert!(
        response
            .answer
            .contains("routing_action=continue_current_task")
    );
}
