use crate::task_store::{StoredTaskRecord, list_registered_tasks};
use fin_contracts::ExecutionStateRecord;
use serde_json::Value;
use std::{
    cmp::Reverse,
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
pub(super) struct TaskStatusSnapshot {
    pub(super) task_id: String,
    pub(super) session_id: String,
    pub(super) status: String,
    pub(super) pending_input_count: usize,
    pub(super) action_kind: String,
    pub(super) plan_step_count: usize,
    pub(super) output_summary: String,
    pub(super) artifact_refs: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct TaskBoardContextSnapshot {
    pub(super) active_task_id: Option<String>,
    pub(super) task_board_summary: Option<String>,
    pub(super) known_task_ids: Vec<String>,
    pub(super) task_status_counts: Vec<String>,
    pub(super) ready_task_ids: Vec<String>,
    pub(super) submitted_task_ids: Vec<String>,
    pub(super) owner_loop_summary: Option<String>,
}

pub(super) fn collect_tasks(runtime_home: &Path) -> Result<BTreeMap<String, TaskSummary>, String> {
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

pub(super) fn read_task_status(summary: &TaskSummary) -> Result<TaskStatusSnapshot, String> {
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

pub(super) fn build_task_board_context(
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
    let Ok(tasks) = list_registered_tasks(Path::new(runtime_home)) else {
        return TaskBoardContextSnapshot::default();
    };
    if tasks.is_empty() {
        return TaskBoardContextSnapshot::default();
    }

    let mut status_counts = BTreeMap::<String, usize>::new();
    let mut ready_task_ids = Vec::new();
    let mut submitted_task_ids = Vec::new();
    let mut working_task_ids = Vec::new();
    let mut known_task_ids = tasks.keys().cloned().collect::<Vec<_>>();
    known_task_ids.sort_by_key(|task_id| Reverse(task_id.clone()));
    known_task_ids.truncate(8);

    for (task_id, (_, task)) in &tasks {
        *status_counts.entry(task.status.clone()).or_insert(0) += 1;
        if is_ready_unclaimed(task) {
            ready_task_ids.push(task_id.clone());
        }
        if task.status == "submitted" {
            submitted_task_ids.push(task_id.clone());
        }
        if matches!(task.status.as_str(), "claimed" | "working" | "reviewing") {
            working_task_ids.push(task_id.clone());
        }
    }
    ready_task_ids.sort();
    submitted_task_ids.sort();
    working_task_ids.sort();

    let active_task_id = preferred_task_id
        .and_then(|task_id| tasks.get(task_id).map(|_| task_id.to_string()))
        .or_else(|| {
            session_id.and_then(|current_session| {
                tasks
                    .iter()
                    .filter(|(_, (summary, _))| summary.session_id == current_session)
                    .map(|(task_id, _)| task_id.clone())
                    .max()
            })
        });
    let counts = status_counts
        .iter()
        .map(|(status, count)| format!("{status}={count}"))
        .collect::<Vec<_>>();
    let owner_loop_summary = if !submitted_task_ids.is_empty() {
        Some(format!(
            "review_submitted_tasks count={} [{}]",
            submitted_task_ids.len(),
            submitted_task_ids.join(", ")
        ))
    } else if !ready_task_ids.is_empty() {
        Some(format!(
            "dispatch_ready_tasks count={} [{}]",
            ready_task_ids.len(),
            ready_task_ids.join(", ")
        ))
    } else if !working_task_ids.is_empty() {
        Some(format!(
            "wait_for_worker_feedback count={} [{}]",
            working_task_ids.len(),
            working_task_ids.join(", ")
        ))
    } else {
        Some("no_actionable_managed_tasks".into())
    };
    let task_board_summary = Some(format!(
        "managed_tasks={} · statuses={} · ready={} · submitted={}{}",
        tasks.len(),
        if counts.is_empty() {
            "none".into()
        } else {
            counts.join(",")
        },
        ready_task_ids.len(),
        submitted_task_ids.len(),
        active_task_id
            .as_deref()
            .map(|task_id| format!(" · active_task={task_id}"))
            .unwrap_or_default()
    ));

    TaskBoardContextSnapshot {
        active_task_id,
        task_board_summary,
        known_task_ids,
        task_status_counts: counts,
        ready_task_ids,
        submitted_task_ids,
        owner_loop_summary,
    }
}

fn is_ready_unclaimed(task: &StoredTaskRecord) -> bool {
    matches!(task.status.as_str(), "created" | "ready") && task.claimed_by_worker_id.is_none()
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
