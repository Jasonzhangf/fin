use crate::{CliError, fs_utils::read_file, runtime_home::resolved_runtime_home};
use fin_config::{
    ConfigMapper, ProviderProfileImport, ProviderProfileImportOptions, SystemConfig,
    parse_system_toml, parse_user_toml, user_to_toml,
};
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
            let existing = parse_system_toml(&content)?;
            Ok(ConfigMapper::merge_user_layer(mapped, existing)?)
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

pub(crate) fn import_rcc_provider_profile(
    user_path: &Path,
    provider_json_path: &Path,
) -> Result<SystemConfig, CliError> {
    let user_toml = match fs::read_to_string(user_path) {
        Ok(content) => content,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            "default_provider = \"mini27\"\n".into()
        }
        Err(source) => {
            return Err(CliError::ReadFile {
                path: user_path.display().to_string(),
                source,
            });
        }
    };
    let mut user = parse_user_toml(&user_toml)?;
    let imported = ProviderProfileImport::from_rcc_file(
        provider_json_path,
        &ProviderProfileImportOptions::default(),
    )?;
    user.default_provider = imported.provider_name.clone();
    user.providers
        .insert(imported.provider_name.clone(), imported.provider);
    let system = ConfigMapper::map_user_to_system(&user)?;
    if let Some(parent) = user_path.parent() {
        fs::create_dir_all(parent).map_err(|source| CliError::WriteFile {
            path: parent.display().to_string(),
            source,
        })?;
    }
    fs::write(user_path, user_to_toml(&user)?).map_err(|source| CliError::WriteFile {
        path: user_path.display().to_string(),
        source,
    })?;
    Ok(system)
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

    #[test]
    fn import_rcc_provider_profile_preserves_existing_providers_and_selects_profile() {
        let runtime_home = temp_runtime_home("rcc-import");
        let user_path = runtime_home.join("config/user.toml");
        fs::write(&user_path, sample_user_toml()).expect("seed user toml");
        let provider_path = runtime_home.join("config/rcc.json");
        fs::write(
            &provider_path,
            r#"{
              "provider": {
                "type": "openai",
                "baseURL": "http://guizhouyun.site:2080",
                "models": { "MiniMax-M2.7": {}, "minimax": {} },
                "auth": { "type": "apikey", "apiKey": "sk-test-minimax" }
              }
            }"#,
        )
        .expect("write provider profile");

        let system = import_rcc_provider_profile(&user_path, &provider_path).expect("import rcc");
        let written = fs::read_to_string(&user_path).expect("read written user toml");
        let user = parse_user_toml(&written).expect("parse written user toml");

        assert_eq!(system.default_provider, "mini27");
        assert_eq!(
            system.default_provider_config().unwrap().model,
            "MiniMax-M2.7"
        );
        assert!(user.providers.contains_key("openai"));
        assert!(user.providers.contains_key("mini27"));
        assert_eq!(user.default_provider, "mini27");
        assert_eq!(
            user.providers["mini27"]
                .headers
                .get("authorization")
                .map(String::as_str),
            Some("Bearer sk-test-minimax")
        );
    }
}
