use super::*;

pub(super) fn parse_tool_calls(raw: &str, extraction_repaired: bool) -> ParsedToolCalls {
    let mut repaired = extraction_repaired;
    let mut working = raw.trim().to_string();
    let (without_fence, fence_repaired) = strip_json_code_fence(&working);
    working = without_fence;
    repaired |= fence_repaired;

    if let Ok(value) = serde_json::from_str::<Value>(&working) {
        return finalize_tool_calls(value, true, repaired, None);
    }

    if let Some(repaired_json) = repair_json_shape(&working) {
        repaired = true;
        if let Ok(value) = serde_json::from_str::<Value>(&repaired_json) {
            return finalize_tool_calls(value, true, repaired, None);
        }
    }

    let invalid_reason = classify_invalid_tool_calls(&working);
    ParsedToolCalls {
        block_present: true,
        parse_status: if partial_tool_signal_present(&working) {
            "masked_partial".into()
        } else {
            "invalid".into()
        },
        invalid_reason: Some(invalid_reason),
        calls: Vec::new(),
    }
}

fn finalize_tool_calls(
    value: Value,
    block_present: bool,
    repaired: bool,
    invalid_reason: Option<String>,
) -> ParsedToolCalls {
    match parse_tool_calls_value(value) {
        Ok((calls, normalized_repaired)) => ParsedToolCalls {
            block_present,
            parse_status: if repaired || normalized_repaired {
                "repaired_deterministic".into()
            } else {
                "exact".into()
            },
            invalid_reason,
            calls,
        },
        Err(reason) => ParsedToolCalls {
            block_present,
            parse_status: "invalid".into(),
            invalid_reason: Some(reason.into()),
            calls: Vec::new(),
        },
    }
}

fn parse_tool_calls_value(value: Value) -> Result<(Vec<ModelToolCall>, bool), &'static str> {
    let (items, repaired) = match value {
        Value::Array(items) => (items, false),
        other => (vec![other], true),
    };
    let mut normalized_repaired = repaired;
    let mut calls = Vec::with_capacity(items.len());
    for item in items {
        let normalized = normalize_tool_call(item)?;
        normalized_repaired |= normalized.repaired;
        calls.push(normalized.call);
    }
    Ok((calls, normalized_repaired))
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct NormalizedToolCall {
    call: ModelToolCall,
    repaired: bool,
}

fn normalize_tool_call(value: Value) -> Result<NormalizedToolCall, &'static str> {
    let object = value.as_object().ok_or("tool_call_not_object")?;
    let used_name_alias = object.contains_key("name") && !object.contains_key("tool_name");
    let tool_name = object
        .get("tool_name")
        .or_else(|| object.get("name"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or("missing_tool_name")?
        .to_string();
    let used_args_alias = object.contains_key("args") && !object.contains_key("arguments");
    let used_default_arguments = !object.contains_key("arguments") && !object.contains_key("args");
    let arguments = object
        .get("arguments")
        .or_else(|| object.get("args"))
        .cloned()
        .unwrap_or_else(|| Value::Object(Default::default()));
    Ok(NormalizedToolCall {
        call: ModelToolCall {
            tool_call_id: None,
            tool_name,
            arguments,
        },
        repaired: used_name_alias || used_args_alias || used_default_arguments,
    })
}

pub(super) fn parse_native_tool_calls(items: &[ProviderToolCall]) -> Vec<ModelToolCall> {
    items
        .iter()
        .map(|call| ModelToolCall {
            tool_call_id: Some(call.tool_call_id.clone()),
            tool_name: call.name.clone(),
            arguments: call.arguments.clone(),
        })
        .collect()
}
