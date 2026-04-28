use super::*;
use crate::headless_daemon_reconcile::reconcile_headless_daemon_state;
use std::{thread, time::Duration};

#[test]
fn reconcile_headless_daemon_state_marks_dead_pid_as_restart_required() {
    let home = temp_runtime_home("reconcile-dead-pid");
    ensure_runtime_home_layout(&home).expect("runtime home");
    fs::create_dir_all(home.join("runtime/current")).expect("current dir");
    fs::create_dir_all(home.join("runtime/leases")).expect("lease dir");
    fs::create_dir_all(home.join("runtime/pids")).expect("pid dir");
    write_file(
        &home.join("runtime/current/current_daemon_state.json"),
        br#"{
  "daemon_id":"mbp.headless-daemon",
  "created_at":"2026-04-24T18:27:31+08:00",
  "updated_at":"2026-04-24T18:27:31+08:00",
  "service_kind":"headless_daemon",
  "lifecycle_state":"active",
  "supervision_state":"running_sessions",
  "mode":"detached",
  "pid":999999,
  "health_state":"healthy",
  "recovery_needed":false,
  "recovery_action_kind":"observe_only",
  "active_binding":"session=session-system-entry",
  "status_summary":"daemon source=headless_loop"
}"#,
    )
    .expect("daemon state");
    write_file(
        &home.join("runtime/leases/headless-daemon.json"),
        br#"{
  "daemon_id":"mbp.headless-daemon",
  "pid":999999,
  "heartbeat_interval_ms":5000,
  "lease_ttl_ms":15000,
  "started_at":"2026-04-24T18:20:00+08:00",
  "updated_at":"2026-04-24T18:27:31+08:00",
  "lifecycle_state":"active",
  "active_session_ids":["session-system-entry"],
  "processed_sessions":1,
  "drove_count":1
}"#,
    )
    .expect("lease");
    write_file(&home.join("runtime/pids/headless-daemon.pid"), b"999999").expect("pid");
    write_file(
        &home.join("runtime/assignments/pending.json"),
        br#"[
  {
    "assignment_id":"assign-op-system-entry-0024",
    "peer_id":"local-worker-builder",
    "project_id":"fin",
    "session_id":"session-system-entry",
    "task_id":"task-owner-dispatch",
    "target_worker_id":"worker-builder",
    "requested_role_id":"project",
    "task_summary":"resume worker task",
    "created_at":"2026-04-24T18:25:00+08:00",
    "status":"pending"
  }
]"#,
    )
    .expect("assignments");

    let outcome = reconcile_headless_daemon_state(&home)
        .expect("reconcile")
        .expect("reconcile outcome");
    assert_eq!(outcome.state.lifecycle_state, "stopped");
    assert_eq!(outcome.state.health_state.as_deref(), Some("dead"));
    assert_eq!(outcome.state.supervision_state, "restart_required");
    assert!(outcome.state.recovery_needed);
    assert_eq!(
        outcome.state.recovery_action_kind.as_deref(),
        Some("restart_headless_daemon")
    );
    assert!(
        !home.join("runtime/pids/headless-daemon.pid").exists(),
        "stale pid file should be cleared"
    );
    let recovery = outcome.recovery.expect("recovery");
    assert_eq!(recovery.action_kind, "restart_headless_daemon");
}

#[test]
fn reconcile_headless_daemon_state_prefers_active_execution_lease() {
    let home = temp_runtime_home("reconcile-active-execution-lease");
    ensure_runtime_home_layout(&home).expect("runtime home");
    fs::create_dir_all(home.join("runtime/current")).expect("current dir");
    fs::create_dir_all(home.join("runtime/leases")).expect("lease dir");
    fs::create_dir_all(home.join("runtime/pids")).expect("pid dir");
    write_file(
        &home.join("runtime/current/current_daemon_state.json"),
        br#"{
  "daemon_id":"mbp.headless-daemon",
  "created_at":"2026-04-24T18:27:31+08:00",
  "updated_at":"2026-04-24T18:27:31+08:00",
  "service_kind":"headless_daemon",
  "lifecycle_state":"active",
  "supervision_state":"running_sessions",
  "mode":"detached",
  "pid":999999,
  "health_state":"healthy",
  "recovery_needed":false,
  "recovery_action_kind":"observe_only",
  "active_binding":"session=session-system-entry",
  "status_summary":"daemon source=headless_loop"
}"#,
    )
    .expect("daemon state");
    write_file(
        &home.join("runtime/leases/headless-daemon.json"),
        br#"{
  "daemon_id":"mbp.headless-daemon",
  "pid":999999,
  "heartbeat_interval_ms":5000,
  "lease_ttl_ms":15000,
  "started_at":"2026-04-24T18:20:00+08:00",
  "updated_at":"2026-04-24T18:27:31+08:00",
  "lifecycle_state":"active",
  "active_session_ids":["session-system-entry"],
  "processed_sessions":1,
  "drove_count":1
}"#,
    )
    .expect("lease");
    write_file(&home.join("runtime/pids/headless-daemon.pid"), b"999999").expect("pid");
    let current_pid = std::process::id();
    let execution_lease = format!(
        r#"{{
  "lease_id":"lease-op-system-entry-0030",
  "session_id":"session-system-entry",
  "task_id":"task-codex-computer-use-mcp-toolu_3b67674b4f2d4f819874c745",
  "operation_id":"op-system-entry-0030",
  "pid":{current_pid},
  "owner_kind":"cli_turn_runner",
  "status":"active",
  "started_at":"2026-04-26T09:50:19+08:00",
  "updated_at":"2026-04-26T09:50:19+08:00"
}}"#
    );
    write_file(
        &home.join("runtime/current/current_execution_lease.json"),
        execution_lease.as_bytes(),
    )
    .expect("execution lease");

    let outcome = reconcile_headless_daemon_state(&home)
        .expect("reconcile")
        .expect("reconcile outcome");
    assert_eq!(outcome.state.lifecycle_state, "active");
    assert_eq!(outcome.state.health_state.as_deref(), Some("healthy"));
    assert_eq!(outcome.state.supervision_state, "running_sessions");
    assert!(!outcome.state.recovery_needed);
    assert_eq!(
        outcome.state.recovery_action_kind.as_deref(),
        Some("observe_only")
    );
    assert_eq!(outcome.state.pid, Some(current_pid));
    assert!(
        outcome
            .state
            .status_summary
            .contains("daemon executing active closure"),
        "status summary should explain why stale daemon lease did not trigger restart"
    );
    assert_eq!(
        fs::read_to_string(home.join("runtime/pids/headless-daemon.pid"))
            .expect("pid file")
            .trim(),
        current_pid.to_string()
    );
}

#[test]
fn headless_daemon_keeps_running_until_stop_is_requested() {
    let _guard = env_lock().lock().expect("env lock");
    let previous = std::env::var("FIN_HEADLESS_DAEMON_MAX_CYCLES").ok();
    unsafe {
        std::env::remove_var("FIN_HEADLESS_DAEMON_MAX_CYCLES");
    }

    let home = temp_runtime_home("keepalive");
    ensure_runtime_home_layout(&home).expect("runtime home should init");
    let user_toml = sample_user_toml();
    let mut system = map_system_config(&user_toml).expect("system config");
    system.runtime.heartbeat_interval_ms = 50;

    let thread_home = home.clone();
    let thread_user_toml = user_toml.clone();
    let thread_system = system.clone();
    let join = thread::spawn(move || {
        let provider = HeadlessResumeProvider::new(&thread_system);
        run_headless_daemon_with_provider(&thread_user_toml, &thread_system, &provider, thread_home)
    });

    thread::sleep(Duration::from_millis(180));
    let stop_report =
        stop_headless_daemon(&user_toml, &system, Some(home.as_path())).expect("stop report");
    assert_eq!(stop_report.status, "stop_requested");

    let report = join.join().expect("daemon thread").expect("daemon report");
    assert!(
        report.cycles_completed >= 2,
        "daemon should survive beyond one cycle"
    );

    let daemon_state = fs::read_to_string(home.join("runtime/current/current_daemon_state.json"))
        .expect("daemon state");
    assert!(daemon_state.contains("\"lifecycle_state\": \"stopped\""));
    assert!(daemon_state.contains("daemon source=stop_requested"));

    if let Some(value) = previous {
        unsafe {
            std::env::set_var("FIN_HEADLESS_DAEMON_MAX_CYCLES", value);
        }
    } else {
        unsafe {
            std::env::remove_var("FIN_HEADLESS_DAEMON_MAX_CYCLES");
        }
    }
}


#[test]
fn headless_daemon_recovers_orphaned_running_state_before_waiting_forever() {
    let _guard = env_lock().lock().expect("env lock");
    let previous = std::env::var("FIN_HEADLESS_DAEMON_MAX_CYCLES").ok();
    unsafe {
        std::env::set_var("FIN_HEADLESS_DAEMON_MAX_CYCLES", "1");
    }

    let home = temp_runtime_home("orphaned-running");
    ensure_runtime_home_layout(&home).expect("runtime home should init");
    let user_toml = sample_user_toml();
    let system = map_system_config(&user_toml).expect("system config");
    let session_dir = home.join("sessions/2026/04/session-system-entry");
    for relative in [
        "conversation",
        "control",
        "queue",
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
    // Use recent timestamps so the checkpoint is not expired by the staleness guard.
    let recent_ts = crate::time::local_timestamp_now();
    let state_json = format!(
        r#"{{
  "state_id":"exec-state-op-system-entry-0019",
  "session_id":"session-system-entry",
  "task_id":null,
  "status":"running",
  "active_turn_id":"turn-op-system-entry-0019",
  "active_step_id":"step-op-system-entry-0019-01-context_build",
  "resume_from_step_id":"step-op-system-entry-0019-01-context_build",
  "resume_checkpoint_ready":false,
  "resume_checkpoint_id":null,
  "pending_input_count":0,
  "accepts_user_input":false,
  "reason":"active closure running",
  "updated_at":"{}"
}}"#,
        recent_ts
    );
    write_file(
        &session_dir.join("control/execution_state.json"),
        state_json.as_bytes(),
    )
    .expect("state");
    let lease_json = format!(
        r#"{{
  "lease_id":"lease-op-system-entry-0019",
  "session_id":"session-system-entry",
  "task_id":null,
  "operation_id":"op-system-entry-0019",
  "pid":999999,
  "owner_kind":"cli_turn_runner",
  "status":"active",
  "started_at":"{}",
  "updated_at":"{}"
}}"#,
        recent_ts, recent_ts
    );
    write_file(
        &session_dir.join("control/execution_lease.json"),
        lease_json.as_bytes(),
    )
    .expect("lease");
    let checkpoint_json = format!(
        r#"{{
  "checkpoint_id":"checkpoint-op-system-entry-0018-r11",
  "session_id":"session-system-entry",
  "task_id":null,
  "trace_id":"trace-system-entry-0018",
  "source_operation_id":"op-system-entry-0018",
  "source_turn_id":"turn-op-system-entry-0018",
  "source_step_id":"step-op-system-entry-0018-41-tool_dispatch",
  "checkpoint_kind":"tool_followup_resume",
  "status":"open",
  "source_round_index":10,
  "next_round_index":11,
  "resume_input":"Continue the same turn.\nOriginal request: report status\nLast assistant response: continue checking.",
  "summary":"resume follow-up round 11 after tool dispatch in round 10",
  "created_at":"{}",
  "consumed_at":null,
  "consumed_by_operation_id":null
}}"#,
        recent_ts
    );
    write_file(
        &session_dir.join("control/execution_checkpoint.json"),
        checkpoint_json.as_bytes(),
    )
    .expect("checkpoint");
    write_file(
        &home.join("runtime/current/last_run.json"),
        br#"{
  "session_id":"session-system-entry",
  "operation_id":"op-system-entry-0018",
  "current_execution_state_path":"runtime/current/current_execution_state.json",
  "current_execution_checkpoint_path":"runtime/current/current_execution_checkpoint.json"
}"#,
    )
    .expect("last run");

    let provider = HeadlessResumeProvider::new(&system);
    let report = run_headless_daemon_with_provider(&user_toml, &system, &provider, home.clone())
        .expect("daemon report");
    assert_eq!(
        report.drove_count, 1,
        "daemon should resume recovered checkpoint"
    );

    let state = fs::read_to_string(session_dir.join("control/execution_state.json"))
        .expect("execution state");
    assert!(state.contains("\"status\": \"idle\""));
    assert!(
        !session_dir.join("control/execution_lease.json").exists(),
        "orphaned execution lease should be cleared"
    );

    let messages = fs::read_to_string(session_dir.join("conversation/messages.json"))
        .expect("messages after recovery");
    assert!(messages.contains("canonical system startup session ready"));
    assert!(!messages.contains("后台守护已恢复并完成本轮检查"));
    assert!(!messages.contains("Continue the same turn."));
    let checkpoint = fs::read_to_string(session_dir.join("control/execution_checkpoint.json"))
        .expect("checkpoint after recovery");
    assert!(checkpoint.contains("\"status\": \"consumed\""));
    assert!(checkpoint.contains("\"consumed_by_operation_id\": \"op-system-entry-0019\""));
    let digests =
        fs::read_to_string(session_dir.join("digests/recent_digests.json")).expect("digests");
    assert_eq!(digests.trim(), "[]");
    let reasoning = fs::read_to_string(session_dir.join("reasoning/recent_reasoning_views.json"))
        .expect("reasoning");
    assert_eq!(reasoning.trim(), "[]");
    let tools =
        fs::read_to_string(session_dir.join("tools/recent_tool_records.json")).expect("tools");
    assert_eq!(tools.trim(), "[]");
    let events = fs::read_to_string(session_dir.join("events/stream.jsonl")).expect("events");
    assert_eq!(events.trim(), "");

    if let Some(value) = previous {
        unsafe {
            std::env::set_var("FIN_HEADLESS_DAEMON_MAX_CYCLES", value);
        }
    } else {
        unsafe {
            std::env::remove_var("FIN_HEADLESS_DAEMON_MAX_CYCLES");
        }
    }
}
