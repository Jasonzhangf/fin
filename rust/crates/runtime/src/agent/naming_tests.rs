use super::*;
use crate::RuntimeError;
use fin_config::SystemConfig;
use fin_config::{
    ConfigMapper, ProviderProtocol, UserConfig, UserProviderConfig, UserRuntimeConfig,
};
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
    let path = std::env::temp_dir().join(format!("fin-agent-naming-{prefix}-{unique}"));
    fs::create_dir_all(&path).expect("temp runtime home");
    path
}

fn system(device_name: Option<&str>) -> SystemConfig {
    ConfigMapper::map_user_to_system(&UserConfig {
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
        runtime: UserRuntimeConfig {
            device_name: device_name.map(str::to_string),
        },
    })
    .expect("system config")
}

#[test]
fn configured_device_name_wins_and_requested_name_is_stable() {
    let runtime_home = temp_runtime_home("configured-device");
    let identity = allocate_local_agent_identity(
        &system(Some("mac-mini")),
        &runtime_home,
        Some("system"),
        "cli",
        Some("system"),
    )
    .expect("identity");
    assert_eq!(identity.device_name, "mac-mini");
    assert_eq!(identity.agent_name, "system");
    assert_eq!(identity.agent_id, "mac-mini.system");
    assert_eq!(identity.worker_id, "worker-system");

    let registry = fs::read_to_string(runtime_home.join("runtime/agents/registry.json"))
        .expect("registry should exist");
    assert!(registry.contains("mac-mini.system"));
}

#[test]
fn resolve_agent_identity_by_worker_id_uses_registry() {
    let runtime_home = temp_runtime_home("worker-lookup");
    let identity = allocate_local_agent_identity(
        &system(Some("studio")),
        &runtime_home,
        Some("atlas"),
        "cli",
        Some("project"),
    )
    .expect("identity");

    let resolved =
        resolve_agent_identity_by_worker_id(&runtime_home, &identity.worker_id).expect("lookup");
    let resolved = resolved.expect("identity should resolve");
    assert_eq!(resolved.agent_id, identity.agent_id);
    assert_eq!(resolved.agent_name, identity.agent_name);
    assert_eq!(resolved.device_name, identity.device_name);
}

#[test]
fn persist_and_read_assignment_summary_round_trip() {
    let runtime_home = temp_runtime_home("assignment-summary");
    let summary = AgentAssignmentSummary {
        assignment_id: "assign-1".into(),
        worker_id: "worker-atlas".into(),
        peer_id: "local-worker-atlas".into(),
        target_agent_id: Some("studio.atlas".into()),
        target_agent_name: Some("atlas".into()),
        requested_role_id: "project".into(),
        owner_worker_id: Some("worker-system".into()),
        task_summary: "inspect logs".into(),
        status: "pending".into(),
        created_at: "2026-04-20T00:00:00+08:00".into(),
    };
    persist_assignment_summary(&runtime_home, &summary).expect("persist");

    let current = read_assignment_summary(&runtime_home)
        .expect("read summary")
        .expect("summary should exist");
    assert_eq!(current.assignment_id, "assign-1");
    assert_eq!(current.target_agent_name.as_deref(), Some("atlas"));
}

#[test]
fn local_name_pool_allocates_distinct_project_names() {
    let runtime_home = temp_runtime_home("pool");
    let system = system(Some("mbp"));

    let first =
        allocate_local_agent_identity(&system, &runtime_home, None, "cli.a", Some("project"))
            .expect("first");
    let second =
        allocate_local_agent_identity(&system, &runtime_home, None, "cli.b", Some("project"))
            .expect("second");

    assert_ne!(first.agent_id, second.agent_id);
    assert_ne!(first.worker_id, second.worker_id);
    assert_eq!(first.agent_id, "mbp.atlas");
    assert_eq!(second.agent_id, "mbp.nova");
}

#[test]
fn select_device_name_falls_back_to_host_sources() {
    assert_eq!(
        select_device_name(None, Some("Office Mac"), Some("ignored"), None),
        "office-mac"
    );
    assert_eq!(
        select_device_name(None, None, Some("Build Host"), None),
        "build-host"
    );
    assert_eq!(
        select_device_name(None, None, None, Some("render-node")),
        "render-node"
    );
}
