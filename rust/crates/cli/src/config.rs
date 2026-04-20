use crate::{CliError, fs_utils::read_file, runtime_home::resolved_runtime_home};
use fin_config::{ConfigMapper, SystemConfig, parse_system_toml, parse_user_toml};
use fin_provider::{ProviderFacade, ProviderRegistry};
use std::{fs, path::Path};

pub(crate) fn map_system_config(user_toml: &str) -> Result<SystemConfig, CliError> {
    let user = parse_user_toml(user_toml)?;
    Ok(ConfigMapper::map_user_to_system(&user)?)
}

pub(crate) fn load_effective_system_config(
    user_toml: &str,
    runtime_home_override: Option<&Path>,
) -> Result<SystemConfig, CliError> {
    let mapped = map_system_config(user_toml)?;
    let runtime_home = resolved_runtime_home(&mapped, runtime_home_override);
    let system_path = runtime_home.join("config/system.toml");
    match fs::read_to_string(&system_path) {
        Ok(content) => {
            let mut existing = parse_system_toml(&content)?;
            existing.default_provider = mapped.default_provider;
            existing.providers = mapped.providers;
            existing.runtime.runtime_home = mapped.runtime.runtime_home;
            existing.runtime.device_name = mapped.runtime.device_name;
            existing.validate()?;
            Ok(existing)
        }
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(mapped),
        Err(source) => Err(CliError::ReadFile {
            path: system_path.display().to_string(),
            source,
        }),
    }
}

pub(crate) fn load_system_config(path: &Path) -> Result<SystemConfig, CliError> {
    let content = read_file(path)?;
    map_system_config(&content)
}

pub(crate) fn default_provider_facade(system: &SystemConfig) -> Result<ProviderFacade, CliError> {
    let mut registry = ProviderRegistry::default();
    for provider in system.providers.values() {
        registry.register_resolved(provider)?;
    }
    let provider = system.default_provider_config()?;
    let _ = registry
        .get(&provider.name)
        .expect("default provider should be registered");
    Ok(ProviderFacade::from_resolved(provider))
}

#[cfg(test)]
mod tests {
    use super::*;
    use fin_config::ProjectAgentMode;
    use std::{
        path::PathBuf,
        time::{SystemTime, UNIX_EPOCH},
    };

    fn sample_user_toml() -> &'static str {
        r#"
default_provider = "openai"

[runtime]
device_name = "mac-studio"

[providers.openai]
protocol = "open-ai-compatible"
base_url = "https://api.example.com/v1"
model = "gpt-5"
api_key_env = "OPENAI_API_KEY"
"#
    }

    fn temp_runtime_home(prefix: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time")
            .as_nanos();
        let path = std::env::temp_dir().join(format!("fin-config-{prefix}-{unique}"));
        fs::create_dir_all(path.join("config")).expect("runtime home config dir");
        path
    }

    #[test]
    fn load_effective_system_config_prefers_system_startup_overrides() {
        let runtime_home = temp_runtime_home("system-override");
        let mut system = map_system_config(sample_user_toml()).expect("mapped system");
        system.runtime.startup.system_agent.local_worker_budget = 7;
        system.runtime.startup.system_agent.auto_resume = false;
        system.runtime.startup.project_agents = vec![fin_config::ProjectAgentStartupConfig {
            project_id: "fin".into(),
            mode: ProjectAgentMode::Local,
            project_root: Some("/tmp/fin".into()),
            endpoint: None,
            agent_name: Some("builder".into()),
            worker_budget: 9,
            always_on: true,
            auto_resume: false,
            auto_connect: false,
        }];
        fs::write(
            runtime_home.join("config/system.toml"),
            fin_config::system_to_toml(&system).expect("system toml"),
        )
        .expect("write system.toml");

        let effective = load_effective_system_config(sample_user_toml(), Some(&runtime_home))
            .expect("effective system config");

        assert_eq!(
            effective.runtime.startup.system_agent.local_worker_budget,
            7
        );
        assert!(!effective.runtime.startup.system_agent.auto_resume);
        assert_eq!(effective.runtime.startup.project_agents.len(), 1);
        assert_eq!(effective.runtime.startup.project_agents[0].worker_budget, 9);
        assert!(!effective.runtime.startup.project_agents[0].auto_resume);
        assert!(!effective.runtime.startup.project_agents[0].auto_connect);
        assert_eq!(
            effective.runtime.startup.project_agents[0].mode,
            ProjectAgentMode::Local
        );
        assert_eq!(
            effective.runtime.device_name.as_deref(),
            Some("mac-studio"),
            "user layer still owns runtime device identity",
        );
        assert_eq!(effective.default_provider, "openai");
        assert!(effective.providers.contains_key("openai"));
    }

    #[test]
    fn load_effective_system_config_uses_mapped_defaults_without_system_toml() {
        let runtime_home = temp_runtime_home("mapped-default");
        let effective = load_effective_system_config(sample_user_toml(), Some(&runtime_home))
            .expect("effective system config");

        assert_eq!(
            effective.runtime.startup.system_agent.local_worker_budget,
            4
        );
        assert!(effective.runtime.startup.system_agent.auto_resume);
        assert_eq!(effective.runtime.startup.project_agents.len(), 0);
        assert_eq!(effective.runtime.device_name.as_deref(), Some("mac-studio"));
    }
}
