use super::*;

pub(crate) fn parse_anthropic_response(
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

pub(crate) fn build_anthropic_messages(request: &PreparedRequest) -> Vec<Value> {
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

pub(crate) fn build_anthropic_tools(request: &PreparedRequest) -> Vec<Value> {
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

pub(crate) fn parse_openai_response(
    request: &PreparedRequest,
    status: u16,
    body: &str,
) -> Result<ProviderResponse, ProviderError> {
    let parsed: Value = serde_json::from_str(body).map_err(|err| ProviderError::ParseResponse {
        message: err.to_string(),
    })?;
    let output_text = parsed
        .get("choices")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| {
                    item.get("message")
                        .and_then(|m| m.get("content"))
                        .and_then(Value::as_str)
                })
                .collect::<String>()
        })
        .unwrap_or_default();
    let stop_reason = parsed
        .get("choices")
        .and_then(Value::as_array)
        .and_then(|items| items.first())
        .and_then(|item| item.get("finish_reason"))
        .and_then(Value::as_str)
        .map(str::to_string);
    let response_id = parsed.get("id").and_then(Value::as_str).map(str::to_string);
    Ok(ProviderResponse {
        provider_name: request.provider_name.clone(),
        model: request.model.clone(),
        output_text,
        response_id,
        stop_reason,
        status,
        tool_calls: Vec::new(),
    })
}
