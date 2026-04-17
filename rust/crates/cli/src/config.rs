use crate::{CliError, fs_utils::read_file};
use fin_config::{ConfigMapper, SystemConfig, parse_user_toml};
use fin_provider::{ProviderFacade, ProviderRegistry};
use std::path::Path;

pub(crate) fn map_system_config(user_toml: &str) -> Result<SystemConfig, CliError> {
    let user = parse_user_toml(user_toml)?;
    Ok(ConfigMapper::map_user_to_system(&user)?)
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
