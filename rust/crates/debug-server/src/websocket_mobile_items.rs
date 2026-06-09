use crate::{DebugDataError, session_view};
use fin_contracts::ToolExecutionRecord;
use serde_json::{Value, json};
use std::{fs, path::Path};

const WS_PROVIDER_STARTED_ITEM_PREFIX: &str = "provider-call-";

pub(crate) fn mobile_tool_records(
    runtime_home: &Path,
) -> Result<Vec<ToolExecutionRecord>, DebugDataError> {
    let last_run = session_view::read_last_run_json(runtime_home)?;
    let operation_id = last_run
        .get("operation_id")
        .and_then(Value::as_str)
        .unwrap_or("");
    if operation_id.is_empty() {
        return Ok(Vec::new());
    }
    let Some(path) =
        session_view::last_run_artifact_path(runtime_home, "session_recent_tool_records_path")?
    else {
        return Ok(Vec::new());
    };
    let body = fs::read_to_string(&path).map_err(|source| DebugDataError::Io {
        path: path.display().to_string(),
        source,
    })?;
    let records = serde_json::from_str::<Vec<ToolExecutionRecord>>(&body)?
        .into_iter()
        .filter(|record| record.operation_id == operation_id)
        .collect();
    Ok(records)
}

pub(crate) fn mobile_tool_item_frame(
    client_message_id: &str,
    turn_id: &str,
    record: &ToolExecutionRecord,
) -> String {
    let status = record.status.as_str();
    let event_type = if status == "failed" {
        "turn.item.failed"
    } else {
        "turn.item.completed"
    };
    json!({
        "type": event_type,
        "client_message_id": client_message_id,
        "turn_id": turn_id,
        "item_id": mobile_item_id(client_message_id, record),
        "item_kind": record.tool_kind,
        "label": record.tool_name,
        "title": record.title,
        "purpose": record.purpose,
        "status": status,
        "duration_ms": record.duration_ms,
        "input_summary": record.input_summary,
        "output_summary": record.output_summary,
        "error_summary": record.error_summary,
        "target_kind": record.target_kind,
    })
    .to_string()
}

pub(crate) fn mobile_provider_item_started_frame(client_message_id: &str, turn_id: &str) -> String {
    json!({
        "type": "turn.item.started",
        "client_message_id": client_message_id,
        "turn_id": turn_id,
        "item_id": format!("{WS_PROVIDER_STARTED_ITEM_PREFIX}{client_message_id}"),
        "item_kind": "provider",
        "label": "provider.call",
        "title": "Provider Call",
        "purpose": "dispatch compiled prompt to provider and wait for response",
        "status": "running",
        "duration_ms": null,
        "input_summary": "",
        "output_summary": "waiting for provider response",
        "error_summary": null,
        "target_kind": "provider",
    })
    .to_string()
}

pub(crate) fn mobile_history_turns(turns: Vec<Value>) -> Vec<Value> {
    turns.into_iter().map(mobile_history_turn).collect()
}

fn mobile_history_turn(turn: Value) -> Value {
    let user_input = string_field(&turn, "user_input");
    let assistant_response = string_field(&turn, "assistant_response")
        .or_else(|| string_field(&turn, "assistant_visible_output"))
        .unwrap_or_default();
    let mut projected = turn;
    if let Value::Object(ref mut object) = projected {
        object.insert("user_input".into(), json!(user_input.unwrap_or_default()));
        object.insert("assistant_response".into(), json!(assistant_response));
        if !object.contains_key("tool_execution_records") {
            object.insert("tool_execution_records".into(), json!([]));
        }
        if !object.contains_key("error_records") {
            object.insert("error_records".into(), json!([]));
        }
    }
    projected
}

fn string_field(value: &Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn mobile_item_id(client_message_id: &str, record: &ToolExecutionRecord) -> String {
    if record.tool_name == "provider.call" {
        format!("{WS_PROVIDER_STARTED_ITEM_PREFIX}{client_message_id}")
    } else {
        record.tool_call_id.clone()
    }
}
