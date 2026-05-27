use crate::{ConfigError, ProviderProtocol, UserProviderConfig};
use serde::Deserialize;
use std::{collections::BTreeMap, fs, path::Path};

const MODEL_PREFERENCE: &[&str] = &[
    "MiniMax-M2.7",
    "minimax",
    "qwen3.6-plus",
    "qwen3-coder-plus",
    "qwen3-coder-next",
    "qwen3.5-plus",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderProfileImportOptions {
    pub provider_name: String,
    pub model: Option<String>,
    pub user_agent: Option<String>,
}

impl Default for ProviderProfileImportOptions {
    fn default() -> Self {
        Self {
            provider_name: "mini27".into(),
            model: None,
            user_agent: Some("opencode/1.2.27".into()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderProfileImport {
    pub provider_name: String,
    pub provider: UserProviderConfig,
}

impl ProviderProfileImport {
    pub fn from_rcc_file(
        path: &Path,
        options: &ProviderProfileImportOptions,
    ) -> Result<Self, ConfigError> {
        match path.extension().and_then(|value| value.to_str()) {
            Some("toml") => Self::from_rcc_toml_file(path, options),
            _ => Self::from_rcc_json_file(path, options),
        }
    }

    pub fn from_rcc_json_file(
        path: &Path,
        options: &ProviderProfileImportOptions,
    ) -> Result<Self, ConfigError> {
        let content = fs::read_to_string(path).map_err(|source| ConfigError::Validation {
            message: format!(
                "provider profile '{}' cannot be read: {source}",
                path.display()
            ),
        })?;
        Self::from_rcc_json_str(&content, options)
    }

    pub fn from_rcc_toml_file(
        path: &Path,
        options: &ProviderProfileImportOptions,
    ) -> Result<Self, ConfigError> {
        let content = fs::read_to_string(path).map_err(|source| ConfigError::Validation {
            message: format!(
                "provider profile '{}' cannot be read: {source}",
                path.display()
            ),
        })?;
        Self::from_rcc_toml_str(&content, options)
    }

    pub fn from_rcc_json_str(
        input: &str,
        options: &ProviderProfileImportOptions,
    ) -> Result<Self, ConfigError> {
        let root: RccRoot =
            serde_json::from_str(input).map_err(|source| ConfigError::Validation {
                message: format!("provider profile json is invalid: {source}"),
            })?;
        Self::from_rcc_provider(root.provider, options)
    }

    pub fn from_rcc_toml_str(
        input: &str,
        options: &ProviderProfileImportOptions,
    ) -> Result<Self, ConfigError> {
        let root: RccRoot = toml::from_str(input).map_err(|source| ConfigError::Validation {
            message: format!("provider profile toml is invalid: {source}"),
        })?;
        Self::from_rcc_provider(root.provider, options)
    }

    fn from_rcc_provider(
        provider: RccProvider,
        options: &ProviderProfileImportOptions,
    ) -> Result<Self, ConfigError> {
        let provider_name = require_value("provider_name", &options.provider_name)?.to_string();
        let protocol =
            protocol_from_type(require_value("provider.type", &provider.provider_type)?)?;
        let base_url = require_value("provider.baseURL", &provider.base_url)?.to_string();
        let auth_type = require_value("provider.auth.type", &provider.auth.auth_type)?;
        let api_key = require_value("provider.auth.apiKey", &provider.auth.api_key)?;
        let model = choose_model(&provider.models, options.model.as_deref())?;
        let (api_key, api_key_env) = parse_api_key_spec(api_key)?;
        let headers = auth_headers(auth_type, api_key.as_deref(), api_key_env.as_deref())?;

        Ok(Self {
            provider_name,
            provider: UserProviderConfig {
                protocol,
                base_url,
                model,
                api_key,
                api_key_env,
                user_agent: options.user_agent.clone(),
                headers,
            },
        })
    }
}

#[derive(Debug, Deserialize)]
struct RccRoot {
    provider: RccProvider,
}

#[derive(Debug, Deserialize)]
struct RccProvider {
    #[serde(rename = "type")]
    provider_type: String,
    #[serde(rename = "baseURL")]
    base_url: String,
    #[serde(default)]
    models: BTreeMap<String, serde_json::Value>,
    auth: RccAuth,
}

#[derive(Debug, Deserialize)]
struct RccAuth {
    #[serde(rename = "type", default)]
    auth_type: String,
    #[serde(rename = "apiKey")]
    api_key: String,
}

fn protocol_from_type(provider_type: &str) -> Result<ProviderProtocol, ConfigError> {
    match provider_type {
        "anthropic" => Ok(ProviderProtocol::AnthropicWire),
        "openai" | "openai-compatible" => Ok(ProviderProtocol::OpenAiCompatible),
        other => Err(ConfigError::Validation {
            message: format!("unsupported provider profile type '{other}'"),
        }),
    }
}

fn choose_model(
    models: &BTreeMap<String, serde_json::Value>,
    requested: Option<&str>,
) -> Result<String, ConfigError> {
    if let Some(requested) = requested {
        let requested = require_value("provider model", requested)?;
        if models.contains_key(requested) {
            return Ok(requested.to_string());
        }
        return Err(ConfigError::Validation {
            message: format!("requested provider model '{requested}' is not present in profile"),
        });
    }
    for candidate in MODEL_PREFERENCE {
        if models.contains_key(*candidate) {
            return Ok((*candidate).to_string());
        }
    }
    models
        .keys()
        .next()
        .cloned()
        .ok_or_else(|| ConfigError::Validation {
            message: "provider profile contains no models".into(),
        })
}

fn parse_api_key_spec(raw_api_key: &str) -> Result<(Option<String>, Option<String>), ConfigError> {
    let value = require_value("provider.auth.apiKey", raw_api_key)?;
    if value.starts_with("${") && value.ends_with('}') && value.len() > 3 {
        Ok((None, Some(value[2..value.len() - 1].to_string())))
    } else {
        Ok((Some(value.to_string()), None))
    }
}

fn auth_headers(
    auth_type: &str,
    api_key: Option<&str>,
    api_key_env: Option<&str>,
) -> Result<BTreeMap<String, String>, ConfigError> {
    let mut headers = BTreeMap::new();
    if auth_type == "apikey" {
        let token = if let Some(env_var) = api_key_env {
            format!("Bearer ${{{env_var}}}")
        } else if let Some(api_key) = api_key {
            format!("Bearer {api_key}")
        } else {
            return Err(ConfigError::Validation {
                message: "provider.auth.apiKey must not be empty".into(),
            });
        };
        headers.insert("authorization".into(), token);
    }
    Ok(headers)
}

fn require_value<'a>(field: &str, value: &'a str) -> Result<&'a str, ConfigError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        Err(ConfigError::Validation {
            message: format!("{field} must not be empty"),
        })
    } else {
        Ok(trimmed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_profile() -> &'static str {
        r#"{
          "provider": {
            "type": "anthropic",
        "baseURL": "https://coding.dashscope.aliyuncs.com/apps/anthropic/v1",
            "models": {
              "qwen3.5-plus": {},
              "qwen3.6-plus": {},
              "qwen3-coder-plus": {}
            },
            "auth": { "type": "bearer", "apiKey": "${DASHSCOPE_API_KEY}" }
          }
        }"#
    }

    fn sample_toml_profile() -> &'static str {
        r#"
version = "2.0.0"
providerId = "mini27"

[provider]
id = "mini27"
enabled = true
type = "openai"
baseURL = "http://guizhouyun.site:2080"

[provider.auth]
type = "apikey"
apiKey = "sk-test"

[provider.models."MiniMax-M2.7"]
supportsStreaming = true
supportsThinking = true
"#
    }

    #[test]
    fn imports_rcc_profile_as_user_provider_config() {
        let imported = ProviderProfileImport::from_rcc_json_str(
            sample_profile(),
            &ProviderProfileImportOptions::default(),
        )
        .expect("import profile");

        assert_eq!(imported.provider_name, "mini27");
        assert_eq!(imported.provider.protocol, ProviderProtocol::AnthropicWire);
        assert_eq!(imported.provider.model, "qwen3.6-plus");
        assert_eq!(
            imported.provider.api_key_env.as_deref(),
            Some("DASHSCOPE_API_KEY")
        );
        assert!(imported.provider.api_key.is_none());
    }

    #[test]
    fn imports_rcc_toml_profile_as_user_provider_config() {
        let imported = ProviderProfileImport::from_rcc_toml_str(
            sample_toml_profile(),
            &ProviderProfileImportOptions::default(),
        )
        .expect("import toml profile");

        assert_eq!(imported.provider_name, "mini27");
        assert_eq!(
            imported.provider.protocol,
            ProviderProtocol::OpenAiCompatible
        );
        assert_eq!(imported.provider.base_url, "http://guizhouyun.site:2080");
        assert_eq!(imported.provider.model, "MiniMax-M2.7");
        assert_eq!(imported.provider.api_key.as_deref(), Some("sk-test"));
    }

    #[test]
    fn rejects_requested_model_missing_from_profile() {
        let err = ProviderProfileImport::from_rcc_json_str(
            sample_profile(),
            &ProviderProfileImportOptions {
                model: Some("missing-model".into()),
                ..ProviderProfileImportOptions::default()
            },
        )
        .expect_err("missing model rejected");

        assert!(
            err.to_string()
                .contains("requested provider model 'missing-model'")
        );
    }
}
