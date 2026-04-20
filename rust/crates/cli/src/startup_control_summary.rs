use crate::CliError;
use crate::startup_topology::StartupTopologySnapshot;
use crate::startup_wakeup::StartupWakeReport;
use serde::de::DeserializeOwned;
use std::{fs, path::Path};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct StartupControlSummary {
    pub(crate) project_count: usize,
    pub(crate) wake_queue_count: usize,
    pub(crate) waiting_project_count: usize,
    pub(crate) recoverable_offline_count: usize,
    pub(crate) last_wake_action_count: usize,
    pub(crate) last_wake_had_remote_wait: bool,
}

impl StartupControlSummary {
    pub(crate) fn status_summary(&self) -> String {
        format!(
            "projects={} wake_queue={} waiting={} recoverable_offline={} wake_actions={}",
            self.project_count,
            self.wake_queue_count,
            self.waiting_project_count,
            self.recoverable_offline_count,
            self.last_wake_action_count
        )
    }
}

pub(crate) fn read_startup_control_summary(
    runtime_home: &Path,
) -> Result<StartupControlSummary, CliError> {
    let topology = read_json_optional::<StartupTopologySnapshot>(
        &runtime_home.join("runtime/current/current_startup_topology.json"),
    )?;
    let wake_report = read_json_optional::<StartupWakeReport>(
        &runtime_home.join("runtime/current/current_startup_wakeup.json"),
    )?;

    let mut summary = StartupControlSummary::default();
    if let Some(topology) = topology {
        summary.project_count = topology.projects.len();
        summary.wake_queue_count = topology.wake_queue.len();
        for project in topology.projects {
            if project.presence_state == "waiting" {
                summary.waiting_project_count += 1;
            }
            if project.presence_state == "offline"
                && (project.always_on || project.unfinished_task_count > 0)
            {
                summary.recoverable_offline_count += 1;
            }
        }
    }
    if let Some(wake_report) = wake_report {
        summary.last_wake_action_count = wake_report.actions.len();
        summary.last_wake_had_remote_wait = wake_report
            .actions
            .iter()
            .any(|item| item.mode == "remote" && item.status == "recorded");
    }
    Ok(summary)
}

fn read_json_optional<T: DeserializeOwned>(path: &Path) -> Result<Option<T>, CliError> {
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
