use fin_contracts::{ProviderPath, ProviderStrategy, ProviderTarget};
use fin_shared::require_non_empty;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("failed to parse {context} toml: {source}")]
    ParseToml {
        context: &'static str,
        #[source]
        source: toml::de::Error,
    },
    #[error("invalid config: {message}")]
    Validation { message: String },
    #[error(transparent)]
    Shared(#[from] fin_shared::SharedError),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProviderProtocol {
    OpenAiCompatible,
    AnthropicWire,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UserProviderConfig {
    pub protocol: ProviderProtocol,
    pub base_url: String,
    pub model: String,
    pub api_key: Option<String>,
    pub api_key_env: Option<String>,
    #[serde(default)]
    pub user_agent: Option<String>,
    #[serde(default)]
    pub headers: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UserConfig {
    pub default_provider: String,
    #[serde(default)]
    pub providers: BTreeMap<String, UserProviderConfig>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResolvedProviderConfig {
    pub name: String,
    pub protocol: ProviderProtocol,
    pub base_url: String,
    pub model: String,
    pub credential: ProviderCredential,
    pub user_agent: Option<String>,
    #[serde(default)]
    pub headers: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ProviderCredential {
    DirectApiKey { api_key: String },
    ApiKeyEnv { env_var: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeConfig {
    pub runtime_home: String,
    pub heartbeat_interval_ms: u64,
}

impl Default for RuntimeConfig {
    fn default() -> Self {
        Self {
            runtime_home: "~/.fin".into(),
            heartbeat_interval_ms: 5_000,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DebugConfig {
    pub retain_raw_events: bool,
    pub live_projection_limit: usize,
}

impl Default for DebugConfig {
    fn default() -> Self {
        Self {
            retain_raw_events: true,
            live_projection_limit: 1_000,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SystemConfig {
    pub default_provider: String,
    #[serde(default)]
    pub providers: BTreeMap<String, ResolvedProviderConfig>,
    #[serde(default)]
    pub policy: RuntimePolicyConfig,
    #[serde(default)]
    pub runtime: RuntimeConfig,
    #[serde(default)]
    pub debug: DebugConfig,
}

impl SystemConfig {
    pub fn default_provider_config(&self) -> Result<&ResolvedProviderConfig, ConfigError> {
        self.providers
            .get(&self.default_provider)
            .ok_or_else(|| ConfigError::Validation {
                message: format!(
                    "default provider '{}' missing from resolved providers",
                    self.default_provider
                ),
            })
    }

    pub fn role_profile(
        &self,
        role_id: Option<&str>,
    ) -> Result<(String, &RoleProfileConfig), ConfigError> {
        let role_id = role_id.unwrap_or(&self.policy.default_role);
        let profile = self
            .policy
            .roles
            .get(role_id)
            .ok_or_else(|| ConfigError::Validation {
                message: format!("runtime policy role '{}' is missing", role_id),
            })?;
        Ok((role_id.to_string(), profile))
    }

    pub fn validate(&self) -> Result<(), ConfigError> {
        require_non_empty("default_provider", &self.default_provider)?;
        if !self.providers.contains_key(&self.default_provider) {
            return Err(ConfigError::Validation {
                message: format!(
                    "default provider '{}' missing from resolved providers",
                    self.default_provider
                ),
            });
        }

        require_non_empty("policy.default_role", &self.policy.default_role)?;
        if !self.policy.roles.contains_key(&self.policy.default_role) {
            return Err(ConfigError::Validation {
                message: format!(
                    "runtime policy default_role '{}' missing from roles",
                    self.policy.default_role
                ),
            });
        }

        for (role_id, profile) in &self.policy.roles {
            require_non_empty("policy.role_id", role_id)?;
            profile.validate_with_providers(&self.providers)?;
        }

        Ok(())
    }
}

fn default_stream() -> bool {
    false
}

fn default_inference_timeout_ms() -> u64 {
    60_000
}

fn default_role_id() -> String {
    "default".into()
}

fn default_protocol_version() -> String {
    "fin.m1".into()
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderPathPolicy {
    #[serde(default = "default_provider_strategy")]
    pub strategy: ProviderStrategy,
    pub targets: Vec<ProviderTarget>,
}

impl ProviderPathPolicy {
    pub fn validate_with_providers(
        &self,
        providers: &BTreeMap<String, ResolvedProviderConfig>,
    ) -> Result<(), ConfigError> {
        if self.targets.is_empty() {
            return Err(ConfigError::Validation {
                message: "provider_path.targets must not be empty".into(),
            });
        }

        for target in &self.targets {
            require_non_empty("provider_path.target.provider_name", &target.provider_name)?;
            require_non_empty("provider_path.target.model", &target.model)?;

            let provider =
                providers
                    .get(&target.provider_name)
                    .ok_or_else(|| ConfigError::Validation {
                        message: format!(
                            "provider_path target '{}' is not present in providers",
                            target.provider_name
                        ),
                    })?;

            if provider.model != target.model {
                return Err(ConfigError::Validation {
                    message: format!(
                        "provider_path target '{}.{}' does not match resolved provider model '{}'",
                        target.provider_name, target.model, provider.model
                    ),
                });
            }
        }

        Ok(())
    }

    pub fn as_provider_path(&self) -> Result<ProviderPath, ConfigError> {
        Ok(ProviderPath::new(self.targets.clone())?)
    }
}

fn default_provider_strategy() -> ProviderStrategy {
    ProviderStrategy::Priority
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RoleProfileConfig {
    pub provider_path: ProviderPathPolicy,
    #[serde(default = "default_stream")]
    pub stream: bool,
    #[serde(default = "default_inference_timeout_ms")]
    pub timeout_ms: u64,
}

impl RoleProfileConfig {
    pub fn validate_with_providers(
        &self,
        providers: &BTreeMap<String, ResolvedProviderConfig>,
    ) -> Result<(), ConfigError> {
        self.provider_path.validate_with_providers(providers)?;
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimePolicyConfig {
    #[serde(default = "default_role_id")]
    pub default_role: String,
    #[serde(default = "default_protocol_version")]
    pub protocol_version: String,
    #[serde(default)]
    pub roles: BTreeMap<String, RoleProfileConfig>,
}

impl Default for RuntimePolicyConfig {
    fn default() -> Self {
        Self {
            default_role: default_role_id(),
            protocol_version: default_protocol_version(),
            roles: BTreeMap::new(),
        }
    }
}

pub struct ConfigMapper;

impl ConfigMapper {
    pub fn map_user_to_system(user: &UserConfig) -> Result<SystemConfig, ConfigError> {
        require_non_empty("default_provider", &user.default_provider)?;
        if !user.providers.contains_key(&user.default_provider) {
            return Err(ConfigError::Validation {
                message: format!(
                    "default_provider '{}' is not present in providers",
                    user.default_provider
                ),
            });
        }

        let mut providers = BTreeMap::new();
        for (name, provider) in &user.providers {
            require_non_empty("provider.name", name)?;
            require_non_empty("provider.base_url", &provider.base_url)?;
            require_non_empty("provider.model", &provider.model)?;
            if let Some(user_agent) = &provider.user_agent {
                require_non_empty("provider.user_agent", user_agent)?;
            }
            for (header_name, header_value) in &provider.headers {
                require_non_empty("provider.headers.name", header_name)?;
                require_non_empty("provider.headers.value", header_value)?;
            }

            let credential = match (&provider.api_key, &provider.api_key_env) {
                (Some(key), None) => {
                    require_non_empty("provider.api_key", key)?;
                    ProviderCredential::DirectApiKey {
                        api_key: key.clone(),
                    }
                }
                (None, Some(env_var)) => {
                    require_non_empty("provider.api_key_env", env_var)?;
                    ProviderCredential::ApiKeyEnv {
                        env_var: env_var.clone(),
                    }
                }
                (Some(_), Some(_)) => {
                    return Err(ConfigError::Validation {
                        message: format!(
                            "provider '{}' cannot set both api_key and api_key_env",
                            name
                        ),
                    });
                }
                (None, None) => {
                    return Err(ConfigError::Validation {
                        message: format!("provider '{}' must set api_key or api_key_env", name),
                    });
                }
            };

            providers.insert(
                name.clone(),
                ResolvedProviderConfig {
                    name: name.clone(),
                    protocol: provider.protocol,
                    base_url: provider.base_url.clone(),
                    model: provider.model.clone(),
                    credential,
                    user_agent: provider.user_agent.clone(),
                    headers: provider.headers.clone(),
                },
            );
        }

        let default_provider = user.default_provider.clone();
        let default_provider_model = providers
            .get(&default_provider)
            .expect("default provider should exist after validation")
            .model
            .clone();

        let system = SystemConfig {
            default_provider: user.default_provider.clone(),
            providers,
            policy: RuntimePolicyConfig {
                default_role: default_role_id(),
                protocol_version: default_protocol_version(),
                roles: BTreeMap::from([(
                    default_role_id(),
                    RoleProfileConfig {
                        provider_path: ProviderPathPolicy {
                            strategy: ProviderStrategy::Priority,
                            targets: vec![ProviderTarget::new(
                                default_provider,
                                default_provider_model,
                            )?],
                        },
                        stream: default_stream(),
                        timeout_ms: default_inference_timeout_ms(),
                    },
                )]),
            },
            runtime: RuntimeConfig::default(),
            debug: DebugConfig::default(),
        };
        system.validate()?;
        Ok(system)
    }
}

pub fn parse_user_toml(input: &str) -> Result<UserConfig, ConfigError> {
    toml::from_str(input).map_err(|source| ConfigError::ParseToml {
        context: "user",
        source,
    })
}

pub fn parse_system_toml(input: &str) -> Result<SystemConfig, ConfigError> {
    let system: SystemConfig = toml::from_str(input).map_err(|source| ConfigError::ParseToml {
        context: "system",
        source,
    })?;
    system.validate()?;
    Ok(system)
}

pub fn system_to_toml(system: &SystemConfig) -> Result<String, ConfigError> {
    toml::to_string_pretty(system).map_err(|err| ConfigError::Validation {
        message: format!("failed to serialize system config: {err}"),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn user_toml_with_comments_parses_and_maps() {
        let input = r#"
# user config

default_provider = "openai"

[providers.openai]
protocol = "open-ai-compatible"
base_url = "https://api.example.com/v1"
model = "gpt-5"
api_key_env = "OPENAI_API_KEY"
user_agent = "opencode/1.2.27"

[providers.openai.headers]
X-Client = "fin"
"#;

        let user = parse_user_toml(input).expect("user config should parse");
        let system = ConfigMapper::map_user_to_system(&user).expect("mapping should succeed");

        assert_eq!(system.default_provider, "openai");
        assert_eq!(
            system.providers["openai"].protocol,
            ProviderProtocol::OpenAiCompatible
        );
        assert_eq!(
            system.providers["openai"].user_agent.as_deref(),
            Some("opencode/1.2.27")
        );
        assert_eq!(system.providers["openai"].headers["X-Client"], "fin");
        assert_eq!(system.runtime.runtime_home, "~/.fin");
        assert_eq!(system.policy.default_role, "default");
        assert_eq!(
            system.policy.roles["default"].provider_path.targets[0].provider_name,
            "openai"
        );
    }

    #[test]
    fn mapper_rejects_ambiguous_credentials() {
        let user = UserConfig {
            default_provider: "anthropic".into(),
            providers: BTreeMap::from([(
                "anthropic".into(),
                UserProviderConfig {
                    protocol: ProviderProtocol::AnthropicWire,
                    base_url: "https://api.anthropic.com".into(),
                    model: "claude-sonnet".into(),
                    api_key: Some("x".into()),
                    api_key_env: Some("ANTHROPIC_API_KEY".into()),
                    user_agent: None,
                    headers: BTreeMap::new(),
                },
            )]),
        };

        let err = ConfigMapper::map_user_to_system(&user)
            .expect_err("should reject merge-like credentials");
        assert!(
            err.to_string()
                .contains("cannot set both api_key and api_key_env")
        );
    }

    #[test]
    fn system_config_round_trip_serializes() {
        let system = SystemConfig {
            default_provider: "openai".into(),
            providers: BTreeMap::from([(
                "openai".into(),
                ResolvedProviderConfig {
                    name: "openai".into(),
                    protocol: ProviderProtocol::OpenAiCompatible,
                    base_url: "https://api.example.com/v1".into(),
                    model: "gpt-5".into(),
                    credential: ProviderCredential::ApiKeyEnv {
                        env_var: "OPENAI_API_KEY".into(),
                    },
                    user_agent: Some("opencode/1.2.27".into()),
                    headers: BTreeMap::from([("X-Client".into(), "fin".into())]),
                },
            )]),
            policy: RuntimePolicyConfig {
                default_role: "default".into(),
                protocol_version: "fin.m1".into(),
                roles: BTreeMap::from([(
                    "default".into(),
                    RoleProfileConfig {
                        provider_path: ProviderPathPolicy {
                            strategy: ProviderStrategy::Priority,
                            targets: vec![
                                ProviderTarget::new("openai", "gpt-5")
                                    .expect("provider target should be valid"),
                            ],
                        },
                        stream: false,
                        timeout_ms: 60_000,
                    },
                )]),
            },
            runtime: RuntimeConfig::default(),
            debug: DebugConfig::default(),
        };

        let toml = system_to_toml(&system).expect("system config should serialize");
        let reparsed = parse_system_toml(&toml).expect("system config should parse");
        assert_eq!(reparsed.default_provider, "openai");
        assert_eq!(reparsed.providers["openai"].model, "gpt-5");
        assert_eq!(
            reparsed.providers["openai"].user_agent.as_deref(),
            Some("opencode/1.2.27")
        );
        assert_eq!(reparsed.providers["openai"].headers["X-Client"], "fin");
        assert_eq!(reparsed.policy.protocol_version, "fin.m1");
    }

    #[test]
    fn parse_system_toml_rejects_role_with_unknown_provider_target() {
        let input = r#"
default_provider = "openai"

[providers.openai]
name = "openai"
protocol = "open-ai-compatible"
base_url = "https://api.example.com/v1"
model = "gpt-5"

[providers.openai.credential]
kind = "api_key_env"
env_var = "OPENAI_API_KEY"

[policy]
default_role = "default"
protocol_version = "fin.m1"

[policy.roles.default]
stream = false
timeout_ms = 60000

[policy.roles.default.provider_path]
strategy = "priority"

[[policy.roles.default.provider_path.targets]]
provider_name = "missing"
model = "gpt-5"
"#;

        let err = parse_system_toml(input).expect_err("unknown provider target must fail");
        assert!(
            err.to_string()
                .contains("provider_path target 'missing' is not present in providers")
        );
    }
}
