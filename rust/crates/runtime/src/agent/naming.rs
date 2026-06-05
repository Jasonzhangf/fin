use crate::{RuntimeError, WorkerRuntime};
use fin_config::SystemConfig;
use serde::{Deserialize, Serialize};
use std::{env, fs, path::Path, process::Command};

const DEFAULT_AGENT_NAMES: &[&str] = &[
    "atlas", "nova", "ember", "aurora", "kepler", "luna", "onyx", "iris",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AllocatedAgentIdentity {
    pub device_name: String,
    pub agent_name: String,
    pub agent_id: String,
    pub worker_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentAssignmentSummary {
    pub assignment_id: String,
    pub worker_id: String,
    pub peer_id: String,
    #[serde(default)]
    pub target_agent_id: Option<String>,
    #[serde(default)]
    pub target_agent_name: Option<String>,
    pub requested_role_id: String,
    #[serde(default)]
    pub owner_worker_id: Option<String>,
    pub task_summary: String,
    pub status: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
struct AgentNamePool {
    #[serde(default)]
    names: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
struct AgentRegistry {
    #[serde(default)]
    agents: Vec<RegisteredAgentIdentity>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct RegisteredAgentIdentity {
    agent_id: String,
    agent_name: String,
    device_name: String,
    worker_id: String,
    role_id: String,
    source: String,
}

pub fn create_named_local_worker(
    system: &SystemConfig,
    runtime_home: &Path,
    requested_agent_name: Option<&str>,
    source: impl Into<String>,
    role_id: Option<&str>,
) -> Result<WorkerRuntime, RuntimeError> {
    let source = source.into();
    let identity = allocate_local_agent_identity(
        system,
        runtime_home,
        requested_agent_name,
        source.as_str(),
        role_id,
    )?;
    WorkerRuntime::from_system(
        system,
        identity.agent_id,
        identity.worker_id,
        source,
        role_id,
    )
}

pub fn allocate_local_agent_identity(
    system: &SystemConfig,
    runtime_home: &Path,
    requested_agent_name: Option<&str>,
    source: &str,
    role_id: Option<&str>,
) -> Result<AllocatedAgentIdentity, RuntimeError> {
    let resolved_role_id = system.role_profile(role_id)?.0;
    let device_name = resolve_device_name(system);
    let requested_agent_name = requested_agent_name.and_then(sanitize_name_part);

    let agents_dir = runtime_home.join("runtime/agents");
    fs::create_dir_all(&agents_dir).map_err(|source| RuntimeError::Io {
        path: agents_dir.display().to_string(),
        source,
    })?;

    let pool_path = agents_dir.join("name_pool.json");
    let pool = load_or_create_name_pool(&pool_path)?;

    let registry_path = agents_dir.join("registry.json");
    let mut registry = load_registry(&registry_path)?;
    if let Some(existing) = find_existing_identity(
        &registry,
        device_name.as_str(),
        resolved_role_id.as_str(),
        source,
        requested_agent_name.as_deref(),
    ) {
        persist_current_registry(runtime_home, &registry)?;
        return Ok(existing);
    }

    let base_agent_name = requested_agent_name
        .or_else(|| default_agent_name(&resolved_role_id, &pool, &registry, &device_name))
        .unwrap_or_else(|| "agent".into());
    let agent_name = uniquify_agent_name(&device_name, &base_agent_name, &registry);
    let identity = AllocatedAgentIdentity {
        agent_id: format!("{device_name}.{agent_name}"),
        worker_id: format!("worker-{agent_name}"),
        device_name,
        agent_name,
    };
    registry.agents.push(RegisteredAgentIdentity {
        agent_id: identity.agent_id.clone(),
        agent_name: identity.agent_name.clone(),
        device_name: identity.device_name.clone(),
        worker_id: identity.worker_id.clone(),
        role_id: resolved_role_id,
        source: source.into(),
    });
    write_json(&registry_path, &registry)?;
    persist_current_registry(runtime_home, &registry)?;
    Ok(identity)
}

pub fn resolve_agent_identity_by_worker_id(
    runtime_home: &Path,
    worker_id: &str,
) -> Result<Option<AllocatedAgentIdentity>, RuntimeError> {
    let registry_path = runtime_home.join("runtime/agents/registry.json");
    let registry = load_registry(&registry_path)?;
    Ok(registry
        .agents
        .iter()
        .find(|entry| entry.worker_id == worker_id)
        .map(to_allocated_identity))
}

pub fn persist_assignment_summary(
    runtime_home: &Path,
    summary: &AgentAssignmentSummary,
) -> Result<(), RuntimeError> {
    write_json(
        &runtime_home.join("runtime/current/current_assignment_summary.json"),
        summary,
    )?;
    let by_worker_path = runtime_home.join(format!(
        "runtime/assignments/by_worker/{}.json",
        summary.worker_id
    ));
    write_json(&by_worker_path, summary)
}

pub fn read_assignment_summary(
    runtime_home: &Path,
) -> Result<Option<AgentAssignmentSummary>, RuntimeError> {
    let current_path = runtime_home.join("runtime/current/current_assignment_summary.json");
    if let Ok(Some(summary)) = read_json::<AgentAssignmentSummary>(&current_path) {
        return Ok(Some(summary));
    }
    let by_worker_dir = runtime_home.join("runtime/assignments/by_worker");
    let Ok(entries) = fs::read_dir(by_worker_dir) else {
        return Ok(None);
    };
    for entry in entries.filter_map(Result::ok) {
        if let Ok(Some(summary)) = read_json::<AgentAssignmentSummary>(&entry.path()) {
            return Ok(Some(summary));
        }
    }
    Ok(None)
}

pub fn resolve_device_name(system: &SystemConfig) -> String {
    select_device_name(
        system.runtime.device_name.as_deref(),
        env::var("COMPUTERNAME").ok().as_deref(),
        env::var("HOSTNAME").ok().as_deref(),
        system_hostname().as_deref(),
    )
}

fn select_device_name(
    configured: Option<&str>,
    computer_name: Option<&str>,
    host_name: Option<&str>,
    hostname_output: Option<&str>,
) -> String {
    configured
        .and_then(sanitize_name_part)
        .or_else(|| computer_name.and_then(sanitize_name_part))
        .or_else(|| host_name.and_then(sanitize_name_part))
        .or_else(|| hostname_output.and_then(sanitize_name_part))
        .unwrap_or_else(|| "device".into())
}

fn system_hostname() -> Option<String> {
    let output = Command::new("hostname").output().ok()?;
    if !output.status.success() {
        return None;
    }
    String::from_utf8(output.stdout)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn sanitize_name_part(raw: &str) -> Option<String> {
    let sanitized = raw
        .trim()
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                ch.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>()
        .trim_matches('-')
        .to_string();
    if sanitized.is_empty() {
        None
    } else {
        Some(sanitized)
    }
}

fn load_or_create_name_pool(path: &Path) -> Result<AgentNamePool, RuntimeError> {
    match fs::read_to_string(path) {
        Ok(content) => serde_json::from_str(&content).map_err(RuntimeError::Serialize),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            let pool = AgentNamePool {
                names: DEFAULT_AGENT_NAMES
                    .iter()
                    .filter_map(|value| sanitize_name_part(value))
                    .collect(),
            };
            write_json(path, &pool)?;
            Ok(pool)
        }
        Err(source) => Err(RuntimeError::Io {
            path: path.display().to_string(),
            source,
        }),
    }
}

fn load_registry(path: &Path) -> Result<AgentRegistry, RuntimeError> {
    match fs::read_to_string(path) {
        Ok(content) => serde_json::from_str(&content).map_err(RuntimeError::Serialize),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(AgentRegistry::default()),
        Err(source) => Err(RuntimeError::Io {
            path: path.display().to_string(),
            source,
        }),
    }
}

fn persist_current_registry(
    runtime_home: &Path,
    registry: &AgentRegistry,
) -> Result<(), RuntimeError> {
    write_json(
        &runtime_home.join("runtime/current/current_agent_registry.json"),
        registry,
    )
}

fn find_existing_identity(
    registry: &AgentRegistry,
    device_name: &str,
    role_id: &str,
    source: &str,
    requested_agent_name: Option<&str>,
) -> Option<AllocatedAgentIdentity> {
    registry
        .agents
        .iter()
        .find(|entry| {
            entry.device_name == device_name
                && entry.role_id == role_id
                && entry.source == source
                && match requested_agent_name {
                    Some(name) => entry.agent_name == name,
                    None => true,
                }
        })
        .map(to_allocated_identity)
}

fn default_agent_name(
    role_id: &str,
    pool: &AgentNamePool,
    registry: &AgentRegistry,
    device_name: &str,
) -> Option<String> {
    if role_id == "system" {
        return Some("system".into());
    }
    pool.names
        .iter()
        .find(|name| !agent_name_taken(device_name, name, registry))
        .cloned()
        .or_else(|| Some(role_id.to_string()))
}

fn uniquify_agent_name(device_name: &str, base_name: &str, registry: &AgentRegistry) -> String {
    if !agent_name_taken(device_name, base_name, registry) {
        return base_name.into();
    }
    for index in 2..=1_000 {
        let candidate = format!("{base_name}-{index}");
        if !agent_name_taken(device_name, &candidate, registry) {
            return candidate;
        }
    }
    format!("{base_name}-overflow")
}

fn agent_name_taken(device_name: &str, candidate: &str, registry: &AgentRegistry) -> bool {
    let agent_id = format!("{device_name}.{candidate}");
    registry
        .agents
        .iter()
        .any(|entry| entry.agent_id == agent_id)
}

fn to_allocated_identity(entry: &RegisteredAgentIdentity) -> AllocatedAgentIdentity {
    AllocatedAgentIdentity {
        device_name: entry.device_name.clone(),
        agent_name: entry.agent_name.clone(),
        agent_id: entry.agent_id.clone(),
        worker_id: entry.worker_id.clone(),
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

fn read_json<T: for<'de> Deserialize<'de>>(path: &Path) -> Result<Option<T>, RuntimeError> {
    match fs::read_to_string(path) {
        Ok(content) => serde_json::from_str(&content)
            .map(Some)
            .map_err(RuntimeError::Serialize),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(source) => Err(RuntimeError::Io {
            path: path.display().to_string(),
            source,
        }),
    }
}

#[cfg(test)]
#[path = "naming_tests.rs"]
mod naming_tests;
