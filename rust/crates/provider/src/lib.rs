use fin_config::{ProviderProtocol, ResolvedProviderConfig};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ProviderError {
    #[error("duplicate provider '{name}'")]
    DuplicateProvider { name: String },
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
            endpoint: self.base_url.clone(),
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

pub trait ProviderClient {
    fn descriptor(&self) -> &ProviderDescriptor;
    fn protocol(&self) -> ProviderProtocol {
        self.descriptor().protocol
    }
    fn prepare_request(&self, request: &ProviderRequest) -> PreparedRequest {
        self.descriptor().prepare_request(request)
    }
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

impl ProviderClient for StaticProviderClient {
    fn descriptor(&self) -> &ProviderDescriptor {
        &self.descriptor
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
    }
}
