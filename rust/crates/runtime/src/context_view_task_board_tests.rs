use crate::{ContextAssemblyInput, ContextViewBuilder, WorkerRuntime};
use fin_config::{ConfigMapper, ProviderProtocol, UserConfig, UserProviderConfig};
use fin_contracts::EntityRefs;
use std::{
    collections::BTreeMap,
    fs,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

fn temp_runtime_home(prefix: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "fin-context-view-{prefix}-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos()
    ))
}

fn worker_runtime() -> WorkerRuntime {
    let user = UserConfig {
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
        runtime: fin_config::UserRuntimeConfig::default(),
    };
    let system = ConfigMapper::map_user_to_system(&user).expect("mapping should succeed");
    WorkerRuntime::from_system(&system, "agent-1", "worker-1", "runtime", None)
        .expect("worker runtime")
}

#[test]
fn context_view_includes_task_board_digest_in_project_block() {
    let worker = worker_runtime();
    let runtime_home = temp_runtime_home("task-board");
    let session_dir = runtime_home.join("sessions/2026/04/session-rich");
    fs::create_dir_all(session_dir.join("tasks/routing")).expect("routing");
    fs::create_dir_all(session_dir.join("control")).expect("control");
    fs::create_dir_all(session_dir.join("tasks/plan")).expect("plan");
    fs::create_dir_all(session_dir.join("conversation")).expect("conversation");
    fs::write(
        session_dir.join("tasks/routing/latest_action.json"),
        br#"{"task_id":"task-rich","action_kind":"continue_current_task"}"#,
    )
    .expect("routing latest");
    fs::write(
        session_dir.join("control/execution_state.json"),
        br#"{"state_id":"state-1","task_id":"task-rich","status":"running","pending_input_count":2,"accepts_user_input":true,"updated_at":"2026-04-20T12:00:04+08:00"}"#,
    )
    .expect("execution state");
    fs::write(
        session_dir.join("tasks/plan/latest.json"),
        br#"{"steps":[{"step":"inspect","status":"completed"},{"step":"patch","status":"in_progress"}]}"#,
    )
    .expect("plan latest");
    fs::write(
        session_dir.join("conversation/messages.json"),
        br#"[{"message_id":"user-1","task_id":"task-rich"}]"#,
    )
    .expect("messages");

    let context = ContextViewBuilder.build(
        &worker,
        ContextAssemblyInput {
            operation_id: "op-task-board".into(),
            trace_id: "trace-task-board".into(),
            refs: EntityRefs {
                session_id: Some("session-rich".into()),
                task_id: Some("task-rich".into()),
                ..EntityRefs::default()
            },
            input: "continue current task".into(),
            source: "cli.user".into(),
            runtime_home: Some(runtime_home.display().to_string()),
            ..ContextAssemblyInput::default()
        },
    );

    assert_eq!(
        context
            .project
            .as_ref()
            .and_then(|project| project.active_task_id.as_deref()),
        Some("task-rich")
    );
    assert!(
        context
            .project
            .as_ref()
            .and_then(|project| project.task_board_summary.as_deref())
            .unwrap_or_default()
            .contains("status=running")
    );
    assert_eq!(
        context
            .project
            .as_ref()
            .map(|project| project.known_task_ids.as_slice()),
        Some(&["task-rich".to_string()][..])
    );
}

#[test]
fn context_view_combines_task_board_and_collab_backlog_for_same_active_task() {
    let worker = {
        let user = UserConfig {
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
            runtime: fin_config::UserRuntimeConfig::default(),
        };
        let system = ConfigMapper::map_user_to_system(&user).expect("mapping should succeed");
        WorkerRuntime::from_system(
            &system,
            "agent-system",
            "worker-1",
            "runtime",
            Some("system"),
        )
        .expect("system worker runtime")
    };
    let runtime_home = temp_runtime_home("task-board-collab");
    let session_dir = runtime_home.join("sessions/2026/04/session-collab");
    fs::create_dir_all(session_dir.join("tasks/routing")).expect("routing");
    fs::create_dir_all(session_dir.join("control")).expect("control");
    fs::create_dir_all(session_dir.join("tasks/plan")).expect("plan");
    fs::create_dir_all(session_dir.join("conversation")).expect("conversation");
    fs::create_dir_all(runtime_home.join("runtime/projects")).expect("projects");
    fs::create_dir_all(runtime_home.join("runtime/assignments")).expect("assignments");
    fs::create_dir_all(runtime_home.join("runtime/mailbox/local-worker-2")).expect("mailbox");
    fs::write(
        session_dir.join("tasks/routing/latest_action.json"),
        br#"{"task_id":"task-collab","action_kind":"continue_current_task"}"#,
    )
    .expect("routing latest");
    fs::write(
        session_dir.join("control/execution_state.json"),
        br#"{"state_id":"state-2","task_id":"task-collab","status":"running","pending_input_count":1,"accepts_user_input":true,"updated_at":"2026-04-20T12:30:04+08:00"}"#,
    )
    .expect("execution state");
    fs::write(
        session_dir.join("tasks/plan/latest.json"),
        br#"{"steps":[{"step":"inspect backlog","status":"completed"},{"step":"dispatch worker","status":"in_progress"}]}"#,
    )
    .expect("plan latest");
    fs::write(
        session_dir.join("conversation/messages.json"),
        br#"[{"message_id":"user-1","task_id":"task-collab"},{"message_id":"assistant-1","task_id":"task-collab"}]"#,
    )
    .expect("messages");
    fs::write(
        runtime_home.join("runtime/projects/registry.json"),
        br#"[
  {
    "project_id":"fin",
    "agent_id":"agent-system",
    "project_root":"/tmp/fin",
    "presence_state":"busy",
    "unfinished_task_count":1
  }
]"#,
    )
    .expect("project registry");
    fs::write(
        runtime_home.join("runtime/assignments/pending.json"),
        br#"[
  {
    "peer_id":"local-worker-2",
    "target_worker_id":"worker-2",
    "owner_worker_id":"worker-1",
    "status":"pending"
  }
]"#,
    )
    .expect("pending assignments");
    fs::write(
        runtime_home.join("runtime/mailbox/local-worker-2/inbox.json"),
        br#"[
  {"target_peer_id":"local-worker-2","summary":"inspect src"},
  {"target_peer_id":"local-worker-2","summary":"report back"}
]"#,
    )
    .expect("mailbox inbox");

    let context = ContextViewBuilder.build(
        &worker,
        ContextAssemblyInput {
            operation_id: "op-task-board-collab".into(),
            trace_id: "trace-task-board-collab".into(),
            refs: EntityRefs {
                session_id: Some("session-collab".into()),
                task_id: Some("task-collab".into()),
                ..EntityRefs::default()
            },
            input: "resume active task with worker backlog".into(),
            source: "cli.user".into(),
            runtime_home: Some(runtime_home.display().to_string()),
            ..ContextAssemblyInput::default()
        },
    );

    let project = context.project.expect("project block");
    assert_eq!(project.active_task_id.as_deref(), Some("task-collab"));
    assert!(
        project
            .task_board_summary
            .as_deref()
            .unwrap_or_default()
            .contains("pending_inputs=1")
    );
    assert!(
        project
            .task_board_summary
            .as_deref()
            .unwrap_or_default()
            .contains("plan_steps=2")
    );
    assert!(
        project
            .assignment_queue_summary
            .as_deref()
            .unwrap_or_default()
            .contains("pending_assignments=1")
    );
    assert!(
        project
            .mailbox_summary
            .as_deref()
            .unwrap_or_default()
            .contains("mailbox_messages=2")
    );
}
