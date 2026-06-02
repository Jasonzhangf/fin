use crate::{
    CliError,
    startup_topology::{ProjectWakeRequest, StartupTopologySnapshot},
};
use fin_config::SystemConfig;
use fin_contracts::DaemonEnsurePeerRequestRecord;
use serde::Serialize;
use std::{fs, path::Path};

pub(crate) fn merge_daemon_ensure_wake_requests(
    runtime_home: &Path,
    system: &SystemConfig,
    mut snapshot: StartupTopologySnapshot,
    updated_at: &str,
) -> Result<StartupTopologySnapshot, CliError> {
    let mut requests = read_requests(runtime_home)?;
    for request in requests.iter_mut().filter(|item| item.status == "pending") {
        let Some(project_id) = request_project_id(request) else {
            continue;
        };
        let Some(project) = snapshot
            .projects
            .iter()
            .find(|item| item.project_id == project_id)
        else {
            continue;
        };
        if matches!(project.presence_state.as_str(), "busy" | "idle" | "waiting") {
            request.status = "completed".into();
            request.consumed_at = Some(updated_at.into());
            continue;
        }
        if snapshot
            .wake_queue
            .iter()
            .any(|item| item.request_id == request.request_id)
        {
            continue;
        }
        let auto_resume = system
            .runtime
            .startup
            .project_agents
            .iter()
            .find(|item| item.project_id == project_id)
            .map(|item| item.auto_resume)
            .unwrap_or(false);
        snapshot.wake_queue.push(ProjectWakeRequest {
            request_id: request.request_id.clone(),
            project_id: project_id.to_string(),
            agent_id: project.agent_id.clone(),
            reason: "daemon.ensure_peer_requested".into(),
            resume_task_id: project.last_active_task_id.clone(),
            requested_by: "daemon.ensure_peer".into(),
            auto_resume,
            created_at: updated_at.into(),
        });
    }
    write_requests(runtime_home, &requests)?;
    Ok(snapshot)
}

pub(crate) fn complete_daemon_ensure_requests(
    runtime_home: &Path,
    request_ids: &[String],
    completed_at: &str,
) -> Result<(), CliError> {
    if request_ids.is_empty() {
        return Ok(());
    }
    let mut requests = read_requests(runtime_home)?;
    if requests.is_empty() {
        return Ok(());
    }
    for request in requests
        .iter_mut()
        .filter(|item| request_ids.iter().any(|id| id == &item.request_id))
    {
        request.status = "completed".into();
        request.consumed_at = Some(completed_at.into());
    }
    write_requests(runtime_home, &requests)
}

fn request_project_id<'a>(request: &'a DaemonEnsurePeerRequestRecord) -> Option<&'a str> {
    request.project_id.as_deref().or_else(|| {
        request
            .peer_id
            .strip_prefix("peer-project-agent-")
            .filter(|value| !value.trim().is_empty())
    })
}

fn read_requests(runtime_home: &Path) -> Result<Vec<DaemonEnsurePeerRequestRecord>, CliError> {
    let path = runtime_home.join("runtime/peers/ensure_requests.json");
    match fs::read_to_string(&path) {
        Ok(content) => serde_json::from_str(&content).map_err(CliError::Serialize),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(source) => Err(CliError::ReadFile {
            path: path.display().to_string(),
            source,
        }),
    }
}

fn write_requests(
    runtime_home: &Path,
    requests: &[DaemonEnsurePeerRequestRecord],
) -> Result<(), CliError> {
    let index_path = runtime_home.join("runtime/peers/ensure_requests.json");
    write_json(&index_path, requests)?;
    let dir = runtime_home.join("runtime/peers/ensure_requests");
    fs::create_dir_all(&dir).map_err(|source| CliError::WriteFile {
        path: dir.display().to_string(),
        source,
    })?;
    for request in requests {
        write_json(&dir.join(format!("{}.json", request.peer_id)), request)?;
    }
    write_json(
        &runtime_home.join("runtime/current/current_daemon_ensure_requests.json"),
        requests,
    )
}

fn write_json<T: Serialize + ?Sized>(path: &Path, value: &T) -> Result<(), CliError> {
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
