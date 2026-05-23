use crate::agent_presence::{ensure_entry_agent_presence, ensure_project_agent_presence};
use fin_config::{
    ConfigMapper, ProjectAgentMode, ProjectAgentStartupConfig, ProviderProtocol, SystemConfig,
    UserConfig, UserProviderConfig, UserRuntimeConfig,
};
use serde_json::Value;
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
    let path = std::env::temp_dir().join(format!("fin-agent-presence-{prefix}-{unique}"));
    fs::create_dir_all(&path).expect("temp runtime home");
    path
}

fn system_config() -> SystemConfig {
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
}

#[test]
fn write_presence_updates_presence_registry() {
    let runtime_home = temp_runtime_home("registry");
    let system = system_config();
    let updated_at = "2026-04-20T15:00:00+08:00";

    let entry =
        ensure_entry_agent_presence(&system, &runtime_home, updated_at).expect("entry presence");
    let project = ensure_project_agent_presence(
        &system,
        &runtime_home,
        &ProjectAgentStartupConfig {
            project_id: "fin".into(),
            mode: ProjectAgentMode::Local,
            project_root: Some("/tmp/fin".into()),
            endpoint: None,
            agent_name: Some("builder".into()),
            worker_budget: 2,
            always_on: true,
            auto_resume: true,
            auto_connect: true,
        },
        updated_at,
    )
    .expect("project presence");

    let registry = fs::read_to_string(runtime_home.join("runtime/agents/presence_registry.json"))
        .expect("presence registry");
    assert!(registry.contains(&entry.agent_id));
    assert!(registry.contains(&project.agent_id));
    assert!(registry.contains("\"status\": \"idle\""));

    let registry_json: Value = serde_json::from_str(&registry).expect("registry json");
    let agents = registry_json["agents"]
        .as_array()
        .expect("agents array in presence registry");
    assert_eq!(
        agents.len(),
        8,
        "entry + 4 system workers + project agent + 2 project workers",
    );
    let system_worker_count = agents
        .iter()
        .filter(|item| item["agent_kind"] == "system_worker")
        .count();
    let project_worker_count = agents
        .iter()
        .filter(|item| item["agent_kind"] == "project_worker")
        .count();
    assert_eq!(system_worker_count, 4);
    assert_eq!(project_worker_count, 2);
    assert!(agents.iter().all(|item| item.get("worker_id").is_some()));

    let identities =
        fs::read_to_string(runtime_home.join("runtime/agents/control/identities.json"))
            .expect("agent control identities");
    assert!(identities.contains("system_agent"));
    assert!(identities.contains("system:mbp."));
}
