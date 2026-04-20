use super::*;
use crate::project_runtime_resume::drive_ready_project_runtime_resumes;
use fin_config::{
    ConfigMapper, ProviderProtocol, UserConfig, UserProviderConfig, UserRuntimeConfig,
};
use fin_debug_server::ChatSendResponse;
use serde_json::Value;
use std::{collections::BTreeMap, fs};

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

fn temp_home(prefix: &str) -> std::path::PathBuf {
    let unique = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system time")
        .as_nanos();
    let path = std::env::temp_dir().join(format!("fin-startup-wakeup-{prefix}-{unique}"));
    fs::create_dir_all(&path).expect("home");
    path
}

#[test]
fn refresh_executes_local_wake_and_clears_queue_on_rebuild() {
    let home = temp_home("local");
    let snapshot = refresh_startup_control_plane(&home, &system(), "2026-04-20T11:00:00+08:00")
        .expect("startup refresh");
    assert_eq!(snapshot.projects.len(), 1);
    assert_eq!(snapshot.projects[0].presence_state, "idle");
    assert_eq!(snapshot.projects[0].wake_state, "steady");
    assert!(snapshot.wake_queue.is_empty());
    assert!(
        home.join("runtime/agents/state/device.builder.json")
            .exists()
            || home.join("runtime/agents/state").exists()
    );
    let peer_state =
        fs::read_to_string(home.join("runtime/peers/registry.json")).expect("peer registry");
    assert!(peer_state.contains("peer-project-agent-fin"));
    let wake_report = fs::read_to_string(home.join("runtime/current/current_startup_wakeup.json"))
        .expect("wakeup report");
    assert!(wake_report.contains(r#""status": "completed""#));
}

#[test]
fn remote_wake_becomes_waiting_not_idle() {
    let home = temp_home("remote");
    let mut system = system();
    system.runtime.startup.project_agents[0].mode = ProjectAgentMode::Remote;
    system.runtime.startup.project_agents[0].project_root = None;
    system.runtime.startup.project_agents[0].endpoint = Some("tcp://10.0.0.8:4711".into());
    let snapshot = refresh_startup_control_plane(&home, &system, "2026-04-20T11:10:00+08:00")
        .expect("startup refresh");
    assert_eq!(snapshot.projects[0].presence_state, "waiting");
    let wake_report: Value = serde_json::from_str(
        &fs::read_to_string(home.join("runtime/current/current_startup_wakeup.json"))
            .expect("wake report"),
    )
    .expect("json");
    assert_eq!(
        wake_report["actions"][0]["status"].as_str(),
        Some("recorded")
    );
}

#[test]
fn refresh_builds_resume_chain_and_auto_resume_can_drive_claimed_idle_project() {
    let home = temp_home("resume-chain");
    let session_dir = home.join("sessions/2026/04/session-fin");
    fs::create_dir_all(session_dir.join("conversation")).expect("conversation");
    fs::create_dir_all(session_dir.join("context")).expect("context");
    fs::create_dir_all(session_dir.join("control")).expect("control");
    fs::create_dir_all(session_dir.join("queue")).expect("queue");
    fs::create_dir_all(session_dir.join("tasks/registry")).expect("tasks");
    fs::write(session_dir.join("conversation/messages.json"), b"[]").expect("messages");
    fs::write(
        session_dir.join("context/current_context.json"),
        br#"{"project":{"primary_project":{"project_id":"fin"}}}"#,
    )
    .expect("context");
    fs::write(
        session_dir.join("control/execution_state.json"),
        br#"{
  "state_id":"exec-state-project",
  "session_id":"session-fin",
  "task_id":"task-fin-1",
  "status":"idle",
  "pending_input_count":1,
  "accepts_user_input":true,
  "updated_at":"2026-04-20T11:59:00+08:00"
}"#,
    )
    .expect("state");
    fs::write(session_dir.join("queue/pending_inputs.json"), b"[]").expect("pending");
    fs::write(
        session_dir.join("tasks/registry/task-fin-1.json"),
        br#"{
  "task_id":"task-fin-1",
  "session_id":"session-fin",
  "title":"task",
  "summary":"task",
  "status":"ready",
  "created_at":"2026-04-20T11:58:00+08:00",
  "updated_at":"2026-04-20T11:58:00+08:00"
}"#,
    )
    .expect("task");

    let system = system();
    let snapshot = refresh_startup_control_plane(&home, &system, "2026-04-20T12:00:00+08:00")
        .expect("startup refresh");
    assert_eq!(snapshot.projects.len(), 1);
    assert_eq!(snapshot.projects[0].presence_state, "idle");
    assert_eq!(snapshot.projects[0].unfinished_task_count, 1);
    assert_eq!(
        snapshot.projects[0].last_active_task_id.as_deref(),
        Some("task-fin-1")
    );

    let supervision =
        fs::read_to_string(home.join("runtime/current/current_project_supervision.json"))
            .expect("supervision");
    assert!(supervision.contains(r#""supervision_state": "resume_ready""#));
    assert!(supervision.contains(r#""desired_action": "resume_project_task""#));

    let handoffs =
        fs::read_to_string(home.join("runtime/current/current_project_execution_handoffs.json"))
            .expect("handoffs");
    assert!(handoffs.contains(r#""handoff_state": "prepared""#));
    assert!(handoffs.contains(r#""task_id": "task-fin-1""#));

    let pickups =
        fs::read_to_string(home.join("runtime/current/current_project_runtime_pickups.json"))
            .expect("pickups");
    assert!(pickups.contains(r#""pickup_state": "claimed_idle""#));
    assert!(pickups.contains(r#""next_action": "await_manual_work""#));

    let report = drive_ready_project_runtime_resumes(
        &home,
        &system,
        "project_runtime_resume",
        "2026-04-20T12:01:00+08:00",
        |binding, message, source, _attachments, _merge_segment| {
            assert_eq!(message, "continue work");
            assert_eq!(source, "project.resume");
            Ok(ChatSendResponse {
                binding,
                answer: "done".into(),
                digest_id: "digest-project-resume".into(),
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
    .expect("resume report");

    assert_eq!(report.attempted_count, 1);
    assert_eq!(report.drove_count, 1);
    let pending =
        fs::read_to_string(session_dir.join("queue/pending_inputs.json")).expect("pending");
    assert_eq!(pending.trim(), "[]");
}
