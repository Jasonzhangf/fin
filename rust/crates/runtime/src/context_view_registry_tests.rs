use crate::{ContextAssemblyInput, ContextViewBuilder, WorkerRuntime};
use fin_config::{ConfigMapper, ProviderProtocol, UserConfig, UserProviderConfig};
use std::{collections::BTreeMap, fs};

fn system_worker_runtime() -> WorkerRuntime {
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
        "worker-system",
        "runtime",
        Some("system"),
    )
    .expect("system worker runtime")
}

#[test]
fn system_context_view_loads_active_and_registered_projects_from_runtime_registry() {
    let worker = system_worker_runtime();
    let runtime_home = std::env::temp_dir().join("fin-context-project-registry-test");
    fs::create_dir_all(runtime_home.join("runtime/projects")).expect("project runtime dir");
    fs::write(
        runtime_home.join("runtime/projects/registry.json"),
        br#"[
  {
    "project_id":"fin",
    "agent_id":"mbp.builder",
    "mode":"local",
    "project_root":"/tmp/fin",
    "always_on":true,
    "auto_resume":true,
    "auto_connect":true,
    "worker_budget":2,
    "unfinished_task_count":2,
    "presence_state":"busy",
    "wake_state":"steady",
    "updated_at":"2026-04-20T21:00:00+08:00"
  },
  {
    "project_id":"infra",
    "agent_id":"mbp.infra",
    "mode":"local",
    "project_root":"/tmp/infra",
    "always_on":true,
    "auto_resume":true,
    "auto_connect":true,
    "worker_budget":1,
    "unfinished_task_count":1,
    "presence_state":"idle",
    "wake_state":"steady",
    "updated_at":"2026-04-20T21:00:00+08:00"
  },
  {
    "project_id":"archive",
    "agent_id":"mbp.archive",
    "mode":"local",
    "project_root":"/tmp/archive",
    "always_on":false,
    "auto_resume":false,
    "auto_connect":false,
    "worker_budget":1,
    "unfinished_task_count":0,
    "presence_state":"offline",
    "wake_state":"steady",
    "updated_at":"2026-04-20T21:00:00+08:00"
  }
]"#,
    )
    .expect("project registry");
    fs::create_dir_all(runtime_home.join("runtime/current")).expect("current dir");
    fs::write(
        runtime_home.join("runtime/current/current_agent_presence_registry.json"),
        br#"{
  "agents":[
    {"agent_id":"mbp.system","agent_name":"system","device_name":"mbp","status":"busy"},
    {"agent_id":"mbp.builder","agent_name":"builder","device_name":"mbp","status":"idle"},
    {"agent_id":"mbp.infra","agent_name":"infra","device_name":"mbp","status":"waiting"}
  ]
}"#,
    )
    .expect("presence registry");
    fs::write(
        runtime_home.join("runtime/current/current_project_supervision.json"),
        br#"{
  "ready_count":0,
  "resume_ready_count":1,
  "busy_count":1,
  "waiting_count":1,
  "recover_needed_count":1,
  "projects":[
    {"project_id":"fin","desired_action":"monitor_running_task"},
    {"project_id":"infra","desired_action":"resume_project_task"},
    {"project_id":"archive","desired_action":"recover_project_agent"}
  ]
}"#,
    )
    .expect("project supervision");
    fs::create_dir_all(runtime_home.join("runtime/assignments")).expect("assignments dir");
    fs::write(
        runtime_home.join("runtime/assignments/pending.json"),
        br#"[
  {
    "peer_id":"local-worker-b",
    "target_worker_id":"worker-b",
    "owner_worker_id":"worker-system",
    "status":"pending"
  },
  {
    "peer_id":"local-worker-c",
    "target_worker_id":"worker-c",
    "owner_worker_id":"worker-system",
    "status":"pending"
  }
]"#,
    )
    .expect("pending assignments");
    fs::create_dir_all(runtime_home.join("runtime/mailbox/local-worker-b")).expect("mailbox");
    fs::write(
        runtime_home.join("runtime/mailbox/local-worker-b/inbox.json"),
        br#"[
  {"target_peer_id":"local-worker-b"},
  {"target_peer_id":"local-worker-b"}
]"#,
    )
    .expect("mailbox inbox");

    let context = ContextViewBuilder.build(
        &worker,
        ContextAssemblyInput {
            operation_id: "op-system-projects".into(),
            trace_id: "trace-system-projects".into(),
            input: "inspect current portfolio".into(),
            source: "cli.user".into(),
            runtime_home: Some(runtime_home.display().to_string()),
            project_label: Some("fin".into()),
            ..ContextAssemblyInput::default()
        },
    );

    let project = context.project.expect("project block");
    assert_eq!(project.active_projects.len(), 2);
    assert_eq!(project.projects.len(), 3);
    assert_eq!(
        project
            .primary_project
            .as_ref()
            .map(|item| item.project_id.as_str()),
        Some("fin")
    );
    assert!(
        project
            .active_projects
            .iter()
            .any(|item| item.project_id == "infra")
    );
    assert!(
        project
            .projects
            .iter()
            .any(|item| item.project_id == "archive")
    );
    assert_eq!(
        project.active_agent_ids,
        vec![
            "mbp.builder".to_string(),
            "mbp.infra".to_string(),
            "mbp.system".to_string()
        ]
    );
    assert!(
        project
            .agent_presence_summary
            .as_deref()
            .unwrap_or_default()
            .contains("agents=3")
    );
    assert!(
        project
            .project_supervision_summary
            .as_deref()
            .unwrap_or_default()
            .contains("resume_ready=1")
    );
    assert!(
        project
            .assignment_queue_summary
            .as_deref()
            .unwrap_or_default()
            .contains("pending_assignments=2")
    );
    assert!(
        project
            .assignment_queue_summary
            .as_deref()
            .unwrap_or_default()
            .contains("worker-b<-worker-system:pending")
    );
    assert!(
        project
            .mailbox_summary
            .as_deref()
            .unwrap_or_default()
            .contains("mailbox_messages=2")
    );
    assert!(
        project
            .supervision_actions
            .iter()
            .any(|item| item == "infra:resume_project_task")
    );
    assert!(
        project
            .scope_summary
            .as_deref()
            .unwrap_or_default()
            .contains("active_projects=2 registered_projects=3")
    );
}
