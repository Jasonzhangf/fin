use crate::task::board_snapshot::TaskSummary;
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoredTaskRecord {
    pub task_id: String,
    pub session_id: String,
    pub title: String,
    pub summary: String,
    #[serde(default)]
    pub epic_id: Option<String>,
    pub status: String,
    #[serde(default)]
    pub creator_worker_id: Option<String>,
    #[serde(default)]
    pub creator_role_id: Option<String>,
    #[serde(default)]
    pub review_owner_worker_id: Option<String>,
    #[serde(default)]
    pub claimed_by_worker_id: Option<String>,
    #[serde(default)]
    pub submitted_by_worker_id: Option<String>,
    #[serde(default)]
    pub reviewed_by_worker_id: Option<String>,
    #[serde(default)]
    pub latest_submission_summary: Option<String>,
    #[serde(default)]
    pub latest_review_decision: Option<String>,
    #[serde(default)]
    pub latest_review_summary: Option<String>,
    #[serde(default)]
    pub artifact_refs: Vec<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskMutationReceipt {
    pub task: StoredTaskRecord,
    pub artifact_refs: Vec<String>,
}

pub fn list_registered_tasks(
    runtime_home: &Path,
) -> Result<BTreeMap<String, (TaskSummary, StoredTaskRecord)>, String> {
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
                let session_dir = session.path();
                if !session_dir.is_dir() {
                    continue;
                }
                let registry_dir = session_dir.join("tasks/registry");
                let Ok(entries) = fs::read_dir(&registry_dir) else {
                    continue;
                };
                let session_id = session.file_name().to_string_lossy().to_string();
                for entry in entries.filter_map(Result::ok) {
                    let path = entry.path();
                    if path.extension().and_then(|value| value.to_str()) != Some("json") {
                        continue;
                    }
                    let Ok(content) = fs::read_to_string(&path) else {
                        continue;
                    };
                    let Ok(task) = serde_json::from_str::<StoredTaskRecord>(&content) else {
                        continue;
                    };
                    tasks.insert(
                        task.task_id.clone(),
                        (
                            TaskSummary {
                                task_id: task.task_id.clone(),
                                session_id: session_id.clone(),
                                session_dir: session_dir.clone(),
                            },
                            task,
                        ),
                    );
                }
            }
        }
    }
    Ok(tasks)
}

pub fn create_task_record(
    runtime_home: &Path,
    session_id: &str,
    task: StoredTaskRecord,
) -> Result<TaskMutationReceipt, String> {
    persist_task_record(runtime_home, session_id, task)
}

pub fn load_task_record(
    runtime_home: &Path,
    task_id: &str,
    preferred_session_id: Option<&str>,
) -> Result<Option<(TaskSummary, StoredTaskRecord)>, String> {
    let tasks = list_registered_tasks(runtime_home)?;
    if let Some(session_id) = preferred_session_id {
        if let Some((summary, task)) = tasks
            .values()
            .find(|(summary, task)| summary.session_id == session_id && task.task_id == task_id)
        {
            return Ok(Some((summary.clone(), task.clone())));
        }
    }
    Ok(tasks.get(task_id).cloned())
}

pub fn update_task_record(
    runtime_home: &Path,
    session_id: &str,
    task: StoredTaskRecord,
) -> Result<TaskMutationReceipt, String> {
    persist_task_record(runtime_home, session_id, task)
}

pub fn session_dir_for_session_id(
    runtime_home: &Path,
    session_id: &str,
) -> Result<PathBuf, String> {
    let sessions_root = runtime_home.join("sessions");
    if !sessions_root.exists() {
        return Err("runtime_home/sessions does not exist".into());
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
            let candidate = month.path().join(session_id);
            if candidate.is_dir() {
                return Ok(candidate);
            }
        }
    }
    Err(format!(
        "session directory not found for session_id={session_id}"
    ))
}

fn persist_task_record(
    runtime_home: &Path,
    session_id: &str,
    task: StoredTaskRecord,
) -> Result<TaskMutationReceipt, String> {
    let session_dir = session_dir_for_session_id(runtime_home, session_id)?;
    let registry_dir = session_dir.join("tasks/registry");
    fs::create_dir_all(&registry_dir).map_err(|error| error.to_string())?;
    fs::create_dir_all(session_dir.join("tasks/board")).map_err(|error| error.to_string())?;
    let task_path = registry_dir.join(format!("{}.json", task.task_id));
    let payload = serde_json::to_vec_pretty(&task).map_err(|error| error.to_string())?;
    fs::write(&task_path, payload).map_err(|error| error.to_string())?;

    let board_path = session_dir.join("tasks/board/latest.json");
    let board_snapshot = build_session_board_snapshot(&registry_dir)?;
    let board_bytes =
        serde_json::to_vec_pretty(&board_snapshot).map_err(|error| error.to_string())?;
    fs::write(&board_path, board_bytes).map_err(|error| error.to_string())?;

    let current_board = runtime_home.join("runtime/current/current_task_board.json");
    if let Some(parent) = current_board.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    let current_bytes =
        serde_json::to_vec_pretty(&board_snapshot).map_err(|error| error.to_string())?;
    fs::write(&current_board, current_bytes).map_err(|error| error.to_string())?;

    Ok(TaskMutationReceipt {
        task,
        artifact_refs: vec![
            relative_artifact(runtime_home, &task_path),
            relative_artifact(runtime_home, &board_path),
            relative_artifact(runtime_home, &current_board),
        ],
    })
}

fn build_session_board_snapshot(registry_dir: &Path) -> Result<serde_json::Value, String> {
    let mut items = Vec::new();
    if registry_dir.exists() {
        for entry in fs::read_dir(registry_dir).map_err(|error| error.to_string())? {
            let entry = entry.map_err(|error| error.to_string())?;
            let path = entry.path();
            if path.extension().and_then(|value| value.to_str()) != Some("json") {
                continue;
            }
            let content = fs::read_to_string(&path).map_err(|error| error.to_string())?;
            let task = serde_json::from_str::<StoredTaskRecord>(&content)
                .map_err(|error| error.to_string())?;
            items.push(task);
        }
    }
    items.sort_by(|left, right| left.task_id.cmp(&right.task_id));
    Ok(serde_json::json!({
        "task_count": items.len(),
        "tasks": items,
    }))
}

fn relative_artifact(runtime_home: &Path, absolute: &Path) -> String {
    absolute
        .strip_prefix(runtime_home)
        .ok()
        .map(|value| value.display().to_string())
        .unwrap_or_else(|| absolute.display().to_string())
}
