use super::*;

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
        let client = http_client::build_client()?;
        let api_key = self.resolve_api_key()?;
        let headers = self.build_anthropic_headers(&api_key)?;
        let payload = serde_json::json!({
            "model": request.model,
            "max_tokens": ANTHROPIC_MAX_OUTPUT_TOKENS,
            "messages": build_anthropic_messages(request),
            "tools": build_anthropic_tools(request),
        });
        let mut last_retryable_error = None;

        for attempt in 1..=MAX_REQUEST_ATTEMPTS {
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
                        MAX_REQUEST_ATTEMPTS,
                    );
                    if failure.retryable && attempt < MAX_REQUEST_ATTEMPTS {
                        last_retryable_error = Some(failure.message);
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
                        MAX_REQUEST_ATTEMPTS,
                    );
                    if failure.retryable && attempt < MAX_REQUEST_ATTEMPTS {
                        last_retryable_error = Some(failure.message);
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

            return parse_anthropic_response(request, status, &body);
        }

        Err(ProviderError::Request {
            message: last_retryable_error.unwrap_or_else(|| {
                format!(
                    "request failed after {MAX_REQUEST_ATTEMPTS} attempts; endpoint={}",
                    request.endpoint
                )
            }),
        })
    }

    pub(crate) fn build_anthropic_headers(
        &self,
        api_key: &str,
    ) -> Result<HeaderMap, ProviderError> {
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
            tools: request.tools.clone(),
            prior_tool_calls: request.prior_tool_calls.clone(),
            tool_results: request.tool_results.clone(),
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

fn parse_anthropic_response(
    request: &PreparedRequest,
    status: u16,
    body: &str,
) -> Result<ProviderResponse, ProviderError> {
    let parsed: Value = serde_json::from_str(body).map_err(|err| ProviderError::ParseResponse {
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
    let tool_calls = parsed
        .get("content")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter(|item| item.get("type").and_then(Value::as_str) == Some("tool_use"))
                .filter_map(|item| {
                    Some(ProviderToolCall {
                        tool_call_id: item.get("id")?.as_str()?.to_string(),
                        name: item.get("name")?.as_str()?.to_string(),
                        arguments: item.get("input").cloned().unwrap_or(Value::Null),
                    })
                })
                .collect::<Vec<_>>()
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
        tool_calls,
    })
}

fn build_anthropic_messages(request: &PreparedRequest) -> Vec<Value> {
    let mut messages = vec![serde_json::json!({
        "role": "user",
        "content": request.rendered_input,
    })];
    if !request.prior_tool_calls.is_empty() {
        messages.push(serde_json::json!({
            "role": "assistant",
            "content": request
                .prior_tool_calls
                .iter()
                .map(|call| {
                    serde_json::json!({
                        "type": "tool_use",
                        "id": call.tool_call_id,
                        "name": call.name,
                        "input": call.arguments,
                    })
                })
                .collect::<Vec<_>>(),
        }));
    }
    if !request.tool_results.is_empty() {
        messages.push(serde_json::json!({
            "role": "user",
            "content": request
                .tool_results
                .iter()
                .map(|result| {
                    serde_json::json!({
                        "type": "tool_result",
                        "tool_use_id": result.tool_call_id,
                        "is_error": result.is_error,
                        "content": result.content,
                    })
                })
                .collect::<Vec<_>>(),
        }));
    }
    messages
}

fn build_anthropic_tools(request: &PreparedRequest) -> Vec<Value> {
    request
        .tools
        .iter()
        .map(|tool| {
            serde_json::json!({
                "name": tool.name,
                "description": tool.description,
                "input_schema": tool.input_schema,
            })
        })
        .collect()
}
