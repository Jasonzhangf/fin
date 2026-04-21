use crate::CliError;
use fin_config::RuntimeRetentionConfig;
use fin_contracts::{
    DebugVisibility, EntityRefs, EventEnvelope, ExecutionCheckpointRecord, Severity,
};
use fin_debug_server::DebugBinding;
use fin_runtime::append_framework_events;
use serde::{Serialize, de::DeserializeOwned};
use serde_json::{Value, json};
use std::{
    fs,
    path::{Path, PathBuf},
};

#[derive(Debug, Clone)]
struct SessionCheckpointPaths {
    session_dir: PathBuf,
}

impl SessionCheckpointPaths {
    fn latest_path(&self) -> PathBuf {
        self.session_dir.join("control/execution_checkpoint.json")
    }

    fn recent_path(&self) -> PathBuf {
        self.session_dir
            .join("control/recent_execution_checkpoints.json")
    }
}

pub(crate) fn load_open_execution_checkpoint(
    runtime_home: &Path,
    binding: &DebugBinding,
) -> Result<Option<ExecutionCheckpointRecord>, CliError> {
    let Some(paths) = resolve_paths(runtime_home, binding)? else {
        return Ok(None);
    };
    let checkpoint = read_json_if_exists::<ExecutionCheckpointRecord>(&paths.latest_path())?;
    Ok(checkpoint.filter(|item| item.status == "open"))
}

pub(crate) fn restore_execution_checkpoint(
    runtime_home: &Path,
    binding: &DebugBinding,
    checkpoint: &ExecutionCheckpointRecord,
) -> Result<(), CliError> {
    let Some(paths) = resolve_paths(runtime_home, binding)? else {
        return Ok(());
    };
    write_json(&paths.latest_path(), checkpoint)?;
    let mut recent = read_json_or_empty::<ExecutionCheckpointRecord>(&paths.recent_path())?;
    if let Some(item) = recent
        .iter_mut()
        .rfind(|item| item.checkpoint_id == checkpoint.checkpoint_id)
    {
        *item = checkpoint.clone();
    } else {
        recent.push(checkpoint.clone());
    }
    write_json(&paths.recent_path(), &recent)?;
    write_json(
        &runtime_home.join("runtime/current/current_execution_checkpoint.json"),
        checkpoint,
    )?;
    update_last_run_paths(
        runtime_home,
        json!({
            "current_execution_checkpoint_path": "runtime/current/current_execution_checkpoint.json"
        }),
    )
}

pub(crate) fn consume_execution_checkpoint(
    runtime_home: &Path,
    binding: &DebugBinding,
    checkpoint: &ExecutionCheckpointRecord,
    retention: &RuntimeRetentionConfig,
    consumed_at: &str,
) -> Result<Option<ExecutionCheckpointRecord>, CliError> {
    let Some(paths) = resolve_paths(runtime_home, binding)? else {
        return Ok(None);
    };
    let mut updated = checkpoint.clone();
    updated.status = "consumed".into();
    updated.consumed_at = Some(consumed_at.into());
    updated.consumed_by_operation_id = read_last_operation_id(runtime_home)?;
    write_json(&paths.latest_path(), &updated)?;
    let mut recent = read_json_or_empty::<ExecutionCheckpointRecord>(&paths.recent_path())?;
    if let Some(item) = recent
        .iter_mut()
        .rfind(|item| item.checkpoint_id == updated.checkpoint_id)
    {
        *item = updated.clone();
    } else {
        recent.push(updated.clone());
    }
    write_json(&paths.recent_path(), &recent)?;
    write_json(
        &runtime_home.join("runtime/current/current_execution_checkpoint.json"),
        &updated,
    )?;
    update_last_run_paths(
        runtime_home,
        json!({
            "current_execution_checkpoint_path": "runtime/current/current_execution_checkpoint.json"
        }),
    )?;
    append_framework_events(
        runtime_home,
        &paths.session_dir,
        &[checkpoint_event(
            consumed_at,
            &entity_refs(binding),
            &updated,
            "execution.checkpoint_consumed",
        )],
        retention,
    )
    .map_err(map_runtime_error)?;
    Ok(Some(updated))
}

fn checkpoint_event(
    occurred_at: &str,
    refs: &EntityRefs,
    checkpoint: &ExecutionCheckpointRecord,
    event_type: &str,
) -> EventEnvelope<Value> {
    let mut event = EventEnvelope::new(
        format!("{}-evt-01", checkpoint.checkpoint_id),
        event_type,
        occurred_at.to_string(),
        "cli.execution_checkpoint",
        checkpoint.trace_id.clone(),
        1,
        serde_json::to_value(checkpoint).unwrap_or_else(|_| json!({})),
    );
    event.refs = refs.clone();
    event.operation_id = checkpoint.consumed_by_operation_id.clone();
    event.severity = Severity::Info;
    event.debug_visibility = DebugVisibility::Important;
    event
}

fn read_last_operation_id(runtime_home: &Path) -> Result<Option<String>, CliError> {
    let path = runtime_home.join("runtime/current/last_run.json");
    let value = match fs::read_to_string(&path) {
        Ok(content) => serde_json::from_str::<Value>(&content).map_err(CliError::Serialize)?,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(source) => {
            return Err(CliError::ReadFile {
                path: path.display().to_string(),
                source,
            });
        }
    };
    Ok(value
        .get("operation_id")
        .and_then(serde_json::Value::as_str)
        .map(str::to_string))
}

fn resolve_paths(
    runtime_home: &Path,
    binding: &DebugBinding,
) -> Result<Option<SessionCheckpointPaths>, CliError> {
    if let Some(relative) = &binding.session_messages_path {
        if let Some(prefix) = relative.strip_suffix("conversation/messages.json") {
            return Ok(Some(SessionCheckpointPaths {
                session_dir: runtime_home.join(prefix.trim_end_matches('/')),
            }));
        }
    }
    let Some(session_id) = binding.session_id.as_deref() else {
        return Ok(None);
    };
    Ok(find_session_dir(runtime_home, session_id)
        .map(|session_dir| SessionCheckpointPaths { session_dir }))
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

fn entity_refs(binding: &DebugBinding) -> EntityRefs {
    EntityRefs {
        session_id: binding.session_id.clone(),
        task_id: binding.task_id.clone(),
        ..EntityRefs::default()
    }
}

fn read_json_if_exists<T: DeserializeOwned>(path: &Path) -> Result<Option<T>, CliError> {
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

fn read_json_or_empty<T: DeserializeOwned>(path: &Path) -> Result<Vec<T>, CliError> {
    match fs::read_to_string(path) {
        Ok(content) => serde_json::from_str(&content).map_err(CliError::Serialize),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
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

fn update_last_run_paths(runtime_home: &Path, updates: Value) -> Result<(), CliError> {
    let path = runtime_home.join("runtime/current/last_run.json");
    let mut value = match fs::read_to_string(&path) {
        Ok(content) => serde_json::from_str::<Value>(&content).map_err(CliError::Serialize)?,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => json!({}),
        Err(source) => {
            return Err(CliError::ReadFile {
                path: path.display().to_string(),
                source,
            });
        }
    };
    let object = value.as_object_mut().ok_or(CliError::Usage)?;
    for (key, val) in updates.as_object().into_iter().flatten() {
        object.insert(key.clone(), val.clone());
    }
    write_json(&path, &value)
}

fn map_runtime_error(error: fin_runtime::RuntimeError) -> CliError {
    match error {
        fin_runtime::RuntimeError::Io { path, source } => CliError::WriteFile { path, source },
        other => CliError::Serialize(serde_json::Error::io(std::io::Error::other(
            other.to_string(),
        ))),
    }
}
