use fin_config::{ProviderCredential, ProviderProtocol, ResolvedProviderConfig};
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use thiserror::Error;

const DEFAULT_USER_AGENT: &str = "fin-coding-agent/0.1";

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ProviderError {
    #[error("duplicate provider '{name}'")]
    DuplicateProvider { name: String },
    #[error("unsupported protocol for real execution: {protocol:?}")]
    UnsupportedProtocol { protocol: ProviderProtocol },
    #[error("missing provider credential env '{env_var}'")]
    MissingCredentialEnv { env_var: String },
    #[error("http status {status}: {body}")]
    HttpStatus { status: u16, body: String },
    #[error("request failed: {message}")]
    Request { message: String },
    #[error("response parse failed: {message}")]
    ParseResponse { message: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderCapabilities {
    pub supports_streaming: bool,
    pub supports_tool_calls: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderDescriptor {
    pub name: String,
    pub protocol: ProviderProtocol,
    pub base_url: String,
    pub default_model: String,
    pub capabilities: ProviderCapabilities,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderRequest {
    pub input: String,
    pub override_model: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PreparedRequest {
    pub provider_name: String,
    pub protocol: ProviderProtocol,
    pub endpoint: String,
    pub model: String,
    pub input: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderResponse {
    pub provider_name: String,
    pub model: String,
    pub output_text: String,
    pub response_id: Option<String>,
    pub stop_reason: Option<String>,
    pub status: u16,
}

impl ProviderDescriptor {
    pub fn from_resolved(config: &ResolvedProviderConfig) -> Self {
        Self {
            name: config.name.clone(),
            protocol: config.protocol,
            base_url: config.base_url.clone(),
            default_model: config.model.clone(),
            capabilities: ProviderCapabilities::for_protocol(config.protocol),
        }
    }

    pub fn prepare_request(&self, request: &ProviderRequest) -> PreparedRequest {
        PreparedRequest {
            provider_name: self.name.clone(),
            protocol: self.protocol,
            endpoint: endpoint_for_protocol(&self.base_url, self.protocol),
            model: request
                .override_model
                .clone()
                .unwrap_or_else(|| self.default_model.clone()),
            input: request.input.clone(),
        }
    }
}

impl ProviderCapabilities {
    pub fn for_protocol(protocol: ProviderProtocol) -> Self {
        match protocol {
            ProviderProtocol::OpenAiCompatible => Self {
                supports_streaming: true,
                supports_tool_calls: true,
            },
            ProviderProtocol::AnthropicWire => Self {
                supports_streaming: true,
                supports_tool_calls: false,
            },
        }
    }
}

fn endpoint_for_protocol(base_url: &str, protocol: ProviderProtocol) -> String {
    let base = base_url.trim_end_matches('/');
    match protocol {
        ProviderProtocol::AnthropicWire => format!("{base}/v1/messages"),
        ProviderProtocol::OpenAiCompatible => format!("{base}/chat/completions"),
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ProviderRegistry {
    providers: BTreeMap<String, ProviderDescriptor>,
}

impl ProviderRegistry {
    pub fn register(&mut self, descriptor: ProviderDescriptor) -> Result<(), ProviderError> {
        let name = descriptor.name.clone();
        if self.providers.contains_key(&name) {
            return Err(ProviderError::DuplicateProvider { name });
        }
        self.providers.insert(descriptor.name.clone(), descriptor);
        Ok(())
    }

    pub fn register_resolved(
        &mut self,
        config: &ResolvedProviderConfig,
    ) -> Result<(), ProviderError> {
        self.register(ProviderDescriptor::from_resolved(config))
    }

    pub fn get(&self, name: &str) -> Option<&ProviderDescriptor> {
        self.providers.get(name)
    }

    pub fn len(&self) -> usize {
        self.providers.len()
    }

    pub fn is_empty(&self) -> bool {
        self.providers.is_empty()
    }
}

pub trait InferenceProvider {
    fn descriptor(&self) -> &ProviderDescriptor;

    fn protocol(&self) -> ProviderProtocol {
        self.descriptor().protocol
    }

    fn prepare_request(&self, request: &ProviderRequest) -> PreparedRequest {
        self.descriptor().prepare_request(request)
    }

    fn execute_prepared(
        &self,
        request: &PreparedRequest,
    ) -> Result<ProviderResponse, ProviderError>;
}

#[derive(Debug, Clone)]
pub struct StaticProviderClient {
    descriptor: ProviderDescriptor,
}

impl StaticProviderClient {
    pub fn new(descriptor: ProviderDescriptor) -> Self {
        Self { descriptor }
    }
}

impl InferenceProvider for StaticProviderClient {
    fn descriptor(&self) -> &ProviderDescriptor {
        &self.descriptor
    }

    fn execute_prepared(
        &self,
        request: &PreparedRequest,
    ) -> Result<ProviderResponse, ProviderError> {
        Ok(ProviderResponse {
            provider_name: request.provider_name.clone(),
            model: request.model.clone(),
            output_text: format!("simulated response for {}", request.input),
            response_id: Some("simulated-response".into()),
            stop_reason: Some("end_turn".into()),
            status: 200,
        })
    }
}

#[derive(Debug, Clone)]
pub struct ProviderFacade {
    descriptor: ProviderDescriptor,
    credential: ProviderCredential,
}

impl ProviderFacade {
    pub fn from_resolved(config: &ResolvedProviderConfig) -> Self {
        Self {
            descriptor: ProviderDescriptor::from_resolved(config),
            credential: config.credential.clone(),
        }
    }

    fn resolve_api_key(&self) -> Result<String, ProviderError> {
        match &self.credential {
            ProviderCredential::DirectApiKey { api_key } => Ok(api_key.clone()),
            ProviderCredential::ApiKeyEnv { env_var } => {
                std::env::var(env_var).map_err(|_| ProviderError::MissingCredentialEnv {
                    env_var: env_var.clone(),
                })
            }
        }
    }

    fn execute_anthropic(
        &self,
        request: &PreparedRequest,
    ) -> Result<ProviderResponse, ProviderError> {
        let client = Client::new();
        let payload = serde_json::json!({
            "model": request.model,
            "max_tokens": 256,
            "messages": [
                {
                    "role": "user",
                    "content": request.input,
                }
            ]
        });
        let response = client
            .post(&request.endpoint)
            .header("content-type", "application/json")
            .header("accept", "application/json")
            .header("x-api-key", self.resolve_api_key()?)
            .header("anthropic-version", "2023-06-01")
            .header("user-agent", DEFAULT_USER_AGENT)
            .json(&payload)
            .send()
            .map_err(|err| ProviderError::Request {
                message: err.to_string(),
            })?;

        let status = response.status().as_u16();
        let body = response.text().map_err(|err| ProviderError::Request {
            message: err.to_string(),
        })?;

        if status >= 400 {
            return Err(ProviderError::HttpStatus { status, body });
        }

        let parsed: Value =
            serde_json::from_str(&body).map_err(|err| ProviderError::ParseResponse {
                message: err.to_string(),
            })?;
        let output_text = parsed
            .get("content")
            .and_then(Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .filter_map(|item| {
                        item.get("type")
                            .and_then(Value::as_str)
                            .filter(|kind| *kind == "text")
                            .and_then(|_| item.get("text"))
                            .and_then(Value::as_str)
                    })
                    .collect::<String>()
            })
            .unwrap_or_default();

        Ok(ProviderResponse {
            provider_name: request.provider_name.clone(),
            model: request.model.clone(),
            output_text,
            response_id: parsed.get("id").and_then(Value::as_str).map(str::to_string),
            stop_reason: parsed
                .get("stop_reason")
                .and_then(Value::as_str)
                .map(str::to_string),
            status,
        })
    }
}

impl InferenceProvider for ProviderFacade {
    fn descriptor(&self) -> &ProviderDescriptor {
        &self.descriptor
    }

    fn execute_prepared(
        &self,
        request: &PreparedRequest,
    ) -> Result<ProviderResponse, ProviderError> {
        match self.descriptor.protocol {
            ProviderProtocol::AnthropicWire => self.execute_anthropic(request),
            protocol => Err(ProviderError::UnsupportedProtocol { protocol }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fin_config::{ProviderCredential, ResolvedProviderConfig};

    fn openai_config() -> ResolvedProviderConfig {
        ResolvedProviderConfig {
            name: "openai".into(),
            protocol: ProviderProtocol::OpenAiCompatible,
            base_url: "https://api.example.com/v1".into(),
            model: "gpt-5".into(),
            credential: ProviderCredential::ApiKeyEnv {
                env_var: "OPENAI_API_KEY".into(),
            },
        }
    }

    #[test]
    fn registry_registers_resolved_provider() {
        let mut registry = ProviderRegistry::default();
        registry
            .register_resolved(&openai_config())
            .expect("register should succeed");

        let descriptor = registry.get("openai").expect("provider should exist");
        assert_eq!(descriptor.default_model, "gpt-5");
        assert!(descriptor.capabilities.supports_tool_calls);
    }

    #[test]
    fn duplicate_provider_is_rejected() {
        let mut registry = ProviderRegistry::default();
        let config = openai_config();
        registry
            .register_resolved(&config)
            .expect("first register should succeed");
        let err = registry
            .register_resolved(&config)
            .expect_err("duplicate must fail");
        assert_eq!(
            err,
            ProviderError::DuplicateProvider {
                name: "openai".into()
            }
        );
    }

    #[test]
    fn client_prepares_request_from_descriptor() {
        let descriptor = ProviderDescriptor::from_resolved(&openai_config());
        let client = StaticProviderClient::new(descriptor);
        let prepared = client.prepare_request(&ProviderRequest {
            input: "hello".into(),
            override_model: None,
        });

        assert_eq!(client.protocol(), ProviderProtocol::OpenAiCompatible);
        assert_eq!(prepared.model, "gpt-5");
        assert_eq!(prepared.provider_name, "openai");
        assert_eq!(
            prepared.endpoint,
            "https://api.example.com/v1/chat/completions"
        );
    }

    #[test]
    fn anthropic_descriptor_prepares_messages_endpoint() {
        let config = ResolvedProviderConfig {
            name: "ali-coding-plan".into(),
            protocol: ProviderProtocol::AnthropicWire,
            base_url: "https://coding.dashscope.aliyuncs.com/apps/anthropic".into(),
            model: "qwen3.6-plus".into(),
            credential: ProviderCredential::ApiKeyEnv {
                env_var: "ALI_CODINGPLAN_KEY".into(),
            },
        };
        let facade = ProviderFacade::from_resolved(&config);
        let prepared = facade.prepare_request(&ProviderRequest {
            input: "hello".into(),
            override_model: None,
        });
        assert_eq!(
            prepared.endpoint,
            "https://coding.dashscope.aliyuncs.com/apps/anthropic/v1/messages"
        );
    }
}
