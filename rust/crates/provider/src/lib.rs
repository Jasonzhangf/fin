use fin_config::{ProviderCredential, ProviderProtocol, ResolvedProviderConfig};
use reqwest::header::{ACCEPT, CONTENT_TYPE, HeaderMap, HeaderName, HeaderValue, USER_AGENT};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use thiserror::Error;

mod http_client;
mod hub_pipeline;
#[cfg(test)]
mod hub_pipeline_static_tests;
mod provider_facade;
mod provider_static;

const DEFAULT_USER_AGENT: &str = "fin-coding-agent/0.1";
const MAX_REQUEST_ATTEMPTS: usize = 5;
const ANTHROPIC_MAX_OUTPUT_TOKENS: u64 = 2048;
const OPENAI_MAX_OUTPUT_TOKENS: u64 = 8192;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ProviderError {
    #[error("duplicate provider '{name}'")]
    DuplicateProvider { name: String },
    #[error("unsupported protocol for real execution: {protocol:?}")]
    UnsupportedProtocol { protocol: ProviderProtocol },
    #[error("missing provider credential env '{env_var}'")]
    MissingCredentialEnv { env_var: String },
    #[error("invalid header '{name}': {message}")]
    InvalidHeader { name: String, message: String },
    #[error("http status {status}: {body}")]
    HttpStatus { status: u16, body: String },
    #[error("request failed: {message}")]
    Request { message: String },
    #[error("response parse failed: {message}")]
    ParseResponse { message: String },
    #[error("invalid resolve override '{entry}': {message}")]
    InvalidResolveOverride { entry: String, message: String },
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
    pub rendered_input: Option<String>,
    pub override_model: Option<String>,
    #[serde(default)]
    pub tools: Vec<ProviderToolSpec>,
    #[serde(default)]
    pub prior_tool_calls: Vec<ProviderToolCall>,
    #[serde(default)]
    pub tool_results: Vec<ProviderToolResult>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PreparedRequest {
    pub provider_name: String,
    pub protocol: ProviderProtocol,
    pub endpoint: String,
    pub model: String,
    pub input: String,
    pub rendered_input: String,
    pub user_agent: Option<String>,
    pub sanitized_headers: BTreeMap<String, String>,
    #[serde(default)]
    pub tools: Vec<ProviderToolSpec>,
    #[serde(default)]
    pub prior_tool_calls: Vec<ProviderToolCall>,
    #[serde(default)]
    pub tool_results: Vec<ProviderToolResult>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderResponse {
    pub provider_name: String,
    pub model: String,
    pub output_text: String,
    pub response_id: Option<String>,
    pub stop_reason: Option<String>,
    pub status: u16,
    #[serde(default)]
    pub tool_calls: Vec<ProviderToolCall>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderToolSpec {
    pub name: String,
    pub description: String,
    pub input_schema: Value,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderToolCall {
    pub tool_call_id: String,
    pub name: String,
    pub arguments: Value,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderToolResult {
    pub tool_call_id: String,
    pub name: String,
    pub content: String,
    #[serde(default)]
    pub is_error: bool,
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
            rendered_input: request
                .rendered_input
                .clone()
                .unwrap_or_else(|| request.input.clone()),
            user_agent: None,
            sanitized_headers: BTreeMap::new(),
            tools: request.tools.clone(),
            prior_tool_calls: request.prior_tool_calls.clone(),
            tool_results: request.tool_results.clone(),
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

// ProviderFacade, StaticProviderClient, and StructuredStaticProviderClient
// implementations are in their own modules: provider_facade.rs and provider_static.rs

pub use provider_static::{StaticProviderClient, StructuredStaticProviderClient};

#[derive(Debug, Clone)]
pub struct ProviderFacade {
    descriptor: ProviderDescriptor,
    credential: ProviderCredential,
    user_agent: Option<String>,
    headers: BTreeMap<String, String>,
    resolve_overrides: BTreeMap<String, std::net::IpAddr>,
    resolve_override_error: Option<String>,
}

#[cfg(test)]
mod tests;
