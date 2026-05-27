use fin_config::{ProviderCredential, ProviderProtocol, ResolvedProviderConfig};
use fin_shared::{DEFAULT_RETRY_ATTEMPTS, exponential_backoff};
use serde_json::Value;
use std::collections::BTreeMap;

mod http_client;
#[cfg(test)]
mod tests;

pub mod blocks {
    pub mod descriptor;
    pub mod errors;
    pub mod request;
    pub mod response;
}

pub mod wire {
    pub mod anthropic_wire;
    pub mod openai_wire;
}

pub use blocks::descriptor::{ProviderCapabilities, ProviderDescriptor, endpoint_for_protocol};
pub use blocks::errors::ProviderError;
pub use blocks::request::{PreparedRequest, ProviderRequest, TokenUsage};
pub use blocks::response::{ProviderResponse, parse_anthropic_response, parse_openai_response};

const DEFAULT_USER_AGENT: &str = "fin-coding-agent/0.1";

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

impl ProviderDescriptor {
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
            prompt_cache_key: request.prompt_cache_key.clone(),
            user_agent: None,
            sanitized_headers: BTreeMap::new(),
        }
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
            usage: Some(TokenUsage {
                prompt_tokens: Some(request.rendered_input.chars().count() as u64 / 4),
                completion_tokens: Some(8),
                total_tokens: None,
                cached_tokens: None,
                reasoning_tokens: None,
                usage_source: "static_estimate".into(),
            }),
        })
    }
}

#[derive(Debug, Clone)]
pub struct StructuredStaticProviderClient {
    descriptor: ProviderDescriptor,
}

impl StructuredStaticProviderClient {
    pub fn new(descriptor: ProviderDescriptor) -> Self {
        Self { descriptor }
    }
}

impl InferenceProvider for StructuredStaticProviderClient {
    fn descriptor(&self) -> &ProviderDescriptor {
        &self.descriptor
    }

    fn execute_prepared(
        &self,
        request: &PreparedRequest,
    ) -> Result<ProviderResponse, ProviderError> {
        let user_response = format!("simulated response for {}", request.input);
        let escaped_response = serde_json::to_string(&user_response).map_err(|error| {
            ProviderError::ParseResponse {
                message: format!("failed to encode structured static response: {error}"),
            }
        })?;
        let trimmed_input = request.input.trim();
        let word_count = trimmed_input.split_whitespace().count();
        let punctuation_count = trimmed_input
            .chars()
            .filter(|ch| matches!(ch, '?' | '？' | '!' | '！'))
            .count();
        let is_simple_query = word_count <= 12 && punctuation_count <= 1;
        let continuity_confidence = if is_simple_query { 72 } else { 58 };
        let topic_shift_confidence = if is_simple_query { 28 } else { 36 };
        let simple_query_confidence = if is_simple_query { 88 } else { 24 };
        let escaped_current_topic =
            serde_json::to_string(trimmed_input).map_err(|error| ProviderError::ParseResponse {
                message: format!("failed to encode current topic summary: {error}"),
            })?;
        Ok(ProviderResponse {
            provider_name: request.provider_name.clone(),
            model: request.model.clone(),
            output_text: format!(
                "<fin_user_response>{user_response}</fin_user_response>\\
<fin_control_feedback>{{\"origin\":\"model_output_contract_v1\",\"is_continuation\":false,\"is_simple_query\":{is_simple_query},\"candidate_task_id\":null,\"candidate_topic_thread_id\":null,\"continuity_confidence\":{continuity_confidence},\"topic_shift_confidence\":{topic_shift_confidence},\"simple_query_confidence\":{simple_query_confidence},\"previous_topic_summary\":\"static provider\",\"current_topic_summary\":{escaped_current_topic},\"note_candidate\":{escaped_response},\"digest_candidate\":{escaped_response},\"reason\":\"structured static provider\"}}</fin_control_feedback>\\
<fin_tool_calls>[{{\"name\":\"reasoning.stop\",\"arguments\":{{\"summary\":{escaped_response}}}}}]</fin_tool_calls>"
            ),
            response_id: Some("structured-static-response".into()),
            stop_reason: Some("end_turn".into()),
            status: 200,
            usage: Some(TokenUsage {
                prompt_tokens: Some(request.rendered_input.chars().count() as u64 / 4),
                completion_tokens: Some(16),
                total_tokens: None,
                cached_tokens: None,
                reasoning_tokens: None,
                usage_source: "static_estimate".into(),
            }),
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

    fn execute_json_request(
        &self,
        request: &PreparedRequest,
        payload: &Value,
        headers: reqwest::header::HeaderMap,
        parser: fn(&PreparedRequest, u16, &str) -> Result<ProviderResponse, ProviderError>,
    ) -> Result<ProviderResponse, ProviderError> {
        let client = http_client::build_client()?;
        let mut last_retryable_error = None;

        for attempt in 1..=DEFAULT_RETRY_ATTEMPTS {
            let response = match client
                .post(&request.endpoint)
                .headers(headers.clone())
                .json(&payload)
                .send()
            {
                Ok(response) => response,
                Err(err) => {
                    let failure = http_client::classify_reqwest_error(
                        err,
                        "send",
                        &request.endpoint,
                        attempt,
                        DEFAULT_RETRY_ATTEMPTS,
                    );
                    if failure.retryable && attempt < DEFAULT_RETRY_ATTEMPTS {
                        last_retryable_error = Some(failure.message);
                        std::thread::sleep(exponential_backoff(attempt));
                        continue;
                    }
                    return Err(ProviderError::Request {
                        message: failure.message,
                    });
                }
            };

            let status = response.status().as_u16();
            let body = match response.text() {
                Ok(body) => body,
                Err(err) => {
                    let failure = http_client::classify_reqwest_error(
                        err,
                        "read_body",
                        &request.endpoint,
                        attempt,
                        DEFAULT_RETRY_ATTEMPTS,
                    );
                    if failure.retryable && attempt < DEFAULT_RETRY_ATTEMPTS {
                        last_retryable_error = Some(failure.message);
                        std::thread::sleep(exponential_backoff(attempt));
                        continue;
                    }
                    return Err(ProviderError::Request {
                        message: failure.message,
                    });
                }
            };

            if status >= 400 {
                return Err(ProviderError::HttpStatus { status, body });
            }

            return parser(request, status, &body);
        }

        Err(ProviderError::Request {
            message: last_retryable_error.unwrap_or_else(|| {
                format!(
                    "request failed after {} attempts; endpoint={}",
                    DEFAULT_RETRY_ATTEMPTS, request.endpoint
                )
            }),
        })
    }

    fn build_anthropic_headers(&self) -> Result<reqwest::header::HeaderMap, ProviderError> {
        let api_key = self.resolve_api_key()?;
        wire::anthropic_wire::build_headers(&api_key, self.effective_user_agent(), &self.headers)
    }

    fn build_openai_headers(&self) -> Result<reqwest::header::HeaderMap, ProviderError> {
        let api_key = self.resolve_api_key()?;
        wire::openai_wire::build_headers(&api_key, self.effective_user_agent(), &self.headers)
    }

    fn effective_user_agent(&self) -> &str {
        self.user_agent.as_deref().unwrap_or(DEFAULT_USER_AGENT)
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
            sanitized_headers: match self.descriptor.protocol {
                ProviderProtocol::AnthropicWire => wire::anthropic_wire::build_sanitized_headers(
                    self.effective_user_agent(),
                    &self.headers,
                ),
                ProviderProtocol::OpenAiCompatible => wire::openai_wire::build_sanitized_headers(
                    self.effective_user_agent(),
                    &self.headers,
                ),
            },
            prompt_cache_key: request.prompt_cache_key.clone(),
        }
    }

    fn execute_prepared(
        &self,
        request: &PreparedRequest,
    ) -> Result<ProviderResponse, ProviderError> {
        let api_key = self.resolve_api_key()?;
        match self.descriptor.protocol {
            ProviderProtocol::AnthropicWire => {
                let payload =
                    wire::anthropic_wire::build_payload(&request.model, &request.rendered_input);
                let headers = wire::anthropic_wire::build_headers(
                    &api_key,
                    self.effective_user_agent(),
                    &self.headers,
                )?;
                self.execute_json_request(request, &payload, headers, parse_anthropic_response)
            }
            ProviderProtocol::OpenAiCompatible => {
                let payload =
                    wire::openai_wire::build_payload(&request.model, &request.rendered_input);
                let headers = wire::openai_wire::build_headers(
                    &api_key,
                    self.effective_user_agent(),
                    &self.headers,
                )?;
                self.execute_json_request(request, &payload, headers, parse_openai_response)
            }
        }
    }
}
