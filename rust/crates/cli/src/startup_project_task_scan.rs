use crate::CliError;
use fin_contracts::ExecutionStateRecord;
use fin_runtime::StoredTaskRecord;
use serde::Deserialize;
use serde_json::Value;
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct ProjectTaskScan {
    pub(crate) unfinished_task_count: usize,
    pub(crate) last_active_task_id: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct ProjectTaskAggregate {
    unfinished_task_ids: BTreeSet<String>,
    last_active_task_id: Option<String>,
    last_active_updated_at: Option<String>,
}

pub(crate) fn scan_project_tasks(
    runtime_home: &Path,
) -> Result<Vec<(String, ProjectTaskScan)>, CliError> {
    let sessions_root = runtime_home.join("sessions");
    if !sessions_root.exists() {
        return Ok(Vec::new());
    }

    let mut grouped = BTreeMap::<String, ProjectTaskAggregate>::new();
    for session_dir in session_dirs(&sessions_root)? {
        let Some(project_id) = session_project_id(&session_dir)? else {
            continue;
        };
        let aggregate = grouped.entry(project_id).or_default();
        let state = read_json_optional::<ExecutionStateRecord>(
            &session_dir.join("control/execution_state.json"),
        )?;
        merge_execution_state(aggregate, state.as_ref());
        for task in read_task_registry(&session_dir)? {
            merge_task_record(aggregate, &task);
        }
    }

    Ok(grouped
        .into_iter()
        .map(|(project_id, aggregate)| {
            (
                project_id,
                ProjectTaskScan {
                    unfinished_task_count: aggregate.unfinished_task_ids.len(),
                    last_active_task_id: aggregate.last_active_task_id,
                },
            )
        })
        .collect())
}

fn merge_execution_state(
    aggregate: &mut ProjectTaskAggregate,
    state: Option<&ExecutionStateRecord>,
) {
    let Some(state) = state else {
        return;
    };
    let Some(task_id) = state.refs.task_id.as_deref() else {
        return;
    };
    update_last_active(aggregate, task_id, state.updated_at.as_str());
    if task_is_unfinished_state(state) {
        aggregate.unfinished_task_ids.insert(task_id.to_string());
    }
}

fn merge_task_record(aggregate: &mut ProjectTaskAggregate, task: &StoredTaskRecord) {
    update_last_active(aggregate, task.task_id.as_str(), task.updated_at.as_str());
    if task_is_unfinished_record(task) {
        aggregate.unfinished_task_ids.insert(task.task_id.clone());
    }
}

fn update_last_active(aggregate: &mut ProjectTaskAggregate, task_id: &str, updated_at: &str) {
    let should_replace = aggregate
        .last_active_updated_at
        .as_deref()
        .map(|current| updated_at >= current)
        .unwrap_or(true);
    if should_replace {
        aggregate.last_active_task_id = Some(task_id.to_string());
        aggregate.last_active_updated_at = Some(updated_at.to_string());
    }
}

fn task_is_unfinished_state(state: &ExecutionStateRecord) -> bool {
    matches!(
        state.status.as_str(),
        "running" | "paused" | "waiting_external"
    ) || state.pending_input_count > 0
}

fn task_is_unfinished_record(task: &StoredTaskRecord) -> bool {
    matches!(
        task.status.as_str(),
        "created" | "ready" | "claimed" | "working" | "submitted" | "reviewing"
    )
}

fn read_task_registry(session_dir: &Path) -> Result<Vec<StoredTaskRecord>, CliError> {
    let registry_dir = session_dir.join("tasks/registry");
    if !registry_dir.exists() {
        return Ok(Vec::new());
    }
    let mut tasks = Vec::new();
    for entry in fs::read_dir(&registry_dir).map_err(|source| CliError::ReadFile {
        path: registry_dir.display().to_string(),
        source,
    })? {
        let entry = entry.map_err(|source| CliError::ReadFile {
            path: registry_dir.display().to_string(),
            source,
        })?;
        let path = entry.path();
        if path.extension().and_then(|value| value.to_str()) != Some("json") {
            continue;
        }
        let content = fs::read_to_string(&path).map_err(|source| CliError::ReadFile {
            path: path.display().to_string(),
            source,
        })?;
        let task =
            serde_json::from_str::<StoredTaskRecord>(&content).map_err(CliError::Serialize)?;
        tasks.push(task);
    }
    Ok(tasks)
}

fn session_project_id(session_dir: &Path) -> Result<Option<String>, CliError> {
    let context = read_json_optional::<Value>(&session_dir.join("context/current_context.json"))?;
    Ok(context
        .as_ref()
        .and_then(|value| value.get("project"))
        .and_then(|value| value.get("primary_project"))
        .and_then(|value| value.get("project_id"))
        .and_then(Value::as_str)
        .map(str::to_string))
}

fn session_dirs(root: &Path) -> Result<Vec<PathBuf>, CliError> {
    let mut dirs = Vec::new();
    for year in fs::read_dir(root).map_err(|source| CliError::ReadFile {
        path: root.display().to_string(),
        source,
    })? {
        let year = year.map_err(read_dir_error(root))?;
        if !year.path().is_dir() {
            continue;
        }
        for month in fs::read_dir(year.path()).map_err(read_dir_error(&year.path()))? {
            let month = month.map_err(read_dir_error(&year.path()))?;
            if !month.path().is_dir() {
                continue;
            }
            for session in fs::read_dir(month.path()).map_err(read_dir_error(&month.path()))? {
                let session = session.map_err(read_dir_error(&month.path()))?;
                if session.path().is_dir() {
                    dirs.push(session.path());
                }
            }
        }
    }
    Ok(dirs)
}

fn read_dir_error(path: &Path) -> impl FnOnce(std::io::Error) -> CliError + '_ {
    move |source| CliError::ReadFile {
        path: path.display().to_string(),
        source,
    }
}

fn read_json_optional<T: for<'de> Deserialize<'de>>(path: &Path) -> Result<Option<T>, CliError> {
    match fs::read_to_string(path) {
        Ok(content) => serde_json::from_str(&content)
            .map(Some)
            .map_err(CliError::Serialize),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(source) => Err(CliError::ReadFile {
            path: path.display().to_string(),
            source,
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_runtime_home(prefix: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time")
            .as_nanos();
        let path = std::env::temp_dir().join(format!("fin-startup-task-scan-{prefix}-{unique}"));
        fs::create_dir_all(&path).expect("temp runtime home");
        path
    }

    fn write_file(path: &Path, bytes: &[u8]) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("parent");
        }
        fs::write(path, bytes).expect("write");
    }

    #[test]
    fn scan_counts_unfinished_registry_tasks_for_project_session() {
        let home = temp_runtime_home("registry");
        let session_dir = home.join("sessions/2026/04/session-fin");
        write_file(
            &session_dir.join("context/current_context.json"),
            br#"{"project":{"primary_project":{"project_id":"fin"}}}"#,
        );
        write_file(
            &session_dir.join("control/execution_state.json"),
            br#"{
  "state_id":"exec-state-fin",
  "session_id":"session-fin",
  "task_id":"task-fin-state",
  "status":"idle",
  "pending_input_count":0,
  "accepts_user_input":true,
  "updated_at":"2026-04-21T21:00:00+08:00"
}"#,
        );
        write_file(
            &session_dir.join("tasks/registry/task-fin-1.json"),
            br#"{
  "task_id":"task-fin-1",
  "session_id":"session-fin",
  "title":"task 1",
  "summary":"claimed work",
  "status":"claimed",
  "claimed_by_worker_id":"worker-mbp-builder",
  "created_at":"2026-04-21T21:00:00+08:00",
  "updated_at":"2026-04-21T21:01:00+08:00"
}"#,
        );
        write_file(
            &session_dir.join("tasks/registry/task-fin-2.json"),
            br#"{
  "task_id":"task-fin-2",
  "session_id":"session-fin",
  "title":"task 2",
  "summary":"review work",
  "status":"reviewing",
  "created_at":"2026-04-21T21:00:00+08:00",
  "updated_at":"2026-04-21T21:02:00+08:00"
}"#,
        );
        write_file(
            &session_dir.join("tasks/registry/task-fin-done.json"),
            br#"{
  "task_id":"task-fin-done",
  "session_id":"session-fin",
  "title":"task done",
  "summary":"done",
  "status":"done",
  "created_at":"2026-04-21T21:00:00+08:00",
  "updated_at":"2026-04-21T21:03:00+08:00"
}"#,
        );

        let scan = scan_project_tasks(&home).expect("scan");
        assert_eq!(scan.len(), 1);
        assert_eq!(scan[0].0, "fin");
        assert_eq!(scan[0].1.unfinished_task_count, 2);
        assert_eq!(
            scan[0].1.last_active_task_id.as_deref(),
            Some("task-fin-done")
        );
    }
}
