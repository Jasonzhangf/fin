use crate::{
    CliError,
    project_supervision::{ProjectSupervisionRecord, ProjectSupervisionSnapshot},
};
use fin_runtime::handoff_project_task;
use serde::{Deserialize, Serialize};
use std::{fs, path::Path};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct ProjectExecutionHandoffRecord {
    pub(crate) project_id: String,
    pub(crate) agent_id: String,
    pub(crate) worker_id: String,
    #[serde(default)]
    pub(crate) session_id: Option<String>,
    #[serde(default)]
    pub(crate) task_id: Option<String>,
    pub(crate) handoff_state: String,
    pub(crate) updated_at: String,
    pub(crate) summary: String,
    #[serde(default)]
    pub(crate) artifact_refs: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct ProjectExecutionHandoffSnapshot {
    pub(crate) updated_at: String,
    pub(crate) project_count: usize,
    pub(crate) prepared_count: usize,
    pub(crate) noop_count: usize,
    pub(crate) missing_task_count: usize,
    #[serde(default)]
    pub(crate) projects: Vec<ProjectExecutionHandoffRecord>,
}

impl ProjectExecutionHandoffSnapshot {
    pub(crate) fn status_summary(&self) -> String {
        format!(
            "prepared={} noop={} missing_task={}",
            self.prepared_count, self.noop_count, self.missing_task_count
        )
    }
}

pub(crate) fn materialize_project_execution_handoffs(
    runtime_home: &Path,
    supervision: &ProjectSupervisionSnapshot,
    updated_at: &str,
) -> Result<ProjectExecutionHandoffSnapshot, CliError> {
    let mut snapshot = ProjectExecutionHandoffSnapshot {
        updated_at: updated_at.into(),
        project_count: supervision.projects.len(),
        ..ProjectExecutionHandoffSnapshot::default()
    };
    for project in &supervision.projects {
        if !should_prepare_handoff(project) {
            continue;
        }
        let worker_id = project_worker_id(project);
        let Some(task_id) = project.resume_task_id.clone() else {
            snapshot.missing_task_count += 1;
            snapshot.projects.push(ProjectExecutionHandoffRecord {
                project_id: project.project_id.clone(),
                agent_id: project.agent_id.clone(),
                worker_id,
                session_id: None,
                task_id: None,
                handoff_state: "missing_task".into(),
                updated_at: updated_at.into(),
                summary: "resume_ready project missing resume_task_id".into(),
                artifact_refs: Vec::new(),
            });
            continue;
        };
        let receipt = handoff_project_task(runtime_home, &task_id, None, &worker_id, updated_at)
            .map_err(map_runtime_error)?;
        match receipt {
            Some(receipt) => {
                let handoff_state = if receipt.artifact_refs.is_empty() {
                    "noop"
                } else {
                    "prepared"
                };
                match handoff_state {
                    "prepared" => snapshot.prepared_count += 1,
                    _ => snapshot.noop_count += 1,
                }
                snapshot.projects.push(ProjectExecutionHandoffRecord {
                    project_id: project.project_id.clone(),
                    agent_id: project.agent_id.clone(),
                    worker_id,
                    session_id: Some(receipt.session_id),
                    task_id: Some(receipt.task_id),
                    handoff_state: handoff_state.into(),
                    updated_at: updated_at.into(),
                    summary: receipt.summary,
                    artifact_refs: receipt.artifact_refs,
                });
            }
            None => {
                snapshot.missing_task_count += 1;
                snapshot.projects.push(ProjectExecutionHandoffRecord {
                    project_id: project.project_id.clone(),
                    agent_id: project.agent_id.clone(),
                    worker_id,
                    session_id: None,
                    task_id: Some(task_id),
                    handoff_state: "missing_task".into(),
                    updated_at: updated_at.into(),
                    summary: "task not found in task registry".into(),
                    artifact_refs: Vec::new(),
                });
            }
        }
    }
    persist_snapshot(runtime_home, &snapshot)?;
    Ok(snapshot)
}

pub(crate) fn read_project_execution_handoffs(
    runtime_home: &Path,
) -> Result<Option<ProjectExecutionHandoffSnapshot>, CliError> {
    read_json_optional(
        &runtime_home.join("runtime/current/current_project_execution_handoffs.json"),
    )
}

fn should_prepare_handoff(project: &ProjectSupervisionRecord) -> bool {
    project.mode == "local" && project.desired_action == "resume_project_task"
}

fn project_worker_id(project: &ProjectSupervisionRecord) -> String {
    let agent_name = project
        .agent_id
        .rsplit('.')
        .next()
        .unwrap_or(project.agent_id.as_str());
    format!(
        "worker-{}",
        agent_name
            .chars()
            .map(|ch| {
                if ch.is_ascii_alphanumeric() {
                    ch.to_ascii_lowercase()
                } else {
                    '-'
                }
            })
            .collect::<String>()
            .trim_matches('-')
    )
}

fn persist_snapshot(
    runtime_home: &Path,
    snapshot: &ProjectExecutionHandoffSnapshot,
) -> Result<(), CliError> {
    let dir = runtime_home.join("runtime/projects/execution_handoffs");
    fs::create_dir_all(&dir).map_err(|source| CliError::WriteFile {
        path: dir.display().to_string(),
        source,
    })?;
    for project in &snapshot.projects {
        write_json(&dir.join(format!("{}.json", project.project_id)), project)?;
    }
    write_json(
        &runtime_home.join("runtime/projects/execution_handoffs.json"),
        snapshot,
    )?;
    write_json(
        &runtime_home.join("runtime/current/current_project_execution_handoffs.json"),
        snapshot,
    )
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

fn write_json<T: Serialize>(path: &Path, value: &T) -> Result<(), CliError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|source| CliError::WriteFile {
            path: parent.display().to_string(),
            source,
        })?;
    }
    fs::write(
        path,
        serde_json::to_vec_pretty(value).map_err(CliError::Serialize)?,
    )
    .map_err(|source| CliError::WriteFile {
        path: path.display().to_string(),
        source,
    })
}

fn map_runtime_error(error: fin_runtime::RuntimeError) -> CliError {
    match error {
        fin_runtime::RuntimeError::Serialize(err) => CliError::Serialize(err),
        fin_runtime::RuntimeError::Io { path, source } => CliError::ReadFile { path, source },
        other => CliError::Runtime(other),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fin_runtime::handoff_project_task;
    use serde_json::json;
    use std::{
        path::PathBuf,
        time::{SystemTime, UNIX_EPOCH},
    };

    fn temp_runtime_home(prefix: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time")
            .as_nanos();
        let path = std::env::temp_dir().join(format!("fin-project-exec-handoff-{prefix}-{unique}"));
        fs::create_dir_all(path.join("sessions/2026/05/session-1/tasks/registry"))
            .expect("temp runtime home");
        path
    }

    #[test]
    fn materialize_prepares_local_resume_task_handoff() {
        let home = temp_runtime_home("prepare");
        fs::write(
            home.join("sessions/2026/05/session-1/tasks/registry/task-1.json"),
            serde_json::to_vec_pretty(&json!({
                "task_id":"task-1",
                "session_id":"session-1",
                "title":"task",
                "summary":"task",
                "status":"ready",
                "created_at":"2026-04-20T22:00:00+08:00",
                "updated_at":"2026-04-20T22:00:00+08:00"
            }))
            .expect("json"),
        )
        .expect("task");

        let supervision = ProjectSupervisionSnapshot {
            updated_at: "2026-04-20T22:01:00+08:00".into(),
            project_count: 1,
            resume_ready_count: 1,
            projects: vec![ProjectSupervisionRecord {
                project_id: "fin".into(),
                agent_id: "mbp.builder".into(),
                mode: "local".into(),
                presence_state: "idle".into(),
                unfinished_task_count: 1,
                resume_task_id: Some("task-1".into()),
                supervision_state: "resume_ready".into(),
                desired_action: "resume_project_task".into(),
                updated_at: "2026-04-20T22:01:00+08:00".into(),
                summary: "resume".into(),
            }],
            ..ProjectSupervisionSnapshot::default()
        };

        let snapshot =
            materialize_project_execution_handoffs(&home, &supervision, &supervision.updated_at)
                .expect("handoff snapshot");
        assert_eq!(snapshot.prepared_count, 1);
        assert_eq!(snapshot.projects[0].handoff_state, "prepared");
        let task =
            fs::read_to_string(home.join("sessions/2026/05/session-1/tasks/registry/task-1.json"))
                .expect("task");
        assert!(task.contains("\"status\": \"claimed\""));
    }

    #[test]
    fn handoff_is_idempotent_for_same_worker() {
        let home = temp_runtime_home("idempotent");
        fs::write(
            home.join("sessions/2026/05/session-1/tasks/registry/task-1.json"),
            serde_json::to_vec_pretty(&json!({
                "task_id":"task-1",
                "session_id":"session-1",
                "title":"task",
                "summary":"task",
                "status":"claimed",
                "claimed_by_worker_id":"worker-builder",
                "created_at":"2026-04-20T22:00:00+08:00",
                "updated_at":"2026-04-20T22:00:00+08:00"
            }))
            .expect("json"),
        )
        .expect("task");
        let receipt = handoff_project_task(
            &home,
            "task-1",
            None,
            "worker-builder",
            "2026-04-20T22:05:00+08:00",
        )
        .expect("handoff")
        .expect("receipt");
        assert!(receipt.artifact_refs.is_empty());
        assert_eq!(receipt.summary, "task already handed off to same worker");
    }
}
