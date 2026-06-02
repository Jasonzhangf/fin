use super::{AgentPresenceRecord, AgentPresenceRegistry};
use crate::CliError;
use serde::Serialize;
use std::{
    fs,
    path::{Path, PathBuf},
};

pub(super) fn presence_path(runtime_home: &Path, agent_id: &str) -> PathBuf {
    runtime_home.join(format!("runtime/agents/state/{agent_id}.json"))
}

pub(super) fn write_presence(
    runtime_home: &Path,
    record: &AgentPresenceRecord,
) -> Result<(), CliError> {
    let path = presence_path(runtime_home, &record.agent_id);
    write_json(&path, record)?;
    update_presence_registry(runtime_home, record)?;
    write_json(
        &runtime_home.join("runtime/current/current_agent_presence.json"),
        record,
    )
}

fn update_presence_registry(
    runtime_home: &Path,
    record: &AgentPresenceRecord,
) -> Result<(), CliError> {
    let registry_path = runtime_home.join("runtime/agents/presence_registry.json");
    let mut registry = read_presence_registry(&registry_path)?;
    if let Some(existing) = registry
        .agents
        .iter_mut()
        .find(|item| item.agent_id == record.agent_id)
    {
        *existing = record.clone();
    } else {
        registry.agents.push(record.clone());
    }
    registry
        .agents
        .sort_by(|left, right| left.agent_id.cmp(&right.agent_id));
    write_json(&registry_path, &registry)?;
    write_json(
        &runtime_home.join("runtime/current/current_agent_presence_registry.json"),
        &registry,
    )
}

fn read_presence_registry(path: &Path) -> Result<AgentPresenceRegistry, CliError> {
    match fs::read_to_string(path) {
        Ok(content) => serde_json::from_str(&content).map_err(CliError::Serialize),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            Ok(AgentPresenceRegistry::default())
        }
        Err(source) => Err(CliError::ReadFile {
            path: path.display().to_string(),
            source,
        }),
    }
}

pub(super) fn read_presence(path: &Path) -> Result<Option<AgentPresenceRecord>, CliError> {
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

pub(super) fn write_json<T: Serialize>(path: &Path, value: &T) -> Result<(), CliError> {
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
