use crate::CliError;
use crate::startup_topology::StartupTopologySnapshot;
use crate::startup_wakeup::StartupWakeReport;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use std::{fs, path::Path};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct StartupControlSummary {
    #[serde(default)]
    pub(crate) updated_at: String,
    #[serde(default)]
    pub(crate) local_worker_budget: usize,
    pub(crate) project_count: usize,
    #[serde(default)]
    pub(crate) configured_project_worker_budget: usize,
    pub(crate) wake_queue_count: usize,
    pub(crate) waiting_project_count: usize,
    pub(crate) recoverable_offline_count: usize,
    pub(crate) last_wake_action_count: usize,
    pub(crate) last_wake_had_remote_wait: bool,
    #[serde(default)]
    pub(crate) started_resource_count: usize,
    #[serde(default)]
    pub(crate) busy_resource_count: usize,
    #[serde(default)]
    pub(crate) started_resources: Vec<String>,
    #[serde(default)]
    pub(crate) busy_resources: Vec<String>,
    #[serde(default)]
    pub(crate) startup_config_summary: String,
    #[serde(default)]
    pub(crate) startup_state_summary: String,
}

impl StartupControlSummary {
    pub(crate) fn status_summary(&self) -> String {
        format!(
            "cfg(system_workers={} project_workers={} projects={}) state(started={} busy={} waiting={} recoverable_offline={} wake_actions={})",
            self.local_worker_budget,
            self.configured_project_worker_budget,
            self.project_count,
            self.started_resource_count,
            self.busy_resource_count,
            self.waiting_project_count,
            self.recoverable_offline_count,
            self.last_wake_action_count,
        )
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
struct AgentPresenceRegistry {
    #[serde(default)]
    agents: Vec<AgentPresenceEntry>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
struct AgentPresenceEntry {
    #[serde(default)]
    agent_id: String,
    #[serde(default)]
    device_name: String,
    #[serde(default)]
    agent_name: String,
    #[serde(default)]
    status: String,
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
        summary.updated_at = topology.updated_at.clone();
        summary.local_worker_budget = topology.local_worker_budget;
        summary.project_count = topology.projects.len();
        summary.configured_project_worker_budget = topology
            .projects
            .iter()
            .map(|project| project.worker_budget)
            .sum();
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
        if summary.updated_at.is_empty() {
            summary.updated_at = wake_report.executed_at.clone();
        }
        summary.last_wake_action_count = wake_report.actions.len();
        summary.last_wake_had_remote_wait = wake_report
            .actions
            .iter()
            .any(|item| item.mode == "remote" && item.status == "recorded");
    }
    if let Some(registry) = read_json_optional::<AgentPresenceRegistry>(
        &runtime_home.join("runtime/current/current_agent_presence_registry.json"),
    )? {
        for agent in registry.agents {
            if matches!(agent.status.as_str(), "busy" | "idle" | "waiting") {
                summary.started_resource_count += 1;
                let label = agent_display_label(&agent);
                summary
                    .started_resources
                    .push(format!("{label}:{}", agent.status));
                if agent.status == "busy" {
                    summary.busy_resource_count += 1;
                    summary.busy_resources.push(label);
                }
            }
        }
        summary.started_resources.sort();
        summary.started_resources.dedup();
        summary.busy_resources.sort();
        summary.busy_resources.dedup();
    }
    summary.startup_config_summary = format!(
        "startup config · system_workers={} · project_workers={} · projects={}",
        summary.local_worker_budget,
        summary.configured_project_worker_budget,
        summary.project_count
    );
    summary.startup_state_summary = format!(
        "startup state · started={} [{}] · busy={} [{}] · waiting={} · wake_actions={}",
        summary.started_resource_count,
        preview_list(summary.started_resources.as_slice(), 4),
        summary.busy_resource_count,
        preview_list(summary.busy_resources.as_slice(), 3),
        summary.waiting_project_count,
        summary.last_wake_action_count
    );
    Ok(summary)
}

pub(crate) fn persist_startup_control_summary(
    runtime_home: &Path,
    summary: &StartupControlSummary,
) -> Result<(), CliError> {
    write_json(
        &runtime_home.join("runtime/current/current_startup_control_summary.json"),
        summary,
    )?;
    write_json(
        &runtime_home.join("runtime/projects/startup_control_summary.json"),
        summary,
    )
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

fn agent_display_label(agent: &AgentPresenceEntry) -> String {
    if !agent.device_name.trim().is_empty() && !agent.agent_name.trim().is_empty() {
        format!("{}.{}", agent.device_name, agent.agent_name)
    } else if !agent.agent_id.trim().is_empty() {
        agent.agent_id.clone()
    } else {
        "unknown-agent".into()
    }
}

fn preview_list(items: &[String], limit: usize) -> String {
    if items.is_empty() {
        return "none".into();
    }
    let preview = items.iter().take(limit).cloned().collect::<Vec<_>>();
    let overflow = items.len().saturating_sub(preview.len());
    if overflow > 0 {
        format!("{} (+{})", preview.join(", "), overflow)
    } else {
        preview.join(", ")
    }
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
        let path = std::env::temp_dir().join(format!("fin-startup-summary-{prefix}-{unique}"));
        fs::create_dir_all(path.join("runtime/current")).expect("runtime current");
        path
    }

    #[test]
    fn startup_summary_includes_config_started_and_busy_resources() {
        let home = temp_runtime_home("summary");
        fs::write(
            home.join("runtime/current/current_startup_topology.json"),
            br#"{
  "updated_at":"2026-04-20T23:00:00+08:00",
  "entry_role":"system",
  "local_worker_budget":4,
  "projects":[
    {
      "project_id":"fin",
      "agent_id":"mbp.builder",
      "mode":"local",
      "project_root":"/tmp/fin",
      "endpoint":null,
      "always_on":true,
      "auto_resume":true,
      "auto_connect":true,
      "worker_budget":2,
      "unfinished_task_count":1,
      "last_active_task_id":"task-fin-1",
      "presence_state":"idle",
      "wake_state":"steady",
      "wake_reason":null,
      "updated_at":"2026-04-20T23:00:00+08:00"
    }
  ],
  "wake_queue":[]
}"#,
        )
        .expect("topology");
        fs::write(
            home.join("runtime/current/current_startup_wakeup.json"),
            br#"{"executed_at":"2026-04-20T23:00:01+08:00","actions":[{"action_id":"wake-fin","project_id":"fin","agent_id":"mbp.builder","mode":"local","status":"completed","reason":"always_on_startup","summary":"ready","executed_at":"2026-04-20T23:00:01+08:00"}]}"#,
        )
        .expect("wakeup");
        fs::write(
            home.join("runtime/current/current_agent_presence_registry.json"),
            br#"{"agents":[
              {"agent_id":"mbp.system-worker-01","device_name":"mbp","agent_name":"system-worker-01","status":"idle"},
              {"agent_id":"mbp.builder","device_name":"mbp","agent_name":"builder","status":"busy"}
            ]}"#,
        )
        .expect("presence");

        let summary = read_startup_control_summary(&home).expect("startup summary");
        assert_eq!(summary.local_worker_budget, 4);
        assert_eq!(summary.configured_project_worker_budget, 2);
        assert_eq!(summary.started_resource_count, 2);
        assert_eq!(summary.busy_resource_count, 1);
        assert!(summary.startup_config_summary.contains("system_workers=4"));
        assert!(summary.startup_state_summary.contains("mbp.builder"));
        assert!(summary.status_summary().contains("started=2"));
    }
}
