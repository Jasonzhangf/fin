use super::tool_dispatch::{
    ToolDispatchInput, ToolDispatchOutcome, failed_record, read_string, runtime_home_from_context,
};
use fin_contracts::ToolExecutionRecord;
use serde_json::{Value, json};
use std::{
    fs,
    path::{Path, PathBuf},
};

pub(super) fn handle_context_history_rebuild(
    outcome: &mut ToolDispatchOutcome,
    input: &ToolDispatchInput<'_>,
    tool_call_id: &str,
    arguments: &Value,
) -> bool {
    let Some(runtime_home) = runtime_home_from_context(input.context) else {
        outcome.tool_records.push(failed_record(
            input,
            tool_call_id.into(),
            "context_history.rebuild",
            "missing context.project.runtime_home, cannot rebuild context history",
        ));
        return true;
    };
    let Some(session_dir) = session_dir_for_refs(&runtime_home, input.refs.session_id.as_deref())
    else {
        outcome.tool_records.push(failed_record(
            input,
            tool_call_id.into(),
            "context_history.rebuild",
            "missing active session_id, cannot rebuild context history",
        ));
        return true;
    };
    let current_context_path = runtime_home.join("runtime/current/current_context.json");
    let current_context = match read_json_value(&current_context_path) {
        Ok(Some(value)) => value,
        Ok(None) => {
            outcome.tool_records.push(failed_record(
                input,
                tool_call_id.into(),
                "context_history.rebuild",
                "current_context.json is missing; no artifact available to rebuild from",
            ));
            return true;
        }
        Err(error) => {
            outcome.tool_records.push(failed_record(
                input,
                tool_call_id.into(),
                "context_history.rebuild",
                format!("failed to read current context artifact: {error}").as_str(),
            ));
            return true;
        }
    };

    let recent_contexts_path = session_dir.join("context/recent_contexts.json");
    let mut recent_contexts = read_json_array(&recent_contexts_path).unwrap_or_default();
    let current_signature = current_context
        .get("operation_id")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    let already_present = recent_contexts.iter().any(|item| {
        item.get("operation_id").and_then(Value::as_str) == Some(current_signature.as_str())
    });
    if !already_present {
        recent_contexts.push(current_context);
    }
    if let Err(error) = write_json_value(
        &recent_contexts_path,
        &Value::Array(recent_contexts.clone()),
    ) {
        outcome.tool_records.push(failed_record(
            input,
            tool_call_id.into(),
            "context_history.rebuild",
            format!("failed to persist recent contexts: {error}").as_str(),
        ));
        return true;
    }

    let recent_digest_count = read_json_array(&session_dir.join("digests/recent.json"))
        .map(|value| value.len())
        .unwrap_or(0);
    let recent_reasoning_count = read_json_array(&session_dir.join("reasoning/recent.json"))
        .map(|value| value.len())
        .unwrap_or(0);
    let recent_tool_count = read_json_array(&session_dir.join("tools/recent.json"))
        .map(|value| value.len())
        .unwrap_or(0);
    let reason = read_string(arguments, "reason")
        .unwrap_or_else(|| "model_tool_context_history_rebuild".into());
    let rebuild_index = json!({
        "rebuilt_at": input.occurred_at,
        "reason": reason,
        "session_id": input.refs.session_id.clone(),
        "task_id": input.refs.task_id.clone(),
        "operation_id": input.operation_id,
        "trace_id": input.trace_id,
        "recent_context_count": recent_contexts.len(),
        "recent_digest_count": recent_digest_count,
        "recent_reasoning_count": recent_reasoning_count,
        "recent_tool_record_count": recent_tool_count,
    });
    let runtime_rebuild_path = runtime_home.join("runtime/current/current_rebuild_index.json");
    let session_rebuild_path = session_dir.join("context/rebuild-index.json");
    if let Err(error) = write_json_value(&runtime_rebuild_path, &rebuild_index)
        .and_then(|_| write_json_value(&session_rebuild_path, &rebuild_index))
    {
        outcome.tool_records.push(failed_record(
            input,
            tool_call_id.into(),
            "context_history.rebuild",
            format!("failed to persist rebuild index: {error}").as_str(),
        ));
        return true;
    }

    outcome.tool_records.push(ToolExecutionRecord {
        tool_call_id: tool_call_id.into(),
        operation_id: input.operation_id.into(),
        trace_id: input.trace_id.into(),
        refs: input.refs.clone(),
        tool_name: "context_history.rebuild".into(),
        tool_kind: "agent_tool".into(),
        title: "Context History Rebuild".into(),
        purpose: "refresh rebuild index from current session artifacts".into(),
        target_kind: Some("context_history".into()),
        target_ref: input.refs.session_id.clone(),
        input_summary: Some(format!("reason={reason}")),
        output_summary: Some(format!(
            "recent_contexts={}, digests={}, reasoning={}, tools={}",
            recent_contexts.len(),
            recent_digest_count,
            recent_reasoning_count,
            recent_tool_count
        )),
        status: "completed".into(),
        started_at: input.occurred_at.into(),
        ended_at: Some(input.occurred_at.into()),
        duration_ms: Some(0),
        side_effects: vec!["refresh_context_rebuild_index".into()],
        artifact_refs: vec![
            relative_artifact(&runtime_home, &recent_contexts_path),
            relative_artifact(&runtime_home, &runtime_rebuild_path),
            relative_artifact(&runtime_home, &session_rebuild_path),
        ],
        error_summary: None,
    });
    outcome
        .events
        .push(("context_history.rebuild_completed".into(), rebuild_index));
    outcome
        .note_hints
        .push("context_history.rebuild refreshed rebuild-index".into());
    true
}

fn session_dir_for_refs(runtime_home: &Path, session_id: Option<&str>) -> Option<PathBuf> {
    let session_id = session_id?.trim();
    if session_id.is_empty() {
        return None;
    }
    let sessions_root = runtime_home.join("sessions");
    for year in fs::read_dir(&sessions_root).ok()?.flatten() {
        if !year.path().is_dir() {
            continue;
        }
        for month in fs::read_dir(year.path()).ok()?.flatten() {
            if !month.path().is_dir() {
                continue;
            }
            let candidate = month.path().join(session_id);
            if candidate.is_dir() {
                return Some(candidate);
            }
        }
    }
    None
}

fn read_json_value(path: &Path) -> Result<Option<Value>, String> {
    match fs::read_to_string(path) {
        Ok(content) => serde_json::from_str::<Value>(&content)
            .map(Some)
            .map_err(|error| error.to_string()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error.to_string()),
    }
}

fn read_json_array(path: &Path) -> Result<Vec<Value>, String> {
    Ok(read_json_value(path)?
        .and_then(|value| value.as_array().cloned())
        .unwrap_or_default())
}

fn write_json_value(path: &Path, value: &Value) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    fs::write(
        path,
        serde_json::to_vec_pretty(value).map_err(|error| error.to_string())?,
    )
    .map_err(|error| error.to_string())
}

fn relative_artifact(runtime_home: &Path, path: &Path) -> String {
    path.strip_prefix(runtime_home)
        .map(|relative| relative.display().to_string())
        .unwrap_or_else(|_| path.display().to_string())
}
