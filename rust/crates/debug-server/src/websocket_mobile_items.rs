use crate::{DebugDataError, session_view};
use fin_contracts::ToolExecutionRecord;
use serde_json::{Value, json};
use std::{fs, path::Path};

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
        "item_id": record.tool_call_id,
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
