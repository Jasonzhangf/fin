use crate::supervisor_heartbeat::run_supervisor_heartbeat;
use fin_config::RuntimeRetentionConfig;
use fin_debug_server::DebugBinding;
use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

static TEMP_SEQ: AtomicU64 = AtomicU64::new(1);

fn temp_runtime_home() -> PathBuf {
    std::env::temp_dir().join(format!(
        "fin-supervisor-heartbeat-{}-{}",
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
        session_id: Some("session-heartbeat".into()),
        task_id: Some("task-heartbeat".into()),
        session_messages_path: Some(
            "sessions/2026/04/session-heartbeat/conversation/messages.json".into(),
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
fn supervisor_heartbeat_records_observation_without_due_cycle() {
    let home = temp_runtime_home();
    let session_dir = home.join("sessions/2026/04/session-heartbeat");
    fs::create_dir_all(session_dir.join("conversation")).expect("conversation");
    fs::create_dir_all(session_dir.join("control/supervisor")).expect("supervisor");
    write_file(&session_dir.join("conversation/messages.json"), b"[]");
    write_file(
        &session_dir.join("control/supervisor/latest.json"),
        br#"{
  "cycle_id":"supervisor-1",
  "created_at":"2099-04-19T23:40:00+08:00",
  "completed_at":"2099-04-19T23:40:00+08:00",
  "session_id":"session-heartbeat",
  "task_id":"task-heartbeat",
  "source":"manual_tick",
  "status":"completed",
  "tick_id":"tick-1",
  "tick_count":1,
  "drove_count":0,
  "pending_input_count_before":0,
  "pending_input_count_after":0,
  "final_tick_status":"idle_observed",
  "final_action_kind":"stay_idle",
  "blocked_by":"no_work_or_blocked",
  "blocked_kind":"idle_no_work",
  "next_wake_hint":"new_input_or_reminder",
  "next_check_at":null,
  "heartbeat_interval_ms":5000,
  "lease_ttl_ms":15000,
  "result_summary":"idle"
}"#,
    );
    write_file(&home.join("runtime/current/last_run.json"), b"{}");

    let outcome = run_supervisor_heartbeat(
        &home,
        &binding(&home),
        "web_debug_request",
        5_000,
        &RuntimeRetentionConfig::default(),
        8,
        |_binding, _message, _source, _attachments, _merge_segment| {
            panic!("no due cycle expected");
        },
    )
    .expect("heartbeat");

    let heartbeat = outcome.heartbeat.expect("heartbeat");
    assert_eq!(heartbeat.status, "observed");
    assert!(!heartbeat.due_for_tick);
    assert!(!heartbeat.stale_lease);
    assert_eq!(heartbeat.blocked_kind.as_deref(), Some("idle_no_work"));
}

#[test]
fn supervisor_heartbeat_triggers_due_cycle_when_next_check_elapsed() {
    let home = temp_runtime_home();
    let session_dir = home.join("sessions/2026/04/session-heartbeat");
    fs::create_dir_all(session_dir.join("conversation")).expect("conversation");
    fs::create_dir_all(session_dir.join("control")).expect("control");
    fs::create_dir_all(session_dir.join("queue")).expect("queue");
    fs::create_dir_all(session_dir.join("tasks/routing")).expect("routing");
    fs::create_dir_all(session_dir.join("control/supervisor")).expect("supervisor");
    write_file(&session_dir.join("conversation/messages.json"), b"[]");
    write_file(
        &session_dir.join("control/execution_state.json"),
        br#"{
  "state_id":"exec-state-running",
  "session_id":"session-heartbeat",
  "task_id":"task-heartbeat",
  "status":"running",
  "active_turn_id":"turn-1",
  "active_step_id":"step-1",
  "resume_from_step_id":"step-1",
  "pending_input_count":0,
  "accepts_user_input":false,
  "reason":"closure still running",
  "updated_at":"2026-04-19T23:41:00+08:00"
}"#,
    );
    write_file(&session_dir.join("queue/pending_inputs.json"), b"[]");
    write_file(
        &session_dir.join("tasks/routing/latest_action.json"),
        br#"{"action_id":"routing-action-1","decision_id":"routing-1","operation_id":"op-1","trace_id":"trace-1","session_id":"session-heartbeat","task_id":"task-heartbeat","created_at":"2026-04-19T23:41:00+08:00","action_kind":"continue_current_task","source_disposition":"continue_current_task","apply_immediately":true,"prompt_user":false,"prompt_text":null,"suggested_task_id":"task-heartbeat","suggested_topic_thread_id":null,"confidence":90,"reason":"same task"}"#,
    );
    write_file(
        &session_dir.join("control/supervisor/latest.json"),
        br#"{
  "cycle_id":"supervisor-1",
  "created_at":"2020-04-19T23:40:00+08:00",
  "completed_at":"2020-04-19T23:40:00+08:00",
  "session_id":"session-heartbeat",
  "task_id":"task-heartbeat",
  "source":"heartbeat_probe",
  "status":"completed",
  "tick_id":"tick-1",
  "tick_count":1,
  "drove_count":0,
  "pending_input_count_before":0,
  "pending_input_count_after":0,
  "final_tick_status":"blocked",
  "final_action_kind":"wait_running",
  "blocked_by":"running",
  "blocked_kind":"wait_running",
  "next_wake_hint":"supervisor_heartbeat",
  "next_check_at":"2020-04-19T23:40:05+08:00",
  "heartbeat_interval_ms":5000,
  "lease_ttl_ms":15000,
  "result_summary":"wait running"
}"#,
    );
    write_file(&home.join("runtime/current/last_run.json"), b"{}");

    let outcome = run_supervisor_heartbeat(
        &home,
        &binding(&home),
        "web_debug_request",
        5_000,
        &RuntimeRetentionConfig::default(),
        8,
        |_binding, _message, _source, _attachments, _merge_segment| {
            panic!("running state should not dispatch user work");
        },
    )
    .expect("heartbeat");

    let heartbeat = outcome.heartbeat.expect("heartbeat");
    assert!(heartbeat.due_for_tick);
    assert!(heartbeat.stale_lease);
    assert_eq!(heartbeat.status, "stale_detected");
    assert!(heartbeat.triggered_cycle_id.is_some());

    let latest_cycle =
        fs::read_to_string(session_dir.join("control/supervisor/latest.json")).expect("cycle");
    assert!(latest_cycle.contains("\"source\": \"supervisor_heartbeat_due\""));
    let latest_heartbeat =
        fs::read_to_string(session_dir.join("control/supervisor/latest_heartbeat.json"))
            .expect("heartbeat");
    assert!(latest_heartbeat.contains("\"due_for_tick\": true"));
    assert!(latest_heartbeat.contains("\"stale_lease\": true"));
    let events =
        fs::read_to_string(session_dir.join("events/stream.jsonl")).expect("events stream");
    assert!(events.contains("supervisor.heartbeat_recorded"));
    assert!(events.contains("supervisor.stale_cycle_detected"));
}
