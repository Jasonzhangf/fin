use crate::{
    RuntimeError,
    task_store::{load_task_record, update_task_record},
};
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskHandoffReceipt {
    pub task_id: String,
    pub session_id: String,
    pub previous_status: String,
    pub next_status: String,
    pub worker_id: String,
    pub summary: String,
    #[serde(default)]
    pub artifact_refs: Vec<String>,
}

pub fn handoff_project_task(
    runtime_home: &Path,
    task_id: &str,
    preferred_session_id: Option<&str>,
    worker_id: &str,
    occurred_at: &str,
) -> Result<Option<TaskHandoffReceipt>, RuntimeError> {
    let Some((summary, mut task)) = load_task_record(runtime_home, task_id, preferred_session_id)
        .map_err(map_string_error(runtime_home))?
    else {
        return Ok(None);
    };

    let previous_status = task.status.clone();
    if matches!(previous_status.as_str(), "done" | "cancelled") {
        return Ok(Some(TaskHandoffReceipt {
            task_id: task.task_id,
            session_id: summary.session_id,
            previous_status: previous_status.clone(),
            next_status: previous_status,
            worker_id: worker_id.into(),
            summary: "task already terminal; handoff skipped".into(),
            artifact_refs: Vec::new(),
        }));
    }
    if task.claimed_by_worker_id.as_deref() == Some(worker_id)
        && matches!(task.status.as_str(), "claimed" | "working" | "reviewing")
    {
        return Ok(Some(TaskHandoffReceipt {
            task_id: task.task_id,
            session_id: summary.session_id,
            previous_status: previous_status.clone(),
            next_status: previous_status,
            worker_id: worker_id.into(),
            summary: "task already handed off to same worker".into(),
            artifact_refs: Vec::new(),
        }));
    }

    task.status = if matches!(task.status.as_str(), "submitted" | "reviewing") {
        "reviewing".into()
    } else {
        "claimed".into()
    };
    task.claimed_by_worker_id = Some(worker_id.into());
    task.updated_at = occurred_at.into();
    let receipt = update_task_record(runtime_home, &summary.session_id, task.clone())
        .map_err(map_string_error(runtime_home))?;

    let next_status = task.status.clone();
    Ok(Some(TaskHandoffReceipt {
        task_id: task.task_id,
        session_id: summary.session_id,
        previous_status,
        next_status: next_status.clone(),
        worker_id: worker_id.into(),
        summary: format!(
            "task handed off to {} with status={}",
            worker_id, next_status
        ),
        artifact_refs: receipt.artifact_refs,
    }))
}

fn map_string_error(runtime_home: &Path) -> impl FnOnce(String) -> RuntimeError + '_ {
    move |message| RuntimeError::Io {
        path: runtime_home.display().to_string(),
        source: std::io::Error::other(message),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::task_store::{StoredTaskRecord, create_task_record};
    use std::{
        fs,
        path::PathBuf,
        time::{SystemTime, UNIX_EPOCH},
    };

    fn temp_runtime_home(prefix: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time")
            .as_nanos();
        let path = std::env::temp_dir().join(format!("fin-task-handoff-{prefix}-{unique}"));
        fs::create_dir_all(path.join("sessions/2026/04/session-1")).expect("temp runtime home");
        path
    }

    #[test]
    fn handoff_claims_ready_task_for_worker() {
        let home = temp_runtime_home("claim");
        let task = StoredTaskRecord {
            task_id: "task-1".into(),
            session_id: "session-1".into(),
            title: "task".into(),
            summary: "task".into(),
            status: "ready".into(),
            created_at: "2026-04-20T21:00:00+08:00".into(),
            updated_at: "2026-04-20T21:00:00+08:00".into(),
            ..StoredTaskRecord::default()
        };
        create_task_record(&home, "session-1", task).expect("create task");

        let receipt = handoff_project_task(
            &home,
            "task-1",
            Some("session-1"),
            "worker-mbp-builder",
            "2026-04-20T21:05:00+08:00",
        )
        .expect("handoff")
        .expect("receipt");

        assert_eq!(receipt.previous_status, "ready");
        assert_eq!(receipt.next_status, "claimed");
        let updated =
            fs::read_to_string(home.join("sessions/2026/04/session-1/tasks/registry/task-1.json"))
                .expect("task");
        assert!(updated.contains("\"claimed_by_worker_id\": \"worker-mbp-builder\""));
    }
}
