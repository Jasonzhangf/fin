use crate::CliError;
use fin_contracts::EntityRefs;
use fin_debug_server::{ChatSendRequest, DebugBinding};
use serde::{Serialize, de::DeserializeOwned};
use serde_json::{Value, json};
use std::{
    fs,
    path::{Path, PathBuf},
};

#[derive(Debug, Clone)]
pub(crate) struct SessionExecutionPaths {
    session_dir: PathBuf,
}

impl SessionExecutionPaths {
    pub(crate) fn execution_state_path(&self) -> PathBuf {
        self.session_dir.join("control/execution_state.json")
    }

    pub(crate) fn execution_lease_path(&self) -> PathBuf {
        self.session_dir.join("control/execution_lease.json")
    }

    pub(crate) fn pause_checkpoint_path(&self) -> PathBuf {
        self.session_dir.join("control/pause_checkpoint.json")
    }

    pub(crate) fn pending_inputs_path(&self) -> PathBuf {
        self.session_dir.join("queue/pending_inputs.json")
    }
}

pub(crate) fn resolve_paths(
    runtime_home: &Path,
    binding: &DebugBinding,
) -> Result<Option<SessionExecutionPaths>, CliError> {
    if let Some(relative) = &binding.session_messages_path {
        if let Some(prefix) = relative.strip_suffix("conversation/messages.json") {
            return Ok(Some(SessionExecutionPaths {
                session_dir: runtime_home.join(prefix.trim_end_matches('/')),
            }));
        }
    }
    let Some(session_id) = binding.session_id.as_deref() else {
        return Ok(None);
    };
    Ok(find_session_dir(runtime_home, session_id)
        .map(|dir| SessionExecutionPaths { session_dir: dir }))
}

pub(crate) fn ensure_session_runtime_dirs(paths: &SessionExecutionPaths) -> Result<(), CliError> {
    for rel in ["control", "queue"] {
        fs::create_dir_all(paths.session_dir.join(rel)).map_err(|source| CliError::WriteFile {
            path: paths.session_dir.join(rel).display().to_string(),
            source,
        })?;
    }
    Ok(())
}

pub(crate) fn entity_refs(binding: &DebugBinding) -> EntityRefs {
    EntityRefs {
        session_id: binding.session_id.clone(),
        task_id: binding.task_id.clone(),
        ..EntityRefs::default()
    }
}

pub(crate) fn trim_head<T>(items: &mut Vec<T>, limit: usize) {
    if items.len() > limit {
        let drain_count = items.len() - limit;
        items.drain(0..drain_count);
    }
}

pub(crate) fn pending_input_source(request: &ChatSendRequest) -> &str {
    if request.input_kind.as_deref() == Some("parallel_chat") {
        "cli.parallel_user"
    } else if request.input_kind.as_deref() == Some("parallel_channel_ingress") {
        "channel.parallel_user"
    } else if request.input_kind.as_deref() == Some("channel_ingress") {
        "channel.qqbot"
    } else {
        "cli.user"
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

fn find_session_dir(runtime_home: &Path, session_id: &str) -> Option<PathBuf> {
    let root = runtime_home.join("sessions");
    let years = fs::read_dir(root).ok()?;
    for year in years.flatten() {
        let months = fs::read_dir(year.path()).ok()?;
        for month in months.flatten() {
            let dir = month.path().join(session_id);
            if dir.exists() {
                return Some(dir);
            }
        }
    }
    None
}
