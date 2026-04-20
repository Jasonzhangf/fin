use crate::{
    project_execution_handoff::materialize_project_execution_handoffs,
    project_runtime_pickup::materialize_project_runtime_pickups,
    project_supervision::{ProjectSupervisionRecord, ProjectSupervisionSnapshot},
};
use fin_config::{
    ConfigMapper, ProjectAgentMode, ProjectAgentStartupConfig, ProviderProtocol, SystemConfig,
    UserConfig, UserProviderConfig, UserRuntimeConfig,
};
use fin_contracts::{EntityRefs, ExecutionStateRecord};
use serde_json::json;
use std::{
    collections::BTreeMap,
    fs,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

fn temp_runtime_home(prefix: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time")
        .as_nanos();
    let path = std::env::temp_dir().join(format!("fin-project-runtime-pickup-{prefix}-{unique}"));
    fs::create_dir_all(path.join("sessions/2026/04/session-1/tasks/registry"))
        .expect("temp runtime home");
    fs::create_dir_all(path.join("sessions/2026/04/session-1/control")).expect("control");
    fs::create_dir_all(path.join("sessions/2026/04/session-1/queue")).expect("queue");
    fs::create_dir_all(path.join("sessions/2026/04/session-1/conversation")).expect("conversation");
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

#[test]
fn materialize_marks_ready_to_resume_when_claimed_task_has_pending_inputs() {
    let home = temp_runtime_home("ready");
    fs::write(
        home.join("sessions/2026/04/session-1/tasks/registry/task-1.json"),
        serde_json::to_vec_pretty(&json!({
            "task_id":"task-1",
            "session_id":"session-1",
            "title":"task",
            "summary":"task",
            "status":"ready",
            "created_at":"2026-04-20T23:00:00+08:00",
            "updated_at":"2026-04-20T23:00:00+08:00"
        }))
        .expect("task"),
    )
    .expect("task");
    fs::write(
        home.join("sessions/2026/04/session-1/control/execution_state.json"),
        serde_json::to_vec_pretty(&ExecutionStateRecord {
            state_id: "state-1".into(),
            refs: EntityRefs {
                session_id: Some("session-1".into()),
                task_id: Some("task-1".into()),
                ..EntityRefs::default()
            },
            status: "idle".into(),
            pending_input_count: 1,
            accepts_user_input: true,
            updated_at: "2026-04-20T23:00:00+08:00".into(),
            ..ExecutionStateRecord::default()
        })
        .expect("state"),
    )
    .expect("state");
    fs::write(
        home.join("sessions/2026/04/session-1/queue/pending_inputs.json"),
        serde_json::to_vec_pretty(&json!([{
            "pending_input_id":"pending-1",
            "refs":{"session_id":"session-1","task_id":"task-1"},
            "input_kind":"chat",
            "source":"user.chat",
            "message":"continue",
            "status":"queued",
            "enqueue_reason":"test",
            "enqueued_at":"2026-04-20T23:00:00+08:00"
        }]))
        .expect("pending"),
    )
    .expect("pending");
    fs::write(
        home.join("sessions/2026/04/session-1/conversation/messages.json"),
        serde_json::to_vec_pretty(&json!([])).expect("messages"),
    )
    .expect("messages");

    let supervision = ProjectSupervisionSnapshot {
        updated_at: "2026-04-20T23:01:00+08:00".into(),
        project_count: 1,
        resume_ready_count: 1,
        projects: vec![ProjectSupervisionRecord {
            project_id: "fin".into(),
            agent_id: "mbp.builder".into(),
            mode: "local".into(),
            presence_state: "idle".into(),
            unfinished_task_count: 1,
            resume_task_id: Some("task-1".into()),
            supervision_state: "resume_ready".into(),
            desired_action: "resume_project_task".into(),
            updated_at: "2026-04-20T23:01:00+08:00".into(),
            summary: "resume".into(),
        }],
        ..ProjectSupervisionSnapshot::default()
    };
    let _ = materialize_project_execution_handoffs(&home, &supervision, &supervision.updated_at)
        .expect("handoff");

    let snapshot =
        materialize_project_runtime_pickups(&home, &system(), "2026-04-20T23:02:00+08:00")
            .expect("pickup");
    assert_eq!(snapshot.ready_count, 1);
    assert_eq!(snapshot.projects[0].pickup_state, "ready_to_resume");
    assert_eq!(snapshot.projects[0].next_action, "scheduler_tick_needed");
    let presence =
        fs::read_to_string(home.join("runtime/agents/state/mbp.builder.json")).expect("presence");
    assert!(presence.contains("\"status\": \"idle\""));
    assert!(presence.contains("\"current_task_id\": \"task-1\""));
}

#[test]
fn materialize_marks_handoff_idle_when_same_worker_already_claimed_task() {
    let home = temp_runtime_home("handoff-idle");
    fs::write(
        home.join("sessions/2026/04/session-1/tasks/registry/task-1.json"),
        serde_json::to_vec_pretty(&json!({
            "task_id":"task-1",
            "session_id":"session-1",
            "title":"task",
            "summary":"task",
            "status":"claimed",
            "claimed_by_worker_id":"worker-mbp-builder",
            "created_at":"2026-04-20T23:00:00+08:00",
            "updated_at":"2026-04-20T23:05:00+08:00"
        }))
        .expect("task"),
    )
    .expect("task");
    fs::write(
        home.join("sessions/2026/04/session-1/control/execution_state.json"),
        serde_json::to_vec_pretty(&ExecutionStateRecord {
            state_id: "state-1".into(),
            refs: EntityRefs {
                session_id: Some("session-1".into()),
                task_id: Some("task-1".into()),
                ..EntityRefs::default()
            },
            status: "idle".into(),
            pending_input_count: 0,
            accepts_user_input: true,
            updated_at: "2026-04-20T23:06:00+08:00".into(),
            ..ExecutionStateRecord::default()
        })
        .expect("state"),
    )
    .expect("state");
    fs::write(
        home.join("sessions/2026/04/session-1/queue/pending_inputs.json"),
        serde_json::to_vec_pretty(&json!([])).expect("pending"),
    )
    .expect("pending");
    fs::write(
        home.join("sessions/2026/04/session-1/conversation/messages.json"),
        serde_json::to_vec_pretty(&json!([])).expect("messages"),
    )
    .expect("messages");

    let supervision = ProjectSupervisionSnapshot {
        updated_at: "2026-04-20T23:07:00+08:00".into(),
        project_count: 1,
        resume_ready_count: 1,
        projects: vec![ProjectSupervisionRecord {
            project_id: "fin".into(),
            agent_id: "mbp.builder".into(),
            mode: "local".into(),
            presence_state: "idle".into(),
            unfinished_task_count: 1,
            resume_task_id: Some("task-1".into()),
            supervision_state: "resume_ready".into(),
            desired_action: "resume_project_task".into(),
            updated_at: "2026-04-20T23:07:00+08:00".into(),
            summary: "resume".into(),
        }],
        ..ProjectSupervisionSnapshot::default()
    };
    let handoffs =
        materialize_project_execution_handoffs(&home, &supervision, &supervision.updated_at)
            .expect("handoff");
    assert_eq!(handoffs.projects[0].handoff_state, "noop");

    let snapshot =
        materialize_project_runtime_pickups(&home, &system(), "2026-04-20T23:08:00+08:00")
            .expect("pickup");
    assert_eq!(snapshot.ready_count, 1);
    assert_eq!(snapshot.projects[0].pickup_state, "handoff_idle");
    assert_eq!(snapshot.projects[0].next_action, "await_new_project_input");
}
