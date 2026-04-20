use crate::tool_dispatch::{
    ToolDispatchInput, ToolDispatchOutcome, failed_record, read_string, read_u64,
    runtime_home_from_context, short_text,
};
use fin_contracts::{ExecutionStateRecord, ToolExecutionRecord};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

pub(super) fn handle_project_task_status(
    outcome: &mut ToolDispatchOutcome,
    input: &ToolDispatchInput<'_>,
    tool_call_id: &str,
    arguments: &Value,
) -> bool {
    let Some(runtime_home) = runtime_home_from_context(input.context) else {
        outcome.tool_records.push(failed_record(
            input,
            tool_call_id.into(),
            "project.task.status",
            "missing context.project.runtime_home, cannot inspect task status",
        ));
        return true;
    };
    let Some(task_id) = read_string(arguments, "task_id").or_else(|| input.refs.task_id.clone())
    else {
        outcome.tool_records.push(failed_record(
            input,
            tool_call_id.into(),
            "project.task.status",
            "missing task_id and current refs.task_id is unavailable",
        ));
        return true;
    };
    let tasks = match collect_tasks(&runtime_home) {
        Ok(value) => value,
        Err(error) => {
            outcome.tool_records.push(failed_record(
                input,
                tool_call_id.into(),
                "project.task.status",
                format!("failed to scan task registry: {error}").as_str(),
            ));
            return true;
        }
    };
    let Some(summary) = tasks.get(task_id.as_str()) else {
        outcome.tool_records.push(failed_record(
            input,
            tool_call_id.into(),
            "project.task.status",
            format!("task not found in runtime truth: {task_id}").as_str(),
        ));
        return true;
    };
    let detail = match build_task_status_detail(summary) {
        Ok(value) => value,
        Err(error) => {
            outcome.tool_records.push(failed_record(
                input,
                tool_call_id.into(),
                "project.task.status",
                error.as_str(),
            ));
            return true;
        }
    };
    outcome.tool_records.push(ToolExecutionRecord {
        tool_call_id: tool_call_id.into(),
        operation_id: input.operation_id.into(),
        trace_id: input.trace_id.into(),
        refs: input.refs.clone(),
        tool_name: "project.task.status".into(),
        tool_kind: "agent_tool".into(),
        title: "Project Task Status".into(),
        purpose: "inspect task status from execution, routing, and plan artifacts".into(),
        target_kind: Some("task".into()),
        target_ref: Some(task_id.clone()),
        input_summary: Some(format!("task_id={task_id}")),
        output_summary: Some(detail.output_summary.clone()),
        status: "completed".into(),
        started_at: input.occurred_at.into(),
        ended_at: Some(input.occurred_at.into()),
        duration_ms: Some(0),
        side_effects: vec!["read_task_artifacts".into()],
        artifact_refs: detail.artifact_refs,
        error_summary: None,
    });
    outcome.events.push((
        "project.task.status_completed".into(),
        json!({
            "tool_call_id": tool_call_id,
            "task_id": task_id,
            "session_id": summary.session_id,
            "status": detail.status,
            "pending_input_count": detail.pending_input_count,
            "plan_step_count": detail.plan_step_count,
        }),
    ));
    outcome
        .note_hints
        .push(format!("project.task.status {}", detail.output_summary));
    true
}

pub(super) fn handle_project_task_list(
    outcome: &mut ToolDispatchOutcome,
    input: &ToolDispatchInput<'_>,
    tool_call_id: &str,
    arguments: &Value,
) -> bool {
    let Some(runtime_home) = runtime_home_from_context(input.context) else {
        outcome.tool_records.push(failed_record(
            input,
            tool_call_id.into(),
            "project.task.list",
            "missing context.project.runtime_home, cannot list tasks",
        ));
        return true;
    };
    let limit = read_u64(arguments, "limit").unwrap_or(10).clamp(1, 50) as usize;
    let tasks = match collect_tasks(&runtime_home) {
        Ok(value) => value,
        Err(error) => {
            outcome.tool_records.push(failed_record(
                input,
                tool_call_id.into(),
                "project.task.list",
                format!("failed to scan task registry: {error}").as_str(),
            ));
            return true;
        }
    };
    let mut ids = tasks.keys().cloned().collect::<Vec<_>>();
    ids.sort();
    ids.reverse();
    ids.truncate(limit);
    let preview = ids
        .iter()
        .filter_map(|task_id| {
            tasks
                .get(task_id)
                .map(|item| format!("{task_id}@{}", item.session_id))
        })
        .collect::<Vec<_>>()
        .join(", ");
    outcome.tool_records.push(ToolExecutionRecord {
        tool_call_id: tool_call_id.into(),
        operation_id: input.operation_id.into(),
        trace_id: input.trace_id.into(),
        refs: input.refs.clone(),
        tool_name: "project.task.list".into(),
        tool_kind: "agent_tool".into(),
        title: "Project Task List".into(),
        purpose: "list known task ids from runtime/session truth".into(),
        target_kind: Some("task_registry".into()),
        target_ref: Some("runtime_home/sessions".into()),
        input_summary: Some(format!("limit={limit}")),
        output_summary: Some(format!(
            "tasks={} [{}]",
            ids.len(),
            short_text(&preview, 160)
        )),
        status: "completed".into(),
        started_at: input.occurred_at.into(),
        ended_at: Some(input.occurred_at.into()),
        duration_ms: Some(0),
        side_effects: vec!["read_task_registry".into()],
        artifact_refs: vec!["sessions".into()],
        error_summary: None,
    });
    outcome.events.push((
        "project.task.list_completed".into(),
        json!({
            "tool_call_id": tool_call_id,
            "count": ids.len(),
            "task_ids": ids,
        }),
    ));
    outcome
        .note_hints
        .push(format!("project.task.list returned {} task(s)", ids.len()));
    true
}

#[derive(Debug, Clone)]
struct TaskSummary {
    session_id: String,
    session_dir: PathBuf,
}

#[derive(Debug, Clone)]
struct TaskStatusDetail {
    status: String,
    pending_input_count: usize,
    plan_step_count: usize,
    output_summary: String,
    artifact_refs: Vec<String>,
}

fn collect_tasks(runtime_home: &Path) -> Result<BTreeMap<String, TaskSummary>, String> {
    let mut tasks = BTreeMap::new();
    let sessions_root = runtime_home.join("sessions");
    if !sessions_root.exists() {
        return Ok(tasks);
    }
    for year in fs::read_dir(&sessions_root).map_err(|error| error.to_string())? {
        let year = year.map_err(|error| error.to_string())?;
        if !year.path().is_dir() {
            continue;
        }
        for month in fs::read_dir(year.path()).map_err(|error| error.to_string())? {
            let month = month.map_err(|error| error.to_string())?;
            if !month.path().is_dir() {
                continue;
            }
            for session in fs::read_dir(month.path()).map_err(|error| error.to_string())? {
                let session = session.map_err(|error| error.to_string())?;
                let session_path = session.path();
                if !session_path.is_dir() {
                    continue;
                }
                let session_id = session.file_name().to_string_lossy().to_string();
                for task_id in task_ids_from_session(&session_path)? {
                    tasks.entry(task_id).or_insert_with(|| TaskSummary {
                        session_id: session_id.clone(),
                        session_dir: session_path.clone(),
                    });
                }
            }
        }
    }
    Ok(tasks)
}

fn build_task_status_detail(summary: &TaskSummary) -> Result<TaskStatusDetail, String> {
    let execution_state_path = summary.session_dir.join("control/execution_state.json");
    let execution_state = read_json_typed::<ExecutionStateRecord>(&execution_state_path)
        .map_err(|error| format!("failed to read execution state: {error}"))?;
    let routing_path = summary.session_dir.join("tasks/routing/latest_action.json");
    let routing = read_json_value(&routing_path)
        .map_err(|error| format!("failed to read latest routing action: {error}"))?;
    let plan_path = summary.session_dir.join("tasks/plan/latest.json");
    let plan = read_json_value(&plan_path)
        .map_err(|error| format!("failed to read latest plan artifact: {error}"))?;

    let status = execution_state
        .as_ref()
        .map(|value| value.status.clone())
        .unwrap_or_else(|| "unknown".into());
    let pending_input_count = execution_state
        .as_ref()
        .map(|value| value.pending_input_count)
        .unwrap_or(0);
    let action_kind = routing
        .as_ref()
        .and_then(|value| value.get("action_kind"))
        .and_then(Value::as_str)
        .unwrap_or("none");
    let plan_step_count = plan
        .as_ref()
        .and_then(|value| value.get("steps"))
        .and_then(Value::as_array)
        .map(|items| items.len())
        .unwrap_or(0);
    Ok(TaskStatusDetail {
        status: status.clone(),
        pending_input_count,
        plan_step_count,
        output_summary: format!(
            "session={} status={} pending_inputs={} action={} plan_steps={}",
            summary.session_id, status, pending_input_count, action_kind, plan_step_count
        ),
        artifact_refs: vec![
            execution_state_path.display().to_string(),
            routing_path.display().to_string(),
            plan_path.display().to_string(),
        ],
    })
}

fn task_ids_from_session(session_dir: &Path) -> Result<Vec<String>, String> {
    let mut ids = Vec::new();
    if let Some(task_id) = read_json_value(&session_dir.join("tasks/routing/latest_action.json"))?
        .as_ref()
        .and_then(|value| value.get("task_id"))
        .and_then(Value::as_str)
        .map(str::to_string)
    {
        ids.push(task_id);
    }
    if let Some(task_id) =
        read_json_typed::<ExecutionStateRecord>(&session_dir.join("control/execution_state.json"))?
            .and_then(|value| value.refs.task_id)
    {
        if !ids.contains(&task_id) {
            ids.push(task_id);
        }
    }
    if let Ok(messages) = read_json_array(&session_dir.join("conversation/messages.json")) {
        for task_id in messages.iter().filter_map(|item| {
            item.get("task_id")
                .and_then(Value::as_str)
                .map(str::to_string)
        }) {
            if !ids.contains(&task_id) {
                ids.push(task_id);
            }
        }
    }
    Ok(ids)
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

fn read_json_typed<T: serde::de::DeserializeOwned>(path: &Path) -> Result<Option<T>, String> {
    match fs::read_to_string(path) {
        Ok(content) => serde_json::from_str::<T>(&content)
            .map(Some)
            .map_err(|error| error.to_string()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error.to_string()),
    }
}
