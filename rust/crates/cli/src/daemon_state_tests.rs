use crate::daemon_state::refresh_attached_daemon_state;
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
        "fin-daemon-state-{}-{}",
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
        session_id: Some("session-daemon".into()),
        task_id: Some("task-daemon".into()),
        session_messages_path: Some(
            "sessions/2026/04/session-daemon/conversation/messages.json".into(),
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
fn refresh_attached_daemon_state_records_health_and_recovery() {
    let home = temp_runtime_home();
    let session_dir = home.join("sessions/2026/04/session-daemon");
    fs::create_dir_all(session_dir.join("conversation")).expect("conversation");
    fs::create_dir_all(session_dir.join("control/supervisor")).expect("supervisor");
    write_file(&session_dir.join("conversation/messages.json"), b"[]");
    write_file(
        &session_dir.join("control/supervisor/latest.json"),
        br#"{
  "cycle_id":"supervisor-1",
  "created_at":"2020-04-19T23:50:00+08:00",
  "completed_at":"2020-04-19T23:50:00+08:00",
  "session_id":"session-daemon",
  "task_id":"task-daemon",
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
  "next_check_at":"2020-04-19T23:50:05+08:00",
  "heartbeat_interval_ms":5000,
  "lease_ttl_ms":15000,
  "result_summary":"wait running"
}"#,
    );
    write_file(
        &session_dir.join("control/supervisor/latest_heartbeat.json"),
        br#"{
  "heartbeat_id":"heartbeat-1",
  "created_at":"2020-04-19T23:50:30+08:00",
  "session_id":"session-daemon",
  "task_id":"task-daemon",
  "source":"web_debug_request",
  "status":"stale_detected",
  "observed_cycle_id":"supervisor-1",
  "triggered_cycle_id":null,
  "due_for_tick":true,
  "stale_lease":true,
  "blocked_kind":"wait_running",
  "next_check_at":"2020-04-19T23:50:05+08:00",
  "lease_deadline_at":"2020-04-19T23:50:15+08:00",
  "result_summary":"stale"
}"#,
    );
    write_file(&home.join("runtime/current/last_run.json"), b"{}");

    let outcome = refresh_attached_daemon_state(
        &home,
        &binding(&home),
        "web_debug_request",
        &RuntimeRetentionConfig::default(),
        8,
    )
    .expect("daemon state");

    let state = outcome.state.expect("state");
    assert_eq!(state.service_kind, "web_debug_attached");
    assert_eq!(state.lifecycle_state, "attached_active");
    assert_eq!(state.health_state.as_deref(), Some("stale"));
    assert!(state.recovery_needed);
    assert_eq!(
        state.recovery_action_kind.as_deref(),
        Some("recover_stale_cycle")
    );

    let recovery = outcome.recovery_action.expect("recovery");
    assert_eq!(recovery.action_kind, "recover_stale_cycle");
    assert!(recovery.apply_immediately);

    let latest_state =
        fs::read_to_string(session_dir.join("control/daemon/latest_state.json")).expect("state");
    assert!(latest_state.contains("\"service_kind\": \"web_debug_attached\""));
    let latest_recovery =
        fs::read_to_string(session_dir.join("control/daemon/latest_recovery_action.json"))
            .expect("recovery");
    assert!(latest_recovery.contains("\"action_kind\": \"recover_stale_cycle\""));
}
