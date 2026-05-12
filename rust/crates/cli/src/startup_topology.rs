use crate::{
    CliError,
    agent_presence::{ensure_project_agent_presence, ensure_system_worker_pool, project_agent_id},
    startup_project_task_scan::{ProjectTaskScan, scan_project_tasks},
};
use fin_config::{ProjectAgentMode, ProjectAgentStartupConfig, SystemConfig};
use serde::{Deserialize, Serialize};
use std::{fs, path::Path};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct StartupTopologySnapshot {
    pub(crate) updated_at: String,
    pub(crate) entry_role: String,
    pub(crate) local_worker_budget: usize,
    #[serde(default)]
    pub(crate) projects: Vec<ProjectRegistryEntry>,
    #[serde(default)]
    pub(crate) wake_queue: Vec<ProjectWakeRequest>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct ProjectRegistryEntry {
    pub(crate) project_id: String,
    pub(crate) agent_id: String,
    pub(crate) mode: String,
    #[serde(default)]
    pub(crate) project_root: Option<String>,
    #[serde(default)]
    pub(crate) endpoint: Option<String>,
    pub(crate) always_on: bool,
    pub(crate) auto_resume: bool,
    pub(crate) auto_connect: bool,
    pub(crate) worker_budget: usize,
    pub(crate) unfinished_task_count: usize,
    #[serde(default)]
    pub(crate) last_active_task_id: Option<String>,
    pub(crate) presence_state: String,
    pub(crate) wake_state: String,
    #[serde(default)]
    pub(crate) wake_reason: Option<String>,
    pub(crate) updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct ProjectWakeRequest {
    pub(crate) request_id: String,
    pub(crate) project_id: String,
    pub(crate) agent_id: String,
    pub(crate) reason: String,
    #[serde(default)]
    pub(crate) resume_task_id: Option<String>,
    pub(crate) requested_by: String,
    pub(crate) auto_resume: bool,
    pub(crate) created_at: String,
}

pub(crate) fn materialize_startup_topology(
    runtime_home: &Path,
    system: &SystemConfig,
    updated_at: &str,
) -> Result<StartupTopologySnapshot, CliError> {
    let mut projects = Vec::new();
    let mut wake_queue = Vec::new();
    let scan = scan_project_tasks(runtime_home)?;
    let _ = ensure_system_worker_pool(system, runtime_home, updated_at)?;

    for project in &system.runtime.startup.project_agents {
        let presence = ensure_project_agent_presence(system, runtime_home, project, updated_at)?;
        let project_scan = scan
            .iter()
            .find(|(project_id, _)| project_id == &project.project_id)
            .map(|(_, value)| value.clone())
            .unwrap_or_default();
        let agent_id = project_agent_id(system, project);
        let wake_reason = derive_wake_reason(project, &presence.status, &project_scan);
        if let Some(reason) = wake_reason.as_deref() {
            wake_queue.push(ProjectWakeRequest {
                request_id: format!("wake-{}-{}", project.project_id, sanitize_id(updated_at)),
                project_id: project.project_id.clone(),
                agent_id: agent_id.clone(),
                reason: reason.into(),
                resume_task_id: project_scan.last_active_task_id.clone(),
                requested_by: "framework.startup".into(),
                auto_resume: project.auto_resume,
                created_at: updated_at.into(),
            });
        }
        projects.push(ProjectRegistryEntry {
            project_id: project.project_id.clone(),
            agent_id,
            mode: project_mode_name(project),
            project_root: project.project_root.clone(),
            endpoint: project.endpoint.clone(),
            always_on: project.always_on,
            auto_resume: project.auto_resume,
            auto_connect: project.auto_connect,
            worker_budget: project.worker_budget,
            unfinished_task_count: project_scan.unfinished_task_count,
            last_active_task_id: project_scan.last_active_task_id,
            presence_state: presence.status,
            wake_state: wake_reason
                .as_ref()
                .map(|_| "wake_requested".into())
                .unwrap_or_else(|| "steady".into()),
            wake_reason,
            updated_at: updated_at.into(),
        });
    }

    let snapshot = StartupTopologySnapshot {
        updated_at: updated_at.into(),
        entry_role: system.policy.entry_role.clone(),
        local_worker_budget: system.runtime.startup.system_agent.local_worker_budget,
        projects,
        wake_queue,
    };
    persist_snapshot(runtime_home, &snapshot)?;
    Ok(snapshot)
}

pub(crate) fn render_project_registry_summary(runtime_home: &Path) -> Result<String, CliError> {
    let path = runtime_home.join("runtime/projects/registry.json");
    let Some(projects) = read_json_optional::<Vec<ProjectRegistryEntry>>(&path)? else {
        return Ok("projects=none".into());
    };
    if projects.is_empty() {
        return Ok("projects=none".into());
    }
    let names = projects
        .iter()
        .map(|item| {
            format!(
                "{}:{}:{}",
                item.project_id, item.presence_state, item.wake_state
            )
        })
        .collect::<Vec<_>>()
        .join(", ");
    Ok(format!("projects={} [{}]", projects.len(), names))
}

fn derive_wake_reason(
    project: &ProjectAgentStartupConfig,
    presence_state: &str,
    scan: &ProjectTaskScan,
) -> Option<String> {
    if project.always_on && !matches!(presence_state, "busy" | "idle" | "waiting") {
        return Some("always_on_startup".into());
    }
    if scan.unfinished_task_count > 0 && !matches!(presence_state, "busy" | "idle" | "waiting") {
        return Some("unfinished_work_detected".into());
    }
    None
}

fn project_mode_name(project: &ProjectAgentStartupConfig) -> String {
    match project.mode {
        ProjectAgentMode::Local => "local".into(),
        ProjectAgentMode::Remote => "remote".into(),
    }
}

fn persist_snapshot(
    runtime_home: &Path,
    snapshot: &StartupTopologySnapshot,
) -> Result<(), CliError> {
    write_json(
        &runtime_home.join("runtime/projects/registry.json"),
        &snapshot.projects,
    )?;
    write_json(
        &runtime_home.join("runtime/projects/wake_queue.json"),
        &snapshot.wake_queue,
    )?;
    for project in &snapshot.projects {
        write_json(
            &runtime_home.join(format!(
                "runtime/projects/state/{}.json",
                project.project_id
            )),
            project,
        )?;
    }
    write_json(
        &runtime_home.join("runtime/current/current_startup_topology.json"),
        snapshot,
    )
}

fn sanitize_id(raw: &str) -> String {
    raw.chars()
        .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '-' })
        .collect::<String>()
        .trim_matches('-')
        .to_string()
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
    use fin_config::{
        ConfigMapper, ProviderProtocol, UserConfig, UserProviderConfig, UserRuntimeConfig,
    };
    use fin_contracts::ExecutionStateRecord;
    use serde_json::{Value, json};
    use std::{collections::BTreeMap, path::PathBuf};

    fn system() -> SystemConfig {
        let mut system = ConfigMapper::map_user_to_system(&UserConfig {
            default_provider: "openai".into(),
            providers: BTreeMap::from([(
                "openai".into(),
                UserProviderConfig {
                    protocol: ProviderProtocol::OpenAiCompatible,
                    base_url: "https://api.example.com/v1".into(),
                    model: "gpt-5".into(),
                    api_key: None,
                    api_key_env: Some("OPENAI_API_KEY".into()),
                    user_agent: None,
                    headers: BTreeMap::new(),
                },
            )]),
            runtime: UserRuntimeConfig::default(),
        })
        .expect("system");
        system
            .runtime
            .startup
            .project_agents
            .push(ProjectAgentStartupConfig {
                project_id: "fin".into(),
                mode: ProjectAgentMode::Local,
                project_root: Some("/tmp/fin".into()),
                endpoint: None,
                agent_name: Some("builder".into()),
                worker_budget: 2,
                always_on: true,
                auto_resume: true,
                auto_connect: true,
            });
        system
    }

    fn temp_home(prefix: &str) -> PathBuf {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system time")
            .as_nanos();
        let path = std::env::temp_dir().join(format!("fin-startup-topology-{prefix}-{unique}"));
        fs::create_dir_all(&path).expect("home");
        path
    }

    #[test]
    fn materialize_writes_always_on_project_and_wake_queue() {
        let home = temp_home("always-on");
        let session_dir = home.join("sessions/2026/05/session-fin");
        fs::create_dir_all(session_dir.join("context")).expect("context");
        fs::create_dir_all(session_dir.join("control")).expect("control");
        fs::write(
            session_dir.join("context/current_context.json"),
            serde_json::to_vec_pretty(&json!({
                "project": {
                    "primary_project": {
                        "project_id": "fin"
                    }
                }
            }))
            .expect("json"),
        )
        .expect("context");
        fs::write(
            session_dir.join("control/execution_state.json"),
            serde_json::to_vec_pretty(&ExecutionStateRecord {
                refs: fin_contracts::EntityRefs {
                    task_id: Some("task-fin-1".into()),
                    ..Default::default()
                },
                status: "running".into(),
                pending_input_count: 0,
                ..Default::default()
            })
            .expect("json"),
        )
        .expect("state");

        let snapshot = materialize_startup_topology(&home, &system(), "2026-04-20T10:00:00+08:00")
            .expect("snapshot");
        assert_eq!(snapshot.projects.len(), 1);
        assert_eq!(snapshot.local_worker_budget, 4);
        assert_eq!(snapshot.projects[0].unfinished_task_count, 1);
        assert_eq!(snapshot.projects[0].wake_state, "wake_requested");
        assert_eq!(snapshot.wake_queue.len(), 1);
        assert!(home.join("runtime/projects/registry.json").exists());
        assert!(home.join("runtime/projects/wake_queue.json").exists());

        let presence_registry =
            fs::read_to_string(home.join("runtime/current/current_agent_presence_registry.json"))
                .expect("presence registry");
        let presence_json: Value =
            serde_json::from_str(&presence_registry).expect("presence registry json");
        let agents = presence_json["agents"]
            .as_array()
            .expect("presence registry agents");
        assert_eq!(
            agents
                .iter()
                .filter(|item| item["agent_kind"] == "system_worker")
                .count(),
            4
        );
        assert_eq!(
            agents
                .iter()
                .filter(|item| item["agent_kind"] == "project_worker")
                .count(),
            2
        );
    }
}
