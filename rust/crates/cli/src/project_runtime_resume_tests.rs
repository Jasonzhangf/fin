use crate::project_runtime_resume::drive_ready_project_runtime_resumes;
use fin_config::{
    ConfigMapper, ProjectAgentMode, ProjectAgentStartupConfig, ProviderProtocol, SystemConfig,
    UserConfig, UserProviderConfig, UserRuntimeConfig,
};
use fin_debug_server::ChatSendResponse;
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

fn temp_runtime_home(prefix: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time")
        .as_nanos();
    let path = std::env::temp_dir().join(format!("fin-project-runtime-resume-{prefix}-{unique}"));
    fs::create_dir_all(&path).expect("temp runtime home");
    path
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

fn write_file(path: &Path, bytes: &[u8]) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("parent");
    }
    fs::write(path, bytes).expect("write");
}

#[test]
fn drive_ready_project_runtime_resumes_ticks_ready_project_session() {
    let home = temp_runtime_home("tick");
    let session_dir = home.join("sessions/2026/04/session-fin");
    fs::create_dir_all(session_dir.join("conversation")).expect("conversation");
    fs::create_dir_all(session_dir.join("context")).expect("context");
    fs::create_dir_all(session_dir.join("queue")).expect("queue");
    fs::create_dir_all(session_dir.join("tasks/registry")).expect("tasks");
    write_file(&session_dir.join("conversation/messages.json"), b"[]");
    write_file(
        &session_dir.join("context/current_context.json"),
        br#"{"project":{"primary_project":{"project_id":"fin"}}}"#,
    );
    write_file(
        &session_dir.join("queue/pending_inputs.json"),
        br#"[
  {
    "pending_input_id":"pending-session-fin-01",
    "refs":{"session_id":"session-fin","task_id":"task-fin-1"},
    "input_kind":"chat",
    "source":"project.resume",
    "message":"continue work",
    "attachments":[],
    "status":"pending",
    "enqueue_reason":"resume",
    "enqueued_at":"2026-04-20T23:40:00+08:00"
  }
]"#,
    );
    write_file(
        &session_dir.join("tasks/registry/task-fin-1.json"),
        br#"{
  "task_id":"task-fin-1",
  "session_id":"session-fin",
  "title":"task",
  "summary":"task",
  "status":"claimed",
  "claimed_by_worker_id":"worker-mbp-builder",
  "created_at":"2026-04-20T23:39:00+08:00",
  "updated_at":"2026-04-20T23:39:00+08:00"
}"#,
    );
    write_file(
        &home.join("runtime/current/current_project_execution_handoffs.json"),
        br#"{
  "updated_at":"2026-04-20T23:41:00+08:00",
  "project_count":1,
  "prepared_count":1,
  "noop_count":0,
  "missing_task_count":0,
  "projects":[{
    "project_id":"fin",
    "agent_id":"mbp.builder",
    "worker_id":"worker-mbp-builder",
    "session_id":"session-fin",
    "task_id":"task-fin-1",
    "handoff_state":"prepared",
    "updated_at":"2026-04-20T23:41:00+08:00",
    "summary":"prepared",
    "artifact_refs":[]
  }]
}"#,
    );
    write_file(&home.join("runtime/current/last_run.json"), b"{}");

    let report = drive_ready_project_runtime_resumes(
        &home,
        &system(),
        "project_runtime_resume",
        "2026-04-20T23:42:00+08:00",
        |binding, _message, _source, _attachments, _merge_segment| {
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
        fs::read_to_string(session_dir.join("queue/pending_inputs.json")).expect("pending inputs");
    assert_eq!(pending.trim(), "[]");
    let state = fs::read_to_string(session_dir.join("control/execution_state.json"))
        .expect("execution state");
    assert!(state.contains("\"status\": \"idle\""));

    let pickup =
        fs::read_to_string(home.join("runtime/current/current_project_runtime_pickups.json"))
            .expect("pickup");
    assert!(pickup.contains("\"pickup_state\": \"claimed_idle\""));
}

#[test]
fn drive_ready_project_runtime_resumes_seeds_claimed_idle_project_queue() {
    let home = temp_runtime_home("seed");
    let session_dir = home.join("sessions/2026/04/session-fin");
    fs::create_dir_all(session_dir.join("conversation")).expect("conversation");
    fs::create_dir_all(session_dir.join("context")).expect("context");
    fs::create_dir_all(session_dir.join("queue")).expect("queue");
    fs::create_dir_all(session_dir.join("tasks/registry")).expect("tasks");
    write_file(&session_dir.join("conversation/messages.json"), b"[]");
    write_file(
        &session_dir.join("context/current_context.json"),
        br#"{"project":{"primary_project":{"project_id":"fin"}}}"#,
    );
    write_file(&session_dir.join("queue/pending_inputs.json"), b"[]");
    write_file(
        &session_dir.join("tasks/registry/task-fin-1.json"),
        br#"{
  "task_id":"task-fin-1",
  "session_id":"session-fin",
  "title":"task",
  "summary":"task",
  "status":"claimed",
  "claimed_by_worker_id":"worker-mbp-builder",
  "created_at":"2026-04-20T23:39:00+08:00",
  "updated_at":"2026-04-20T23:39:00+08:00"
}"#,
    );
    write_file(
        &home.join("runtime/current/current_project_execution_handoffs.json"),
        br#"{
  "updated_at":"2026-04-20T23:41:00+08:00",
  "project_count":1,
  "prepared_count":1,
  "noop_count":0,
  "missing_task_count":0,
  "projects":[{
    "project_id":"fin",
    "agent_id":"mbp.builder",
    "worker_id":"worker-mbp-builder",
    "session_id":"session-fin",
    "task_id":"task-fin-1",
    "handoff_state":"prepared",
    "updated_at":"2026-04-20T23:41:00+08:00",
    "summary":"prepared",
    "artifact_refs":[]
  }]
}"#,
    );
    write_file(&home.join("runtime/current/last_run.json"), b"{}");

    let report = drive_ready_project_runtime_resumes(
        &home,
        &system(),
        "project_runtime_resume",
        "2026-04-20T23:42:00+08:00",
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
        fs::read_to_string(session_dir.join("queue/pending_inputs.json")).expect("pending inputs");
    assert_eq!(pending.trim(), "[]");

    let latest_queue =
        fs::read_to_string(home.join("runtime/current/current_pending_inputs.json")).unwrap();
    assert_eq!(latest_queue.trim(), "[]");

    let pickup =
        fs::read_to_string(home.join("runtime/current/current_project_runtime_pickups.json"))
            .expect("pickup");
    assert!(pickup.contains("\"pickup_state\": \"claimed_idle\""));
}
