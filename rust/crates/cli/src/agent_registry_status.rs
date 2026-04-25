use crate::CliError;
use serde::Deserialize;
use std::{fs, path::Path};

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
struct AgentPresenceRegistry {
    #[serde(default)]
    agents: Vec<AgentPresenceEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
struct AgentPresenceEntry {
    agent_id: String,
    agent_name: String,
    device_name: String,
    role_id: String,
    status: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
struct AgentRegistry {
    #[serde(default)]
    agents: Vec<AgentRegistryEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
struct AgentRegistryEntry {
    agent_id: String,
    agent_name: String,
    device_name: String,
    worker_id: String,
    role_id: String,
    source: String,
}

pub(crate) fn render_agent_registry_summary(runtime_home: &Path) -> Result<String, CliError> {
    let current_presence_path =
        runtime_home.join("runtime/current/current_agent_presence_registry.json");
    if let Some(summary) = read_presence_summary_if_exists(&current_presence_path)? {
        return Ok(summary);
    }
    let presence_path = runtime_home.join("runtime/agents/presence_registry.json");
    if let Some(summary) = read_presence_summary_if_exists(&presence_path)? {
        return Ok(summary);
    }
    let current_path = runtime_home.join("runtime/current/current_agent_registry.json");
    if let Some(summary) = read_summary_if_exists(&current_path)? {
        return Ok(summary);
    }
    let registry_path = runtime_home.join("runtime/agents/registry.json");
    if let Some(summary) = read_summary_if_exists(&registry_path)? {
        return Ok(summary);
    }
    Ok("agents=none".into())
}

fn read_presence_summary_if_exists(path: &Path) -> Result<Option<String>, CliError> {
    match fs::read_to_string(path) {
        Ok(content) => {
            let registry: AgentPresenceRegistry =
                serde_json::from_str(&content).map_err(CliError::Serialize)?;
            Ok(Some(format_presence_summary(&registry)))
        }
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(source) => Err(CliError::ReadFile {
            path: path.display().to_string(),
            source,
        }),
    }
}

fn read_summary_if_exists(path: &Path) -> Result<Option<String>, CliError> {
    match fs::read_to_string(path) {
        Ok(content) => {
            let registry: AgentRegistry =
                serde_json::from_str(&content).map_err(CliError::Serialize)?;
            Ok(Some(format_summary(&registry)))
        }
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(source) => Err(CliError::ReadFile {
            path: path.display().to_string(),
            source,
        }),
    }
}

fn format_presence_summary(registry: &AgentPresenceRegistry) -> String {
    if registry.agents.is_empty() {
        return "agents=none".into();
    }
    let mut items = registry
        .agents
        .iter()
        .map(|entry| {
            format!(
                "{}.{}:{}",
                entry.device_name, entry.agent_name, entry.status
            )
        })
        .collect::<Vec<_>>();
    items.sort();
    items.dedup();
    let display = items.iter().take(4).cloned().collect::<Vec<_>>().join(", ");
    let overflow = items.len().saturating_sub(4);
    if overflow > 0 {
        format!("agents={} [{}] (+{})", items.len(), display, overflow)
    } else {
        format!("agents={} [{}]", items.len(), display)
    }
}

fn format_summary(registry: &AgentRegistry) -> String {
    if registry.agents.is_empty() {
        return "agents=none".into();
    }
    let mut items = registry
        .agents
        .iter()
        .map(|entry| format!("{}.{}", entry.device_name, entry.agent_name))
        .collect::<Vec<_>>();
    items.sort();
    items.dedup();
    let display = items.iter().take(3).cloned().collect::<Vec<_>>().join(", ");
    let overflow = items.len().saturating_sub(3);
    if overflow > 0 {
        format!("agents={} [{}] (+{})", items.len(), display, overflow)
    } else {
        format!("agents={} [{}]", items.len(), display)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        fs,
        path::PathBuf,
        time::{SystemTime, UNIX_EPOCH},
    };

    fn temp_runtime_home(prefix: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time")
            .as_nanos();
        let path = std::env::temp_dir().join(format!("fin-agent-registry-{prefix}-{unique}"));
        fs::create_dir_all(&path).expect("temp runtime home");
        path
    }

    #[test]
    fn prefers_current_registry_and_formats_compact_summary() {
        let home = temp_runtime_home("current");
        let path = home.join("runtime/current/current_agent_registry.json");
        fs::create_dir_all(path.parent().expect("parent")).expect("dir");
        fs::write(
            &path,
            br#"{"agents":[{"agent_id":"mbp.system","agent_name":"system","device_name":"mbp","worker_id":"worker-system","role_id":"system","source":"cli"},{"agent_id":"mbp.atlas","agent_name":"atlas","device_name":"mbp","worker_id":"worker-atlas","role_id":"project","source":"cli"}]}"#,
        )
        .expect("registry");

        let summary = render_agent_registry_summary(&home).expect("summary");
        assert_eq!(summary, "agents=2 [mbp.atlas, mbp.system]");
    }

    #[test]
    fn prefers_presence_registry_and_includes_status() {
        let home = temp_runtime_home("presence");
        let path = home.join("runtime/current/current_agent_presence_registry.json");
        fs::create_dir_all(path.parent().expect("parent")).expect("dir");
        fs::write(
            &path,
            br#"{"agents":[{"agent_id":"mbp.system","agent_name":"system","device_name":"mbp","role_id":"system","status":"busy"},{"agent_id":"mbp.builder","agent_name":"builder","device_name":"mbp","role_id":"project","status":"idle"}]}"#,
        )
        .expect("presence registry");

        let summary = render_agent_registry_summary(&home).expect("summary");
        assert_eq!(summary, "agents=2 [mbp.builder:idle, mbp.system:busy]");
    }

    #[test]
    fn uses_agents_registry_when_current_missing() {
        let home = temp_runtime_home("registry");
        let path = home.join("runtime/agents/registry.json");
        fs::create_dir_all(path.parent().expect("parent")).expect("dir");
        fs::write(
            &path,
            br#"{"agents":[{"agent_id":"mbp.system","agent_name":"system","device_name":"mbp","worker_id":"worker-system","role_id":"system","source":"cli"}]}"#,
        )
        .expect("registry");

        let summary = render_agent_registry_summary(&home).expect("summary");
        assert_eq!(summary, "agents=1 [mbp.system]");
    }
}
