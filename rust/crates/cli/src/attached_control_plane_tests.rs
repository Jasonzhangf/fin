use crate::attached_control_plane::run_attached_control_plane_cycle;
use fin_config::{
    ConfigMapper, ProjectAgentMode, ProjectAgentStartupConfig, ProviderProtocol, SystemConfig,
    UserConfig, UserProviderConfig, UserRuntimeConfig,
};
use fin_debug_server::{ChatSendResponse, DebugBinding};
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicUsize, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

fn temp_runtime_home(prefix: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time")
        .as_nanos();
    let path = std::env::temp_dir().join(format!("fin-attached-cycle-{prefix}-{unique}"));
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

fn binding(home: &Path) -> DebugBinding {
    DebugBinding {
        project_id: "fin".into(),
        project_label: "fin".into(),
        runtime_home: home.display().to_string(),
        session_id: Some("session-entry".into()),
        task_id: Some("task-entry".into()),
        session_messages_path: Some(
            "sessions/2026/04/session-entry/conversation/messages.json".into(),
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
fn attached_control_plane_cycle_drives_ready_project_resume() {
    let home = temp_runtime_home("resume");
    let entry_dir = home.join("sessions/2026/04/session-entry");
    fs::create_dir_all(entry_dir.join("conversation")).expect("entry conversation");
    fs::create_dir_all(entry_dir.join("control")).expect("entry control");
    fs::create_dir_all(entry_dir.join("queue")).expect("entry queue");
    fs::create_dir_all(entry_dir.join("tasks/routing")).expect("entry routing");
    write_file(&entry_dir.join("conversation/messages.json"), b"[]");
    write_file(
        &entry_dir.join("control/execution_state.json"),
        br#"{
  "state_id":"exec-state-entry",
  "session_id":"session-entry",
  "task_id":"task-entry",
  "status":"idle",
  "pending_input_count":0,
  "accepts_user_input":true,
  "updated_at":"2026-04-21T00:10:00+08:00"
}"#,
    );
    write_file(&entry_dir.join("queue/pending_inputs.json"), b"[]");
    write_file(&entry_dir.join("tasks/routing/latest_action.json"), b"{}");

    let project_dir = home.join("sessions/2026/04/session-fin");
    fs::create_dir_all(project_dir.join("conversation")).expect("project conversation");
    fs::create_dir_all(project_dir.join("context")).expect("project context");
    fs::create_dir_all(project_dir.join("control")).expect("project control");
    fs::create_dir_all(project_dir.join("queue")).expect("project queue");
    fs::create_dir_all(project_dir.join("tasks/registry")).expect("project tasks");
    write_file(&project_dir.join("conversation/messages.json"), b"[]");
    write_file(
        &project_dir.join("context/current_context.json"),
        br#"{"project":{"primary_project":{"project_id":"fin"}}}"#,
    );
    write_file(
        &project_dir.join("queue/pending_inputs.json"),
        br#"[
  {
    "pending_input_id":"pending-session-fin-01",
    "session_id":"session-fin",
    "task_id":"task-fin-1",
    "input_kind":"chat",
    "source":"project.resume",
    "message":"continue project",
    "attachments":[],
    "status":"pending",
    "enqueue_reason":"resume",
    "enqueued_at":"2026-04-21T00:11:00+08:00"
  }
]"#,
    );
    write_file(
        &project_dir.join("control/execution_state.json"),
        br#"{
  "state_id":"exec-state-project",
  "session_id":"session-fin",
  "task_id":"task-fin-1",
  "status":"idle",
  "pending_input_count":1,
  "accepts_user_input":true,
  "updated_at":"2026-04-21T00:11:00+08:00"
}"#,
    );
    write_file(
        &project_dir.join("tasks/registry/task-fin-1.json"),
        br#"{
  "task_id":"task-fin-1",
  "session_id":"session-fin",
  "title":"task",
  "summary":"task",
  "status":"claimed",
  "claimed_by_worker_id":"worker-mbp-builder",
  "created_at":"2026-04-21T00:10:00+08:00",
  "updated_at":"2026-04-21T00:10:00+08:00"
}"#,
    );

    write_file(&home.join("runtime/current/last_run.json"), b"{}");

    let entry_calls = AtomicUsize::new(0);
    let project_calls = AtomicUsize::new(0);
    let outcome = run_attached_control_plane_cycle(
        &home,
        &system(),
        &binding(&home),
        "web_debug_request",
        |_binding, _message, _source, _attachments, _merge_segment| {
            entry_calls.fetch_add(1, Ordering::Relaxed);
            Ok(ChatSendResponse {
                binding: binding(&home),
                answer: "entry".into(),
                digest_id: "digest-entry".into(),
                events_count: 0,
                response_kind: "assistant_message".into(),
                freshness: None,
                control_feedback: None,
                progress: None,
                note: None,
                routing_action: None,
            })
        },
        |binding, _agent_name, _message, _source, _attachments, _merge_segment| {
            project_calls.fetch_add(1, Ordering::Relaxed);
            Ok(ChatSendResponse {
                binding,
                answer: "project".into(),
                digest_id: "digest-project".into(),
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
    .expect("attached cycle");

    let pickup_snapshot =
        fs::read_to_string(home.join("runtime/current/current_project_runtime_pickups.json"))
            .unwrap_or_default();
    assert_eq!(
        outcome.project_resume.drove_count, 1,
        "resume_summary={} pickup_snapshot={}",
        outcome.project_resume.summary, pickup_snapshot
    );
    assert_eq!(
        project_calls.load(Ordering::Relaxed),
        1,
        "resume_summary={} pickup_snapshot={}",
        outcome.project_resume.summary,
        pickup_snapshot
    );
    assert_eq!(entry_calls.load(Ordering::Relaxed), 0);

    let pending =
        fs::read_to_string(project_dir.join("queue/pending_inputs.json")).expect("pending");
    assert_eq!(pending.trim(), "[]");
    let report =
        fs::read_to_string(home.join("runtime/current/current_project_runtime_resume.json"))
            .expect("resume report");
    assert!(report.contains("\"drove_count\": 1"));
}
