use crate::{CliError, session_binding::find_session_dir};
use fin_debug_server::DebugBinding;
use serde::{Serialize, de::DeserializeOwned};
use serde_json::{Value, json};
use std::{
    fs,
    path::{Path, PathBuf},
};

#[derive(Debug, Clone)]
pub(crate) struct DaemonPaths {
    pub(crate) session_dir: PathBuf,
}

impl DaemonPaths {
    pub(crate) fn latest_daemon_state_path(&self) -> PathBuf {
        self.session_dir.join("control/daemon/latest_state.json")
    }

    pub(crate) fn recent_daemon_states_path(&self) -> PathBuf {
        self.session_dir.join("control/daemon/recent_states.json")
    }

    pub(crate) fn latest_recovery_action_path(&self) -> PathBuf {
        self.session_dir
            .join("control/daemon/latest_recovery_action.json")
    }

    pub(crate) fn recent_recovery_actions_path(&self) -> PathBuf {
        self.session_dir
            .join("control/daemon/recent_recovery_actions.json")
    }

    pub(crate) fn latest_supervisor_cycle_path(&self) -> PathBuf {
        self.session_dir.join("control/supervisor/latest.json")
    }

    pub(crate) fn latest_supervisor_heartbeat_path(&self) -> PathBuf {
        self.session_dir
            .join("control/supervisor/latest_heartbeat.json")
    }
}

pub(crate) fn resolve_paths(
    runtime_home: &Path,
    binding: &DebugBinding,
) -> Result<Option<DaemonPaths>, CliError> {
    let Some(session_id) = binding.session_id.as_deref() else {
        return Ok(None);
    };
    Ok(find_session_dir(runtime_home, session_id)
        .map(|(_, _, session_dir)| DaemonPaths { session_dir }))
}

pub(crate) fn ensure_dirs(paths: &DaemonPaths) -> Result<(), CliError> {
    fs::create_dir_all(paths.session_dir.join("control/daemon")).map_err(|source| {
        CliError::WriteFile {
            path: paths
                .session_dir
                .join("control/daemon")
                .display()
                .to_string(),
            source,
        }
    })
}

pub(crate) fn session_recent_states_relative(
    paths: &DaemonPaths,
    runtime_home: &Path,
) -> Result<String, CliError> {
    paths
        .recent_daemon_states_path()
        .strip_prefix(runtime_home)
        .map(|path| path.to_string_lossy().trim_start_matches('/').to_string())
        .map_err(|_| CliError::Usage)
}

pub(crate) fn session_recent_recovery_actions_relative(
    paths: &DaemonPaths,
    runtime_home: &Path,
) -> Result<String, CliError> {
    paths
        .recent_recovery_actions_path()
        .strip_prefix(runtime_home)
        .map(|path| path.to_string_lossy().trim_start_matches('/').to_string())
        .map_err(|_| CliError::Usage)
}

pub(crate) fn trim_head<T>(items: &mut Vec<T>, limit: usize) {
    if items.len() > limit {
        let drain_count = items.len() - limit;
        items.drain(0..drain_count);
    }
}

pub(crate) fn read_json_or_empty<T: DeserializeOwned>(path: &Path) -> Result<Vec<T>, CliError> {
    match fs::read_to_string(path) {
        Ok(content) => serde_json::from_str(&content).map_err(CliError::Serialize),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(source) => Err(CliError::ReadFile {
            path: path.display().to_string(),
            source,
        }),
    }
}

pub(crate) fn read_json_if_exists<T: DeserializeOwned>(path: &Path) -> Result<Option<T>, CliError> {
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

pub(crate) fn write_json<T: Serialize>(path: &Path, value: &T) -> Result<(), CliError> {
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

pub(crate) fn update_last_run_paths(runtime_home: &Path, updates: Value) -> Result<(), CliError> {
    let last_run_path = runtime_home.join("runtime/current/last_run.json");
    let mut value = match fs::read_to_string(&last_run_path) {
        Ok(content) => serde_json::from_str::<Value>(&content).map_err(CliError::Serialize)?,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => json!({}),
        Err(source) => {
            return Err(CliError::ReadFile {
                path: last_run_path.display().to_string(),
                source,
            });
        }
    };
    if !value.is_object() {
        value = json!({});
    }
    let object = value.as_object_mut().expect("object");
    for (key, val) in updates.as_object().into_iter().flatten() {
        object.insert(key.clone(), val.clone());
    }
    write_json(&last_run_path, &value)
}

pub(crate) fn sanitize_id(input: &str) -> String {
    input
        .chars()
        .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '-' })
        .collect()
}
