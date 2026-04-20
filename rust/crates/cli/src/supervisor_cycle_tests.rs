use crate::supervisor_cycle::run_supervisor_cycle;
use fin_config::RuntimeRetentionConfig;
use fin_debug_server::{ChatSendResponse, DebugBinding};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

static TEMP_SEQ: AtomicU64 = AtomicU64::new(1);

fn temp_runtime_home() -> PathBuf {
    std::env::temp_dir().join(format!(
        "fin-supervisor-cycle-{}-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time should work")
            .as_nanos(),
        TEMP_SEQ.fetch_add(1, Ordering::Relaxed)
    ))
}

fn binding(home: &Path) -> DebugBinding {
    DebugBinding {
        project_id: "fin".into(),
        project_label: "fin".into(),
        runtime_home: home.display().to_string(),
        session_id: Some("session-supervisor".into()),
        task_id: Some("task-supervisor".into()),
        session_messages_path: Some(
            "sessions/2026/04/session-supervisor/conversation/messages.json".into(),
        ),
        recent_contexts_path: None,
        recent_digests_path: None,
    }
}

fn write_file(path: &Path, bytes: &[u8]) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("parent");
    }
    fs::write(path, bytes).expect("write");
}

#[test]
fn run_supervisor_cycle_persists_cycle_and_events() {
    let home = temp_runtime_home();
    let session_dir = home.join("sessions/2026/04/session-supervisor");
    fs::create_dir_all(session_dir.join("conversation")).expect("conversation");
    fs::create_dir_all(session_dir.join("control")).expect("control");
    fs::create_dir_all(session_dir.join("queue")).expect("queue");
    fs::create_dir_all(session_dir.join("tasks/routing")).expect("routing");
    write_file(&session_dir.join("conversation/messages.json"), b"[]");
    write_file(
        &session_dir.join("control/execution_state.json"),
        br#"{
  "state_id":"exec-state-idle",
  "session_id":"session-supervisor",
  "task_id":"task-supervisor",
  "status":"idle",
  "active_turn_id":null,
  "active_step_id":null,
  "resume_from_step_id":null,
  "pending_input_count":1,
  "accepts_user_input":true,
  "reason":"idle",
  "updated_at":"2026-04-19T23:30:00+08:00"
}"#,
    );
    write_file(
        &session_dir.join("queue/pending_inputs.json"),
        br#"[{"pending_input_id":"pending-1","session_id":"session-supervisor","task_id":"task-supervisor","input_kind":"chat","message":"queued work","status":"pending","enqueue_reason":"test","enqueued_at":"2026-04-19T23:30:01+08:00"}]"#,
    );
    write_file(
        &session_dir.join("tasks/routing/latest_action.json"),
        br#"{"action_id":"routing-action-1","decision_id":"routing-1","operation_id":"op-1","trace_id":"trace-1","session_id":"session-supervisor","task_id":"task-supervisor","created_at":"2026-04-19T23:30:00+08:00","action_kind":"continue_current_task","source_disposition":"continue_current_task","apply_immediately":true,"prompt_user":false,"prompt_text":null,"suggested_task_id":"task-supervisor","suggested_topic_thread_id":null,"confidence":90,"reason":"same task"}"#,
    );
    write_file(&home.join("runtime/current/last_run.json"), b"{}");

    let outcome = run_supervisor_cycle(
        &home,
        &binding(&home),
        "manual_tick",
        5_000,
        &RuntimeRetentionConfig::default(),
        8,
        |binding, message, _source, _attachments, _merge_segment| {
            Ok(ChatSendResponse {
                binding,
                answer: format!("done:{message}"),
                digest_id: "digest".into(),
                events_count: 0,
                response_kind: "assistant_message".into(),
                freshness: None,
                control_feedback: None,
                progress: None,
                note: None,
                routing_action: None,
            })
        },
    )
    .expect("supervisor cycle should succeed");

    let cycle = outcome.cycle.expect("cycle");
    assert_eq!(cycle.tick_count, 1);
    assert_eq!(cycle.drove_count, 1);
    assert_eq!(cycle.source, "manual_tick");
    assert_eq!(cycle.final_action_kind.as_deref(), Some("stay_idle"));
    assert_eq!(cycle.blocked_kind.as_deref(), Some("idle_no_work"));
    assert_eq!(
        cycle.next_wake_hint.as_deref(),
        Some("new_input_or_reminder")
    );
    assert_eq!(cycle.next_check_at.as_deref(), None);
    assert_eq!(cycle.heartbeat_interval_ms, Some(5_000));
    assert_eq!(cycle.lease_ttl_ms, Some(15_000));

    let latest_cycle =
        fs::read_to_string(session_dir.join("control/supervisor/latest.json")).expect("latest");
    assert!(latest_cycle.contains("\"source\": \"manual_tick\""));
    assert!(latest_cycle.contains("\"drove_count\": 1"));

    let events =
        fs::read_to_string(session_dir.join("events/stream.jsonl")).expect("events stream");
    assert!(events.contains("supervisor.cycle_started"));
    assert!(events.contains("supervisor.cycle_completed"));

    let last_run =
        fs::read_to_string(home.join("runtime/current/last_run.json")).expect("last_run");
    assert!(last_run.contains("current_supervisor_cycle_path"));
}

#[test]
fn run_supervisor_cycle_derives_wait_running_next_check() {
    let home = temp_runtime_home();
    let session_dir = home.join("sessions/2026/04/session-supervisor");
    fs::create_dir_all(session_dir.join("conversation")).expect("conversation");
    fs::create_dir_all(session_dir.join("control")).expect("control");
    fs::create_dir_all(session_dir.join("queue")).expect("queue");
    fs::create_dir_all(session_dir.join("tasks/routing")).expect("routing");
    write_file(&session_dir.join("conversation/messages.json"), b"[]");
    write_file(
        &session_dir.join("control/execution_state.json"),
        br#"{
  "state_id":"exec-state-running",
  "session_id":"session-supervisor",
  "task_id":"task-supervisor",
  "status":"running",
  "active_turn_id":"turn-1",
  "active_step_id":"step-1",
  "resume_from_step_id":"step-1",
  "pending_input_count":0,
  "accepts_user_input":false,
  "reason":"closure still running",
  "updated_at":"2026-04-19T23:31:00+08:00"
}"#,
    );
    write_file(&session_dir.join("queue/pending_inputs.json"), b"[]");
    write_file(
        &session_dir.join("tasks/routing/latest_action.json"),
        br#"{"action_id":"routing-action-1","decision_id":"routing-1","operation_id":"op-1","trace_id":"trace-1","session_id":"session-supervisor","task_id":"task-supervisor","created_at":"2026-04-19T23:31:00+08:00","action_kind":"continue_current_task","source_disposition":"continue_current_task","apply_immediately":true,"prompt_user":false,"prompt_text":null,"suggested_task_id":"task-supervisor","suggested_topic_thread_id":null,"confidence":90,"reason":"same task"}"#,
    );
    write_file(&home.join("runtime/current/last_run.json"), b"{}");

    let outcome = run_supervisor_cycle(
        &home,
        &binding(&home),
        "heartbeat_probe",
        5_000,
        &RuntimeRetentionConfig::default(),
        8,
        |_binding, _message, _source, _attachments, _merge_segment| {
            panic!("running state should not dispatch queued work");
        },
    )
    .expect("supervisor cycle should succeed");

    let cycle = outcome.cycle.expect("cycle");
    assert_eq!(cycle.blocked_kind.as_deref(), Some("wait_running"));
    assert_eq!(
        cycle.next_wake_hint.as_deref(),
        Some("supervisor_heartbeat")
    );
    assert!(cycle.next_check_at.is_some());
    assert_eq!(cycle.heartbeat_interval_ms, Some(5_000));
    assert_eq!(cycle.lease_ttl_ms, Some(15_000));
}
