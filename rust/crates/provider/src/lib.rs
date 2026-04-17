use fin_config::{ProviderCredential, ProviderProtocol, ResolvedProviderConfig};
use reqwest::blocking::Client;
use reqwest::header::{ACCEPT, CONTENT_TYPE, HeaderMap, HeaderName, HeaderValue, USER_AGENT};
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
    #[error("invalid header '{name}': {message}")]
    InvalidHeader { name: String, message: String },
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
    pub rendered_input: Option<String>,
    pub override_model: Option<String>,
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
            rendered_input: request
                .rendered_input
                .clone()
                .unwrap_or_else(|| request.input.clone()),
            user_agent: None,
            sanitized_headers: BTreeMap::new(),
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
    user_agent: Option<String>,
    headers: BTreeMap<String, String>,
}

impl ProviderFacade {
    pub fn from_resolved(config: &ResolvedProviderConfig) -> Self {
        Self {
            descriptor: ProviderDescriptor::from_resolved(config),
            credential: config.credential.clone(),
            user_agent: config.user_agent.clone(),
            headers: config.headers.clone(),
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
        let api_key = self.resolve_api_key()?;
        let payload = serde_json::json!({
            "model": request.model,
            "max_tokens": 256,
            "messages": [
                {
                    "role": "user",
                    "content": request.rendered_input,
                }
            ]
        });
        let response = client
            .post(&request.endpoint)
            .headers(self.build_anthropic_headers(&api_key)?)
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

    fn build_anthropic_headers(&self, api_key: &str) -> Result<HeaderMap, ProviderError> {
        let mut headers = self.build_custom_headers()?;
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        headers.insert(ACCEPT, HeaderValue::from_static("application/json"));
        headers.insert(
            HeaderName::from_static("x-api-key"),
            HeaderValue::from_str(api_key).map_err(|err| ProviderError::InvalidHeader {
                name: "x-api-key".into(),
                message: err.to_string(),
            })?,
        );
        headers.insert(
            HeaderName::from_static("anthropic-version"),
            HeaderValue::from_static("2023-06-01"),
        );
        headers.insert(
            USER_AGENT,
            HeaderValue::from_str(self.effective_user_agent()).map_err(|err| {
                ProviderError::InvalidHeader {
                    name: "user-agent".into(),
                    message: err.to_string(),
                }
            })?,
        );
        Ok(headers)
    }

    fn build_custom_headers(&self) -> Result<HeaderMap, ProviderError> {
        let mut headers = HeaderMap::new();
        for (name, value) in &self.headers {
            let header_name = HeaderName::from_bytes(name.as_bytes()).map_err(|err| {
                ProviderError::InvalidHeader {
                    name: name.clone(),
                    message: err.to_string(),
                }
            })?;
            let header_value =
                HeaderValue::from_str(value).map_err(|err| ProviderError::InvalidHeader {
                    name: name.clone(),
                    message: err.to_string(),
                })?;
            headers.insert(header_name, header_value);
        }
        Ok(headers)
    }

    fn build_sanitized_request_headers(&self) -> BTreeMap<String, String> {
        let mut headers = BTreeMap::new();
        for (name, value) in &self.headers {
            if is_reserved_runtime_header(name) {
                continue;
            }
            headers.insert(name.clone(), sanitize_header_value(name, value));
        }
        headers.insert("user-agent".into(), self.effective_user_agent().into());
        if self.descriptor.protocol == ProviderProtocol::AnthropicWire {
            headers.insert("accept".into(), "application/json".into());
            headers.insert("content-type".into(), "application/json".into());
            headers.insert("anthropic-version".into(), "2023-06-01".into());
            headers.insert("x-api-key".into(), "<redacted>".into());
        }
        headers
    }

    fn effective_user_agent(&self) -> &str {
        self.user_agent.as_deref().unwrap_or(DEFAULT_USER_AGENT)
    }
}

fn is_reserved_runtime_header(name: &str) -> bool {
    matches!(
        name.trim().to_ascii_lowercase().as_str(),
        "x-api-key" | "anthropic-version" | "content-type" | "accept" | "user-agent"
    )
}

fn sanitize_header_value(name: &str, value: &str) -> String {
    let name = name.trim().to_ascii_lowercase();
    if ["authorization", "x-api-key", "cookie"]
        .iter()
        .any(|candidate| name == *candidate)
        || ["token", "secret", "apikey", "api-key", "auth"]
            .iter()
            .any(|needle| name.contains(needle))
    {
        "<redacted>".into()
    } else {
        value.into()
    }
}

impl InferenceProvider for ProviderFacade {
    fn descriptor(&self) -> &ProviderDescriptor {
        &self.descriptor
    }

    fn prepare_request(&self, request: &ProviderRequest) -> PreparedRequest {
        PreparedRequest {
            provider_name: self.descriptor.name.clone(),
            protocol: self.descriptor.protocol,
            endpoint: endpoint_for_protocol(&self.descriptor.base_url, self.descriptor.protocol),
            model: request
                .override_model
                .clone()
                .unwrap_or_else(|| self.descriptor.default_model.clone()),
            input: request.input.clone(),
            rendered_input: request
                .rendered_input
                .clone()
                .unwrap_or_else(|| request.input.clone()),
            user_agent: Some(self.effective_user_agent().into()),
            sanitized_headers: self.build_sanitized_request_headers(),
        }
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
mod tests;
