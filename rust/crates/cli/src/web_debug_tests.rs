use super::*;
use crate::{
    config::map_system_config, fs_utils::write_file, runtime_home::ensure_runtime_home_layout,
};
use fin_contracts::{ControlFeedback, ExecutionNote, ProgressBlock};
use fin_provider::{ProviderDescriptor, StructuredStaticProviderClient};
use std::path::PathBuf;
use std::{
    fs,
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

static TEMP_RUNTIME_COUNTER: AtomicU64 = AtomicU64::new(1);
pub(crate) const REMOVED_LEGACY_HIDDEN_FOLLOWUP_EVENT: &str = "framework.task_kickoff_enqueued";

fn sample_user_toml() -> String {
    r#"
default_provider = "openai"

[providers.openai]
protocol = "open-ai-compatible"
base_url = "https://api.example.com/v1"
model = "gpt-5"
api_key_env = "OPENAI_API_KEY"
"#
    .into()
}

fn temp_runtime_home() -> PathBuf {
    let seq = TEMP_RUNTIME_COUNTER.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!(
        "fin-status-probe-{}-{seq}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time should work")
            .as_nanos()
    ))
}

fn static_provider(system: &SystemConfig) -> StructuredStaticProviderClient {
    StructuredStaticProviderClient::new(ProviderDescriptor::from_resolved(
        system.default_provider_config().expect("default provider"),
    ))
}

#[test]
fn status_probe_returns_latest_framework_state_without_new_closure() {
    let home = temp_runtime_home();
    ensure_runtime_home_layout(&home).expect("runtime home should init");
    let system = map_system_config(&sample_user_toml()).expect("system config");
    let handler =
        CliDebugActionHandler::new(sample_user_toml(), system).expect("handler should build");
    let session_dir = home.join("sessions/2026/04/session-web-debug");
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
            origin: "runtime_observation_only_v1".into(),
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
  "session_messages_path":"sessions/2026/04/session-web-debug/conversation/messages.json",
  "session_control_feedback_path":"sessions/2026/04/session-web-debug/control/latest.json",
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
fn orphaned_running_state_without_active_lease_recovers_before_user_input() {
    let home = temp_runtime_home();
    ensure_runtime_home_layout(&home).expect("runtime home should init");
    let system = map_system_config(&sample_user_toml()).expect("system config");
    let handler =
        CliDebugActionHandler::new(sample_user_toml(), system).expect("handler should build");
    let session_dir = home.join("sessions/2026/04/session-orphaned-running");
    fs::create_dir_all(session_dir.join("conversation")).expect("conversation dir");
    fs::create_dir_all(session_dir.join("control")).expect("control dir");
    fs::create_dir_all(session_dir.join("queue")).expect("queue dir");
    write_file(&session_dir.join("conversation/messages.json"), b"[]").expect("messages");
    write_file(&session_dir.join("queue/pending_inputs.json"), b"[]").expect("pending");
    write_file(
        &session_dir.join("control/execution_state.json"),
        br#"{
  "state_id":"exec-state-op-orphaned",
  "session_id":"session-orphaned-running",
  "task_id":"task-orphaned-running",
  "status":"running",
  "active_turn_id":"turn-op-orphaned",
  "active_step_id":"step-op-orphaned-01-context_build",
  "resume_from_step_id":null,
  "pending_input_count":0,
  "accepts_user_input":false,
  "reason":"active closure running",
  "updated_at":"2026-04-18T08:10:02+08:00"
}"#,
    )
    .expect("state");
    write_file(
        &home.join("runtime/current/last_run.json"),
        br#"{
  "session_id":"session-orphaned-running",
  "task_id":"task-orphaned-running",
  "session_messages_path":"sessions/2026/04/session-orphaned-running/conversation/messages.json"
}"#,
    )
    .expect("last_run");

    let response = handler
        .send_message_internal_with_provider(
            &home,
            ChatSendRequest {
                message: "run after orphaned state".into(),
                input_kind: None,
                attachments: Vec::new(),
            },
            &static_provider(&handler.system),
        )
        .expect("orphaned running should not queue forever");

    assert_eq!(response.response_kind, "assistant_message");
    assert!(response.answer.contains("run after orphaned state"));
    let pending =
        fs::read_to_string(session_dir.join("queue/pending_inputs.json")).expect("pending after");
    assert_eq!(pending.trim(), "[]");
    let state_path = response
        .binding
        .session_messages_path
        .as_deref()
        .and_then(|relative| relative.strip_suffix("conversation/messages.json"))
        .map(|prefix| {
            home.join(prefix.trim_end_matches('/'))
                .join("control/execution_state.json")
        })
        .unwrap_or_else(|| home.join("runtime/current/current_execution_state.json"));
    let state_after = fs::read_to_string(state_path).expect("state after");
    assert!(!state_after.contains("\"status\": \"running\""));
    assert!(!state_after.contains("active closure running"));
}

#[test]
fn paused_session_runs_parallel_user_message_and_restores_paused_state() {
    let home = temp_runtime_home();
    ensure_runtime_home_layout(&home).expect("runtime home should init");
    let system = map_system_config(&sample_user_toml()).expect("system config");
    let handler =
        CliDebugActionHandler::new(sample_user_toml(), system).expect("handler should build");
    let session_dir = home.join("sessions/2026/04/session-paused");
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
  "session_messages_path":"sessions/2026/04/session-paused/conversation/messages.json"
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
    let session_dir = home.join("sessions/2026/04/session-paused-attachments");
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
  "session_messages_path":"sessions/2026/04/session-paused-attachments/conversation/messages.json"
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
#[path = "web_debug_tests_runtime_formalize_passive.rs"]
mod web_debug_tests_runtime_formalize_passive;
#[path = "web_debug_tests_runtime_owner_loop.rs"]
mod web_debug_tests_runtime_owner_loop;
#[path = "web_debug_tests_runtime_system_inspection.rs"]
mod web_debug_tests_runtime_system_inspection;
