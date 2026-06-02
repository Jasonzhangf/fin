use crate::daemon_state::refresh_attached_daemon_state;
use fin_config::{
    ConfigMapper, ProjectAgentMode, ProjectAgentStartupConfig, ProviderProtocol,
    RuntimeRetentionConfig, SystemConfig, UserConfig, UserProviderConfig, UserRuntimeConfig,
};
use fin_debug_server::DebugBinding;
use std::{
    collections::BTreeMap,
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

fn system() -> SystemConfig {
    let mut system = ConfigMapper::map_user_to_system(&UserConfig {
        default_provider: "openai".into(),
        providers: BTreeMap::from([(
            "openai".into(),
            UserProviderConfig {
                protocol: ProviderProtocol::OpenAiCompatible,
                base_url: "https://api.example.com/v1".into(),
                model: "gpt-5".into(),
                api_key: None,
                api_key_env: Some("OPENAI_API_KEY".into()),
                user_agent: None,
                headers: BTreeMap::new(),
            },
        )]),
        runtime: UserRuntimeConfig::default(),
    })
    .expect("system");
    system.runtime.device_name = Some("mbp".into());
    system
        .runtime
        .startup
        .project_agents
        .push(ProjectAgentStartupConfig {
            project_id: "fin".into(),
            mode: ProjectAgentMode::Local,
            project_root: Some("/tmp/fin".into()),
            endpoint: None,
            agent_name: Some("builder".into()),
            worker_budget: 2,
            always_on: true,
            auto_resume: true,
            auto_connect: true,
        });
    system
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
        &system(),
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

#[test]
fn refresh_attached_daemon_state_observes_project_recovery_need() {
    let home = temp_runtime_home();
    let session_dir = home.join("sessions/2026/04/session-daemon");
    fs::create_dir_all(session_dir.join("conversation")).expect("conversation");
    write_file(&session_dir.join("conversation/messages.json"), b"[]");
    let project_session_dir = home.join("sessions/2026/04/session-fin");
    fs::create_dir_all(project_session_dir.join("context")).expect("context");
    fs::create_dir_all(project_session_dir.join("control")).expect("control");
    write_file(
        &project_session_dir.join("context/current_context.json"),
        br#"{"project":{"primary_project":{"project_id":"fin"}}}"#,
    );
    write_file(
        &project_session_dir.join("control/execution_state.json"),
        br#"{
  "state_id":"state-fin-1",
  "session_id":"session-fin",
  "task_id":"task-fin-1",
  "status":"running",
  "pending_input_count":0,
  "accepts_user_input":false,
  "updated_at":"2026-04-20T12:00:00+08:00"
}"#,
    );
    write_file(
        &home.join("runtime/current/current_startup_topology.json"),
        br#"{
  "updated_at":"2026-04-20T12:00:00+08:00",
  "entry_role":"system",
  "local_worker_budget":4,
  "projects":[{
    "project_id":"fin",
    "agent_id":"mbp.builder",
    "mode":"local",
    "project_root":"/tmp/fin",
    "endpoint":null,
    "always_on":true,
    "auto_resume":true,
    "auto_connect":true,
    "worker_budget":2,
    "unfinished_task_count":1,
    "last_active_task_id":"task-fin-1",
    "presence_state":"offline",
    "wake_state":"wake_requested",
    "wake_reason":"unfinished_work_detected",
    "updated_at":"2026-04-20T12:00:00+08:00"
  }],
  "wake_queue":[{
    "request_id":"wake-fin-1",
    "project_id":"fin",
    "agent_id":"mbp.builder",
    "reason":"unfinished_work_detected",
    "requested_by":"framework.startup",
    "auto_resume":true,
    "created_at":"2026-04-20T12:00:00+08:00"
  }]
}"#,
    );
    write_file(&home.join("runtime/current/last_run.json"), b"{}");

    let outcome = refresh_attached_daemon_state(
        &home,
        &system(),
        &binding(&home),
        "web_debug_request",
        &RuntimeRetentionConfig::default(),
        8,
    )
    .expect("daemon state");

    let state = outcome.state.expect("state");
    assert_ne!(state.supervision_state, "project_recovery_needed");
    assert!(state.status_summary.contains("recovery_exec=recovered=1"));
    let recovery = outcome.recovery_action.expect("recovery");
    assert_eq!(recovery.action_kind, "recover_project_agents");
    assert!(recovery.apply_immediately);
    assert!(!state.recovery_needed);

    let presence =
        fs::read_to_string(home.join("runtime/agents/state/mbp.builder.json")).expect("presence");
    assert!(presence.contains("\"status\": \"idle\""));
}

#[test]
fn refresh_attached_daemon_state_materializes_runtime_pickup_summary() {
    let home = temp_runtime_home();
    let session_dir = home.join("sessions/2026/04/session-daemon");
    fs::create_dir_all(session_dir.join("conversation")).expect("conversation");
    write_file(&session_dir.join("conversation/messages.json"), b"[]");

    let project_session_dir = home.join("sessions/2026/04/session-fin");
    fs::create_dir_all(project_session_dir.join("context")).expect("context");
    fs::create_dir_all(project_session_dir.join("control")).expect("control");
    fs::create_dir_all(project_session_dir.join("conversation")).expect("conversation");
    fs::create_dir_all(project_session_dir.join("tasks/registry")).expect("tasks");
    write_file(
        &project_session_dir.join("context/current_context.json"),
        br#"{"project":{"primary_project":{"project_id":"fin"}}}"#,
    );
    write_file(
        &project_session_dir.join("conversation/messages.json"),
        b"[]",
    );
    write_file(
        &project_session_dir.join("control/execution_state.json"),
        br#"{
  "state_id":"state-fin-1",
  "session_id":"session-fin",
  "task_id":"task-fin-1",
  "status":"running",
  "pending_input_count":0,
  "accepts_user_input":false,
  "updated_at":"2026-04-20T12:00:00+08:00"
}"#,
    );
    write_file(
        &project_session_dir.join("tasks/registry/task-fin-1.json"),
        br#"{
  "task_id":"task-fin-1",
  "session_id":"session-fin",
  "title":"task",
  "summary":"task",
  "status":"ready",
  "created_at":"2026-04-20T12:00:00+08:00",
  "updated_at":"2026-04-20T12:00:00+08:00"
}"#,
    );
    write_file(
        &home.join("runtime/current/current_startup_topology.json"),
        br#"{
  "updated_at":"2026-04-20T12:00:00+08:00",
  "entry_role":"system",
  "local_worker_budget":4,
  "projects":[{
    "project_id":"fin",
    "agent_id":"mbp.builder",
    "mode":"local",
    "project_root":"/tmp/fin",
    "endpoint":null,
    "always_on":true,
    "auto_resume":true,
    "auto_connect":true,
    "worker_budget":2,
    "unfinished_task_count":1,
    "last_active_task_id":"task-fin-1",
    "presence_state":"offline",
    "wake_state":"wake_requested",
    "wake_reason":"unfinished_work_detected",
    "updated_at":"2026-04-20T12:00:00+08:00"
  }],
  "wake_queue":[{
    "request_id":"wake-fin-1",
    "project_id":"fin",
    "agent_id":"mbp.builder",
    "reason":"unfinished_work_detected",
    "requested_by":"framework.startup",
    "auto_resume":true,
    "created_at":"2026-04-20T12:00:00+08:00"
  }]
}"#,
    );
    write_file(&home.join("runtime/current/last_run.json"), b"{}");

    let outcome = refresh_attached_daemon_state(
        &home,
        &system(),
        &binding(&home),
        "web_debug_request",
        &RuntimeRetentionConfig::default(),
        8,
    )
    .expect("daemon state");

    let state = outcome.state.expect("state");
    assert!(state.status_summary.contains("runtime_pickup=running=1"));

    let runtime_pickups =
        fs::read_to_string(home.join("runtime/current/current_project_runtime_pickups.json"))
            .expect("runtime pickups");
    assert!(runtime_pickups.contains("\"pickup_state\": \"running\""));
    assert!(runtime_pickups.contains("\"project_id\": \"fin\""));

    let presence =
        fs::read_to_string(home.join("runtime/agents/state/mbp.builder.json")).expect("presence");
    assert!(presence.contains("\"status\": \"busy\""));
    assert!(presence.contains("\"current_phase\": \"reasoning\""));
}
