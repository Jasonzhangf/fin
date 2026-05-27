use super::*;
use crate::config::map_system_config;
use crate::fs_utils::write_file;
use crate::runtime_home::ensure_runtime_home_layout;
use crate::runtime_home::read_last_run_value;
use crate::web_debug::CliDebugActionHandler;
use crate::web_debug_tests_support::{sample_user_toml, static_provider, temp_runtime_home};
use fin_debug_server::ChatSendRequest;
use std::fs;

#[test]
fn web_debug_turn_uses_session_dir_truth_without_last_run_projection_paths() {
    let home = temp_runtime_home();
    ensure_runtime_home_layout(&home).expect("runtime home should init");
    let system = map_system_config(&sample_user_toml()).expect("system config");
    let handler =
        CliDebugActionHandler::new(sample_user_toml(), system).expect("handler should build");
    let session_dir = home.join("sessions/2026/05/session-ledger-turn");
    fs::create_dir_all(session_dir.join("conversation")).expect("conversation dir");
    fs::create_dir_all(session_dir.join("digests")).expect("digests dir");
    fs::create_dir_all(session_dir.join("context")).expect("context dir");
    fs::create_dir_all(session_dir.join("reasoning")).expect("reasoning dir");
    fs::create_dir_all(session_dir.join("tools")).expect("tools dir");
    write_file(
        &session_dir.join("conversation/messages.json"),
        br#"[{"message_id":"user-1","role":"user","content":"existing continuity","created_at":"2026-04-18T08:10:00+08:00","session_id":"session-ledger-turn","task_id":"task-ledger-turn","operation_id":"op-old"}]"#,
    )
    .expect("messages");
    write_file(
        &session_dir.join("digests/recent_digests.json"),
        br#"[{"digest_id":"digest-old","closure_id":"closure-old","session_id":"session-ledger-turn","task_id":"task-ledger-turn","summary":"existing digest","continuity_tail":[],"note_refs":[],"artifact_candidates":[],"created_at":"2026-04-18T08:10:01+08:00"}]"#,
    )
    .expect("digests");
    write_file(&session_dir.join("context/recent_contexts.json"), b"[]").expect("contexts");
    write_file(
        &home.join("runtime/current/last_run.json"),
        br#"{
  "session_id":"session-ledger-turn",
  "task_id":"task-ledger-turn"
}"#,
    )
    .expect("last_run");

    let response = handler
        .send_message_internal_with_provider(
            &home,
            ChatSendRequest {
                message: "continue the same task".into(),
                input_kind: None,
                attachments: Vec::new(),
            },
            &static_provider(&handler.system),
        )
        .expect("turn should execute from session dir truth");
    assert_eq!(response.response_kind, "assistant_message");
    assert!(response.answer.contains("continue the same task"));

    let current_context =
        fs::read_to_string(home.join("runtime/current/current_context.json")).expect("context");
    assert!(current_context.contains("existing continuity"));
    assert!(current_context.contains("existing digest"));
}

#[test]
fn slash_new_creates_and_binds_new_session() {
    let home = temp_runtime_home();
    ensure_runtime_home_layout(&home).expect("runtime home should init");
    let system = map_system_config(&sample_user_toml()).expect("system config");
    let handler =
        CliDebugActionHandler::new(sample_user_toml(), system).expect("handler should build");

    let response = handler
        .send_message_internal(
            &home,
            ChatSendRequest {
                message: "/new".into(),
                input_kind: None,
                attachments: Vec::new(),
            },
        )
        .expect("new command should work");
    assert_eq!(response.response_kind, "system_notice");
    let session_id = response.binding.session_id.expect("session");
    let task_id = response.binding.task_id.expect("task");
    assert!(session_id.starts_with("session-"));
    assert!(task_id.starts_with("task-"));
    let last_run = read_last_run_value(&home).expect("last run");
    assert_eq!(
        last_run
            .get("session_id")
            .and_then(serde_json::Value::as_str),
        Some(session_id.as_str())
    );
}

#[test]
fn paused_session_runs_parallel_user_message_and_restores_paused_state() {
    let home = temp_runtime_home();
    ensure_runtime_home_layout(&home).expect("runtime home should init");
    let system = map_system_config(&sample_user_toml()).expect("system config");
    let handler =
        CliDebugActionHandler::new(sample_user_toml(), system).expect("handler should build");
    let session_dir = home.join("sessions/2026/05/session-paused");
    fs::create_dir_all(session_dir.join("conversation")).expect("conversation dir");
    fs::create_dir_all(session_dir.join("control")).expect("control dir");
    fs::create_dir_all(session_dir.join("queue")).expect("queue dir");
    write_file(&session_dir.join("conversation/messages.json"), b"[]").expect("messages");
    write_file(
        &session_dir.join("control/execution_state.json"),
        br#"{
  "state_id":"exec-state-paused",
  "session_id":"session-paused",
  "task_id":"task-paused",
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
    write_file(&session_dir.join("queue/pending_inputs.json"), b"[]").expect("pending");
    write_file(
        &home.join("runtime/current/last_run.json"),
        br#"{
  "session_id":"session-paused",
  "task_id":"task-paused",
  "session_messages_path":"sessions/2026/05/session-paused/conversation/messages.json"
}"#,
    )
    .expect("last_run");

    let response = handler
        .send_message_internal_with_provider(
            &home,
            ChatSendRequest {
                message: "new request while paused".into(),
                input_kind: None,
                attachments: Vec::new(),
            },
            &static_provider(&handler.system),
        )
        .expect("parallel paused input should execute");
    assert_eq!(response.response_kind, "assistant_message");
    assert!(response.answer.contains("new request while paused"));
    let pending = fs::read_to_string(session_dir.join("queue/pending_inputs.json"))
        .expect("pending should exist");
    assert_eq!(pending.trim(), "[]");
    let state_after =
        fs::read_to_string(session_dir.join("control/execution_state.json")).expect("state after");
    assert!(state_after.contains("\"status\": \"paused\""));
}

#[test]
fn paused_session_channel_parallel_input_persists_attachments_into_context() {
    let home = temp_runtime_home();
    ensure_runtime_home_layout(&home).expect("runtime home should init");
    let system = map_system_config(&sample_user_toml()).expect("system config");
    let handler =
        CliDebugActionHandler::new(sample_user_toml(), system).expect("handler should build");
    let session_dir = home.join("sessions/2026/05/session-paused-attachments");
    fs::create_dir_all(session_dir.join("conversation")).expect("conversation dir");
    fs::create_dir_all(session_dir.join("control")).expect("control dir");
    fs::create_dir_all(session_dir.join("queue")).expect("queue dir");
    write_file(&session_dir.join("conversation/messages.json"), b"[]").expect("messages");
    write_file(
        &session_dir.join("control/execution_state.json"),
        br#"{
  "state_id":"exec-state-paused",
  "session_id":"session-paused-attachments",
  "task_id":"task-paused-attachments",
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
    write_file(&session_dir.join("queue/pending_inputs.json"), b"[]").expect("pending");
    write_file(
        &home.join("runtime/current/last_run.json"),
        br#"{
  "session_id":"session-paused-attachments",
  "task_id":"task-paused-attachments",
  "session_messages_path":"sessions/2026/05/session-paused-attachments/conversation/messages.json"
}"#,
    )
    .expect("last_run");

    let response = handler
        .send_message_internal_with_provider(
            &home,
            ChatSendRequest {
                message: "describe image".into(),
                input_kind: Some("channel_ingress".into()),
                attachments: vec![fin_contracts::InputAttachmentSummary {
                    name: Some("queued-proof.png".into()),
                    kind: "image/png".into(),
                    ..Default::default()
                }],
            },
            &static_provider(&handler.system),
        )
        .expect("parallel channel input should execute");
    assert_eq!(response.response_kind, "assistant_message");
    let pending = fs::read_to_string(session_dir.join("queue/pending_inputs.json"))
        .expect("pending should exist");
    assert_eq!(pending.trim(), "[]");
    let current_context =
        fs::read_to_string(home.join("runtime/current/current_context.json")).expect("context");
    assert!(current_context.contains("\"source\": \"channel.parallel_user\""));
    assert!(current_context.contains("queued-proof.png"));
}

#[path = "web_debug_tests_runtime.rs"]
mod web_debug_tests_runtime;
#[path = "web_debug_tests_runtime_assignment_resume.rs"]
mod web_debug_tests_runtime_assignment_resume;
#[path = "web_debug_tests_runtime_closed_loop.rs"]
mod web_debug_tests_runtime_closed_loop;
#[path = "web_debug_tests_runtime_owner_loop.rs"]
mod web_debug_tests_runtime_owner_loop;
#[path = "web_debug_tests_runtime_planning_kickoff.rs"]
mod web_debug_tests_runtime_planning_kickoff;
#[path = "web_debug_tests_status.rs"]
mod web_debug_tests_status;
