use crate::RuntimeError;
use serde::{Deserialize, Serialize};
use std::{fs, path::Path};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssignmentRecord {
    pub assignment_id: String,
    pub peer_id: String,
    #[serde(default)]
    pub project_id: Option<String>,
    #[serde(default)]
    pub session_id: Option<String>,
    #[serde(default)]
    pub task_id: Option<String>,
    #[serde(default)]
    pub target_worker_id: Option<String>,
    #[serde(default)]
    pub target_agent_name: Option<String>,
    pub requested_role_id: String,
    #[serde(default)]
    pub owner_worker_id: Option<String>,
    pub task_summary: String,
    pub created_at: String,
    pub status: String,
    #[serde(default)]
    pub updated_at: Option<String>,
    #[serde(default)]
    pub started_at: Option<String>,
    #[serde(default)]
    pub completed_at: Option<String>,
    #[serde(default)]
    pub result_summary: Option<String>,
    #[serde(default)]
    pub artifact_refs: Vec<String>,
}

pub fn read_assignment_queue(runtime_home: &Path) -> Result<Vec<AssignmentRecord>, RuntimeError> {
    read_json_vec(&runtime_home.join("runtime/assignments/pending.json"))
}

pub fn append_assignment_record(
    runtime_home: &Path,
    record: AssignmentRecord,
) -> Result<Vec<AssignmentRecord>, RuntimeError> {
    let path = runtime_home.join("runtime/assignments/pending.json");
    let mut assignments = read_json_vec(&path)?;
    assignments.push(record);
    write_json(&path, &assignments)?;
    Ok(assignments)
}

pub fn update_assignment_record(
    runtime_home: &Path,
    assignment_id: &str,
    mutator: impl FnOnce(&mut AssignmentRecord),
) -> Result<Option<AssignmentRecord>, RuntimeError> {
    let path = runtime_home.join("runtime/assignments/pending.json");
    let mut assignments = read_json_vec(&path)?;
    let mut updated = None;
    if let Some(record) = assignments
        .iter_mut()
        .find(|item| item.assignment_id == assignment_id)
    {
        mutator(record);
        updated = Some(record.clone());
    }
    write_json(&path, &assignments)?;
    Ok(updated)
}

pub fn target_agent_name_from_worker_id(worker_id: &str) -> Option<String> {
    worker_id
        .strip_prefix("worker-")
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_string())
}

fn read_json_vec(path: &Path) -> Result<Vec<AssignmentRecord>, RuntimeError> {
    match fs::read_to_string(path) {
        Ok(content) => serde_json::from_str(&content).map_err(RuntimeError::Serialize),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(source) => Err(RuntimeError::Io {
            path: path.display().to_string(),
            source,
        }),
    }
}

fn write_json<T: Serialize>(path: &Path, value: &T) -> Result<(), RuntimeError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|source| RuntimeError::Io {
            path: parent.display().to_string(),
            source,
        })?;
    }
    fs::write(
        path,
        serde_json::to_vec_pretty(value).map_err(RuntimeError::Serialize)?,
    )
    .map_err(|source| RuntimeError::Io {
        path: path.display().to_string(),
        source,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        path::PathBuf,
        time::{SystemTime, UNIX_EPOCH},
    };

    fn temp_runtime_home(prefix: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time")
            .as_nanos();
        let path = std::env::temp_dir().join(format!("fin-assignment-queue-{prefix}-{unique}"));
        fs::create_dir_all(&path).expect("temp runtime home");
        path
    }

    #[test]
    fn append_and_update_assignment_record_round_trip() {
        let home = temp_runtime_home("round-trip");
        let assignment = AssignmentRecord {
            assignment_id: "assign-1".into(),
            peer_id: "local-worker-builder".into(),
            project_id: Some("fin".into()),
            session_id: Some("session-1".into()),
            task_id: Some("task-1".into()),
            target_worker_id: Some("worker-builder".into()),
            target_agent_name: Some("builder".into()),
            requested_role_id: "project".into(),
            owner_worker_id: Some("worker-owner".into()),
            task_summary: "implement task".into(),
            created_at: "2026-04-21T20:00:00+08:00".into(),
            status: "pending".into(),
            ..AssignmentRecord::default()
        };

        let all = append_assignment_record(&home, assignment).expect("append");
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].task_id.as_deref(), Some("task-1"));

        let updated = update_assignment_record(&home, "assign-1", |record| {
            record.status = "completed".into();
            record.completed_at = Some("2026-04-21T20:01:00+08:00".into());
        })
        .expect("update")
        .expect("updated");
        assert_eq!(updated.status, "completed");

        let persisted = read_assignment_queue(&home).expect("queue");
        assert_eq!(
            persisted[0].completed_at.as_deref(),
            Some("2026-04-21T20:01:00+08:00")
        );
    }

    #[test]
    fn target_agent_name_uses_worker_suffix() {
        assert_eq!(
            target_agent_name_from_worker_id("worker-builder").as_deref(),
            Some("builder")
        );
        assert_eq!(target_agent_name_from_worker_id("builder"), None);
    }
}
