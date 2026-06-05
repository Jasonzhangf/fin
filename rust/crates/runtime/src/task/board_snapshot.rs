use crate::task::managed_board::load_managed_task_board_truth;
use fin_contracts::ExecutionStateRecord;
use serde_json::Value;
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskSummary {
    pub task_id: String,
    pub session_id: String,
    pub session_dir: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TaskStatusSnapshot {
    pub(crate) task_id: String,
    pub(crate) session_id: String,
    pub(crate) status: String,
    pub(crate) pending_input_count: usize,
    pub(crate) action_kind: String,
    pub(crate) plan_step_count: usize,
    pub(crate) output_summary: String,
    pub(crate) artifact_refs: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct TaskBoardContextSnapshot {
    pub(crate) active_task_id: Option<String>,
    pub(crate) task_board_summary: Option<String>,
    pub(crate) known_task_ids: Vec<String>,
    pub(crate) task_status_counts: Vec<String>,
    pub(crate) ready_task_ids: Vec<String>,
    pub(crate) submitted_task_ids: Vec<String>,
    pub(crate) owner_loop_summary: Option<String>,
}

pub(crate) fn collect_tasks(runtime_home: &Path) -> Result<BTreeMap<String, TaskSummary>, String> {
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
                    tasks.insert(
                        task_id.clone(),
                        TaskSummary {
                            task_id,
                            session_id: session_id.clone(),
                            session_dir: session_path.clone(),
                        },
                    );
                }
            }
        }
    }
    Ok(tasks)
}

pub(crate) fn read_task_status(summary: &TaskSummary) -> Result<TaskStatusSnapshot, String> {
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
        .unwrap_or("none")
        .to_string();
    let plan_step_count = plan
        .as_ref()
        .and_then(|value| value.get("steps"))
        .and_then(Value::as_array)
        .map(|items| items.len())
        .unwrap_or(0);
    Ok(TaskStatusSnapshot {
        task_id: summary.task_id.clone(),
        session_id: summary.session_id.clone(),
        status: status.clone(),
        pending_input_count,
        action_kind: action_kind.clone(),
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

pub(crate) fn build_task_board_context(
    runtime_home: Option<&str>,
    session_id: Option<&str>,
    preferred_task_id: Option<&str>,
) -> TaskBoardContextSnapshot {
    let Some(runtime_home) = runtime_home.filter(|value| !value.trim().is_empty()) else {
        return TaskBoardContextSnapshot::default();
    };
    let tasks = match collect_tasks(Path::new(runtime_home)) {
        Ok(value) => value,
        Err(_) => return TaskBoardContextSnapshot::default(),
    };
    if tasks.is_empty() {
        return managed_task_board_context(runtime_home, session_id, preferred_task_id);
    }

    let mut known_task_ids = tasks.keys().cloned().collect::<Vec<_>>();
    known_task_ids.sort();
    known_task_ids.reverse();
    known_task_ids.truncate(8);

    let active_task_id = preferred_task_id
        .and_then(|task_id| tasks.get(task_id).map(|_| task_id.to_string()))
        .or_else(|| {
            session_id.and_then(|current_session| {
                tasks
                    .values()
                    .filter(|item| item.session_id == current_session)
                    .map(|item| item.task_id.clone())
                    .max()
            })
        });

    let task_board_summary = active_task_id
        .as_deref()
        .and_then(|task_id| tasks.get(task_id))
        .and_then(|summary| read_task_status(summary).ok())
        .map(|detail| {
            format!(
                "active_task={} · status={} · pending_inputs={} · action={} · plan_steps={} · known_tasks={}",
                detail.task_id,
                detail.status,
                detail.pending_input_count,
                detail.action_kind,
                detail.plan_step_count,
                known_task_ids.len()
            )
        })
        .or_else(|| {
            Some(format!(
                "known_tasks={} [{}]",
                known_task_ids.len(),
                known_task_ids.join(", ")
            ))
        });
    let managed = managed_task_board_context(runtime_home, session_id, preferred_task_id);

    TaskBoardContextSnapshot {
        active_task_id: managed.active_task_id.or(active_task_id),
        task_board_summary: managed.task_board_summary.or(task_board_summary),
        known_task_ids: if managed.known_task_ids.is_empty() {
            known_task_ids
        } else {
            managed.known_task_ids
        },
        task_status_counts: managed.task_status_counts,
        ready_task_ids: managed.ready_task_ids,
        submitted_task_ids: managed.submitted_task_ids,
        owner_loop_summary: managed.owner_loop_summary,
    }
}

fn managed_task_board_context(
    runtime_home: &str,
    session_id: Option<&str>,
    preferred_task_id: Option<&str>,
) -> TaskBoardContextSnapshot {
    let Ok(Some(truth)) =
        load_managed_task_board_truth(Path::new(runtime_home), session_id, preferred_task_id)
    else {
        return TaskBoardContextSnapshot::default();
    };

    TaskBoardContextSnapshot {
        active_task_id: truth.active_task_id,
        task_board_summary: Some(truth.task_board_summary),
        known_task_ids: truth.known_task_ids,
        task_status_counts: truth.task_status_counts,
        ready_task_ids: truth.ready_task_ids,
        submitted_task_ids: truth.submitted_task_ids,
        owner_loop_summary: Some(truth.owner_loop_summary),
    }
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
