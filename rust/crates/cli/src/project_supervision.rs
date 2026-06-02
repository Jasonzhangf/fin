use crate::{CliError, startup_topology::StartupTopologySnapshot};
use serde::{Deserialize, Serialize};
use std::{fs, path::Path};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct ProjectSupervisionRecord {
    pub(crate) project_id: String,
    pub(crate) agent_id: String,
    pub(crate) mode: String,
    pub(crate) presence_state: String,
    pub(crate) unfinished_task_count: usize,
    #[serde(default)]
    pub(crate) resume_task_id: Option<String>,
    pub(crate) supervision_state: String,
    pub(crate) desired_action: String,
    pub(crate) updated_at: String,
    pub(crate) summary: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct ProjectSupervisionSnapshot {
    pub(crate) updated_at: String,
    pub(crate) project_count: usize,
    pub(crate) ready_count: usize,
    pub(crate) resume_ready_count: usize,
    pub(crate) busy_count: usize,
    pub(crate) waiting_count: usize,
    pub(crate) recover_needed_count: usize,
    #[serde(default)]
    pub(crate) projects: Vec<ProjectSupervisionRecord>,
}

impl ProjectSupervisionSnapshot {
    pub(crate) fn status_summary(&self) -> String {
        format!(
            "ready={} resume_ready={} busy={} waiting={} recover_needed={}",
            self.ready_count,
            self.resume_ready_count,
            self.busy_count,
            self.waiting_count,
            self.recover_needed_count
        )
    }
}

pub(crate) fn materialize_project_supervision(
    runtime_home: &Path,
    topology: &StartupTopologySnapshot,
) -> Result<ProjectSupervisionSnapshot, CliError> {
    let mut snapshot = ProjectSupervisionSnapshot {
        updated_at: topology.updated_at.clone(),
        project_count: topology.projects.len(),
        ..ProjectSupervisionSnapshot::default()
    };
    for project in &topology.projects {
        let (supervision_state, desired_action, summary) = if project.presence_state == "offline"
            && (project.always_on || project.unfinished_task_count > 0)
        {
            (
                "recover_needed",
                "recover_project_agent",
                format!(
                    "project agent offline; recover needed for {}",
                    project
                        .last_active_task_id
                        .as_deref()
                        .unwrap_or("no-task-known")
                ),
            )
        } else if project.presence_state == "idle" && project.unfinished_task_count > 0 {
            (
                "resume_ready",
                "resume_project_task",
                format!(
                    "project agent ready to resume {}",
                    project
                        .last_active_task_id
                        .as_deref()
                        .unwrap_or("unknown-task")
                ),
            )
        } else if project.presence_state == "busy" {
            (
                "busy",
                "monitor_running_task",
                format!(
                    "project agent busy on {}",
                    project
                        .last_active_task_id
                        .as_deref()
                        .unwrap_or("unknown-task")
                ),
            )
        } else if project.presence_state == "waiting" {
            (
                "waiting",
                "await_remote_connect",
                "project agent waiting for remote attach/connect".into(),
            )
        } else {
            (
                "ready",
                "observe_ready",
                "project agent ready with no unfinished task".into(),
            )
        };
        match supervision_state {
            "recover_needed" => snapshot.recover_needed_count += 1,
            "resume_ready" => snapshot.resume_ready_count += 1,
            "busy" => snapshot.busy_count += 1,
            "waiting" => snapshot.waiting_count += 1,
            _ => snapshot.ready_count += 1,
        }
        snapshot.projects.push(ProjectSupervisionRecord {
            project_id: project.project_id.clone(),
            agent_id: project.agent_id.clone(),
            mode: project.mode.clone(),
            presence_state: project.presence_state.clone(),
            unfinished_task_count: project.unfinished_task_count,
            resume_task_id: project.last_active_task_id.clone(),
            supervision_state: supervision_state.into(),
            desired_action: desired_action.into(),
            updated_at: topology.updated_at.clone(),
            summary,
        });
    }
    persist_snapshot(runtime_home, &snapshot)?;
    Ok(snapshot)
}

pub(crate) fn read_project_supervision_snapshot(
    runtime_home: &Path,
) -> Result<Option<ProjectSupervisionSnapshot>, CliError> {
    read_json_optional(&runtime_home.join("runtime/current/current_project_supervision.json"))
}

fn persist_snapshot(
    runtime_home: &Path,
    snapshot: &ProjectSupervisionSnapshot,
) -> Result<(), CliError> {
    let dir = runtime_home.join("runtime/projects/supervision");
    fs::create_dir_all(&dir).map_err(|source| CliError::WriteFile {
        path: dir.display().to_string(),
        source,
    })?;
    for project in &snapshot.projects {
        write_json(&dir.join(format!("{}.json", project.project_id)), project)?;
    }
    write_json(
        &runtime_home.join("runtime/projects/supervision.json"),
        snapshot,
    )?;
    write_json(
        &runtime_home.join("runtime/current/current_project_supervision.json"),
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::startup_topology::{ProjectRegistryEntry, StartupTopologySnapshot};
    use std::{
        path::PathBuf,
        time::{SystemTime, UNIX_EPOCH},
    };

    fn temp_runtime_home(prefix: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time")
            .as_nanos();
        let path = std::env::temp_dir().join(format!("fin-project-supervision-{prefix}-{unique}"));
        fs::create_dir_all(&path).expect("temp runtime home");
        path
    }

    #[test]
    fn materialize_marks_resume_ready_and_recover_needed() {
        let home = temp_runtime_home("materialize");
        let snapshot = StartupTopologySnapshot {
            updated_at: "2026-04-20T20:00:00+08:00".into(),
            entry_role: "system".into(),
            local_worker_budget: 4,
            projects: vec![
                ProjectRegistryEntry {
                    project_id: "fin".into(),
                    agent_id: "mbp.builder".into(),
                    mode: "local".into(),
                    project_root: Some("/tmp/fin".into()),
                    endpoint: None,
                    always_on: true,
                    auto_resume: true,
                    auto_connect: true,
                    worker_budget: 2,
                    unfinished_task_count: 1,
                    last_active_task_id: Some("task-fin-1".into()),
                    presence_state: "idle".into(),
                    wake_state: "steady".into(),
                    wake_reason: None,
                    updated_at: "2026-04-20T20:00:00+08:00".into(),
                },
                ProjectRegistryEntry {
                    project_id: "remote".into(),
                    agent_id: "mbp.remote".into(),
                    mode: "remote".into(),
                    project_root: None,
                    endpoint: Some("tcp://10.0.0.2:7000".into()),
                    always_on: false,
                    auto_resume: true,
                    auto_connect: true,
                    worker_budget: 1,
                    unfinished_task_count: 2,
                    last_active_task_id: Some("task-remote-1".into()),
                    presence_state: "offline".into(),
                    wake_state: "wake_requested".into(),
                    wake_reason: Some("unfinished_work_detected".into()),
                    updated_at: "2026-04-20T20:00:00+08:00".into(),
                },
            ],
            wake_queue: Vec::new(),
        };

        let supervision = materialize_project_supervision(&home, &snapshot).expect("supervision");
        assert_eq!(supervision.resume_ready_count, 1);
        assert_eq!(supervision.recover_needed_count, 1);
        assert_eq!(
            supervision.projects[0].desired_action,
            "resume_project_task"
        );
        assert_eq!(
            supervision.projects[1].desired_action,
            "recover_project_agent"
        );
        assert!(
            home.join("runtime/current/current_project_supervision.json")
                .exists()
        );
    }
}
