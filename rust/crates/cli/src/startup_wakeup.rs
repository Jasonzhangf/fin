use crate::{
    CliError,
    agent_presence::{
        mark_project_agent_waiting_remote, mark_project_agent_woken_local, project_agent_name,
    },
    project_execution_handoff::materialize_project_execution_handoffs,
    project_runtime_pickup::materialize_project_runtime_pickups,
    project_supervision::materialize_project_supervision,
    startup_control_summary::{persist_startup_control_summary, read_startup_control_summary},
    startup_daemon_ensure::{complete_daemon_ensure_requests, merge_daemon_ensure_wake_requests},
    startup_topology::{ProjectWakeRequest, StartupTopologySnapshot, materialize_startup_topology},
};
use fin_config::{ProjectAgentMode, ProjectAgentStartupConfig, SystemConfig};
use fin_runtime::resolve_device_name;
use serde::{Deserialize, Serialize};
use std::{fs, path::Path};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct StartupWakeAction {
    pub(crate) action_id: String,
    #[serde(default)]
    pub(crate) wake_request_id: Option<String>,
    pub(crate) project_id: String,
    pub(crate) agent_id: String,
    pub(crate) mode: String,
    pub(crate) status: String,
    pub(crate) reason: String,
    pub(crate) summary: String,
    pub(crate) executed_at: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct StartupWakeReport {
    pub(crate) executed_at: String,
    #[serde(default)]
    pub(crate) actions: Vec<StartupWakeAction>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct ManagedPeerStateRecord {
    peer_id: String,
    peer_kind: String,
    lifecycle_state: String,
    last_heartbeat_at: String,
    reconnect_backoff_ms: u64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
struct PeerRegistry {
    #[serde(default)]
    peers: Vec<PeerRegistryEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct PeerRegistryEntry {
    peer_id: String,
    peer_kind: String,
    #[serde(default)]
    agent_name: Option<String>,
    #[serde(default)]
    display_name: Option<String>,
    #[serde(default)]
    device_name: Option<String>,
    #[serde(default)]
    endpoint: Option<String>,
    presence_state: String,
    #[serde(default)]
    runtime_state: Option<String>,
    #[serde(default)]
    connectivity_state: Option<String>,
    #[serde(default)]
    binding_state: Option<String>,
    lifecycle_state: String,
    updated_at: String,
    #[serde(default)]
    last_heartbeat_at: Option<String>,
}

pub(crate) fn refresh_startup_control_plane(
    runtime_home: &Path,
    system: &SystemConfig,
    updated_at: &str,
) -> Result<StartupTopologySnapshot, CliError> {
    let snapshot = merge_daemon_ensure_wake_requests(
        runtime_home,
        system,
        materialize_startup_topology(runtime_home, system, updated_at)?,
        updated_at,
    )?;
    let _ = execute_wake_queue(runtime_home, system, &snapshot, updated_at)?;
    let final_snapshot = materialize_startup_topology(runtime_home, system, updated_at)?;
    let supervision = materialize_project_supervision(runtime_home, &final_snapshot)?;
    let _ = materialize_project_execution_handoffs(runtime_home, &supervision, updated_at)?;
    let _ = materialize_project_runtime_pickups(runtime_home, system, updated_at)?;
    let summary = read_startup_control_summary(runtime_home)?;
    persist_startup_control_summary(runtime_home, &summary)?;
    Ok(final_snapshot)
}

pub(crate) fn execute_wake_queue(
    runtime_home: &Path,
    system: &SystemConfig,
    snapshot: &StartupTopologySnapshot,
    executed_at: &str,
) -> Result<StartupWakeReport, CliError> {
    let mut actions = Vec::new();
    for request in &snapshot.wake_queue {
        let Some(project) = project_config(system, request.project_id.as_str()) else {
            continue;
        };
        let action = execute_single_wake(runtime_home, system, project, request, executed_at)?;
        actions.push(action);
    }
    let report = StartupWakeReport {
        executed_at: executed_at.into(),
        actions,
    };
    let ensure_request_ids = report
        .actions
        .iter()
        .filter_map(|item| item.wake_request_id.clone())
        .collect::<Vec<_>>();
    complete_daemon_ensure_requests(runtime_home, ensure_request_ids.as_slice(), executed_at)?;
    persist_report(runtime_home, &report)?;
    Ok(report)
}

fn execute_single_wake(
    runtime_home: &Path,
    system: &SystemConfig,
    project: &ProjectAgentStartupConfig,
    request: &ProjectWakeRequest,
    executed_at: &str,
) -> Result<StartupWakeAction, CliError> {
    match project.mode {
        ProjectAgentMode::Local => {
            execute_local_wake(runtime_home, system, project, request, executed_at)
        }
        ProjectAgentMode::Remote => {
            execute_remote_wake(runtime_home, system, project, request, executed_at)
        }
    }
}

fn execute_local_wake(
    runtime_home: &Path,
    system: &SystemConfig,
    project: &ProjectAgentStartupConfig,
    request: &ProjectWakeRequest,
    executed_at: &str,
) -> Result<StartupWakeAction, CliError> {
    let summary = if request.auto_resume {
        "local project agent wake executed; resume-ready and awaiting framework scheduling"
    } else {
        "local project agent wake executed; ready and awaiting framework scheduling"
    };
    let _ = mark_project_agent_woken_local(
        system,
        runtime_home,
        project,
        request.resume_task_id.as_deref(),
        executed_at,
        summary,
    )?;
    ensure_managed_peer(runtime_home, system, project, "online", executed_at)?;
    Ok(StartupWakeAction {
        action_id: format!(
            "startup-wake-{}-{}",
            project.project_id,
            sanitize_id(executed_at)
        ),
        wake_request_id: Some(request.request_id.clone()),
        project_id: project.project_id.clone(),
        agent_id: request.agent_id.clone(),
        mode: "local".into(),
        status: "completed".into(),
        reason: request.reason.clone(),
        summary: summary.into(),
        executed_at: executed_at.into(),
    })
}

fn execute_remote_wake(
    runtime_home: &Path,
    system: &SystemConfig,
    project: &ProjectAgentStartupConfig,
    request: &ProjectWakeRequest,
    executed_at: &str,
) -> Result<StartupWakeAction, CliError> {
    let summary = "remote project agent wake recorded; awaiting remote peer connect/reconnect path";
    let _ = mark_project_agent_waiting_remote(
        system,
        runtime_home,
        project,
        request.resume_task_id.as_deref(),
        executed_at,
        summary,
    )?;
    ensure_managed_peer(
        runtime_home,
        system,
        project,
        "waiting_remote_connect",
        executed_at,
    )?;
    Ok(StartupWakeAction {
        action_id: format!(
            "startup-wake-{}-{}",
            project.project_id,
            sanitize_id(executed_at)
        ),
        wake_request_id: Some(request.request_id.clone()),
        project_id: project.project_id.clone(),
        agent_id: request.agent_id.clone(),
        mode: "remote".into(),
        status: "recorded".into(),
        reason: request.reason.clone(),
        summary: summary.into(),
        executed_at: executed_at.into(),
    })
}

fn ensure_managed_peer(
    runtime_home: &Path,
    system: &SystemConfig,
    project: &ProjectAgentStartupConfig,
    lifecycle_state: &str,
    executed_at: &str,
) -> Result<(), CliError> {
    let peer_id = format!("peer-project-agent-{}", project.project_id);
    let agent_name = project_agent_name(system, project);
    let device_name = resolve_device_name(system);
    let display_name = agent_display_name(
        device_name.as_str(),
        agent_name.as_str(),
        &peer_id,
        project.endpoint.as_deref(),
        matches!(project.mode, ProjectAgentMode::Remote),
    );
    let state = ManagedPeerStateRecord {
        peer_id: peer_id.clone(),
        peer_kind: "project_agent".into(),
        lifecycle_state: lifecycle_state.into(),
        last_heartbeat_at: executed_at.into(),
        reconnect_backoff_ms: 0,
    };
    write_json(
        &runtime_home.join(format!("runtime/peers/state/{peer_id}.json")),
        &state,
    )?;

    let registry_path = runtime_home.join("runtime/peers/registry.json");
    let mut registry = read_json::<PeerRegistry>(&registry_path)?.unwrap_or_default();
    if let Some(existing) = registry
        .peers
        .iter_mut()
        .find(|entry| entry.peer_id == peer_id)
    {
        existing.peer_kind = "project_agent".into();
        existing.agent_name = Some(agent_name.clone());
        existing.display_name = Some(display_name.clone());
        existing.device_name = Some(device_name.clone());
        existing.endpoint = project.endpoint.clone();
        existing.presence_state = if lifecycle_state == "online" {
            "online".into()
        } else {
            "waiting".into()
        };
        existing.runtime_state = Some(lifecycle_state.into());
        existing.connectivity_state = Some(if lifecycle_state == "online" {
            "local_ready".into()
        } else {
            "remote_pending".into()
        });
        existing.binding_state = Some("project_managed".into());
        existing.lifecycle_state = lifecycle_state.into();
        existing.updated_at = executed_at.into();
        existing.last_heartbeat_at = Some(executed_at.into());
    } else {
        registry.peers.push(PeerRegistryEntry {
            peer_id,
            peer_kind: "project_agent".into(),
            agent_name: Some(agent_name.clone()),
            display_name: Some(display_name),
            device_name: Some(device_name),
            endpoint: project.endpoint.clone(),
            presence_state: if lifecycle_state == "online" {
                "online".into()
            } else {
                "waiting".into()
            },
            runtime_state: Some(lifecycle_state.into()),
            connectivity_state: Some(if lifecycle_state == "online" {
                "local_ready".into()
            } else {
                "remote_pending".into()
            }),
            binding_state: Some("project_managed".into()),
            lifecycle_state: lifecycle_state.into(),
            updated_at: executed_at.into(),
            last_heartbeat_at: Some(executed_at.into()),
        });
    }
    write_json(&registry_path, &registry)
}

fn agent_display_name(
    local_device_name: &str,
    agent_name: &str,
    peer_id: &str,
    endpoint: Option<&str>,
    remote: bool,
) -> String {
    if remote {
        let remote_prefix = endpoint
            .and_then(endpoint_host)
            .or_else(|| peer_id.split_once('.').map(|(device, _)| device))
            .unwrap_or(local_device_name);
        return format!("{remote_prefix}.{agent_name}");
    }
    let peer_device = peer_id
        .split_once('.')
        .map(|(device, _)| device)
        .unwrap_or(local_device_name);
    if peer_device == local_device_name {
        agent_name.into()
    } else {
        format!("{peer_device}.{agent_name}")
    }
}

fn endpoint_host(endpoint: &str) -> Option<&str> {
    let without_scheme = endpoint
        .split_once("://")
        .map(|(_, rest)| rest)
        .unwrap_or(endpoint);
    let host_port = without_scheme.split('/').next().unwrap_or(without_scheme);
    let host = host_port
        .rsplit_once(':')
        .map(|(host, _)| host)
        .unwrap_or(host_port);
    let trimmed = host.trim_matches(|ch| ch == '[' || ch == ']');
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed)
    }
}

fn persist_report(runtime_home: &Path, report: &StartupWakeReport) -> Result<(), CliError> {
    write_json(
        &runtime_home.join("runtime/projects/wake_actions.json"),
        &report.actions,
    )?;
    write_json(
        &runtime_home.join("runtime/current/current_startup_wakeup.json"),
        report,
    )
}

fn project_config<'a>(
    system: &'a SystemConfig,
    project_id: &str,
) -> Option<&'a ProjectAgentStartupConfig> {
    system
        .runtime
        .startup
        .project_agents
        .iter()
        .find(|item| item.project_id == project_id)
}

fn sanitize_id(raw: &str) -> String {
    raw.chars()
        .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '-' })
        .collect::<String>()
        .trim_matches('-')
        .to_string()
}

fn read_json<T: for<'de> Deserialize<'de>>(path: &Path) -> Result<Option<T>, CliError> {
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
#[path = "startup_wakeup_tests.rs"]
mod tests;
