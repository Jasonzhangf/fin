use crate::CliError;
use fin_contracts::{EntityRefs, ExecutionStateRecord, PauseCheckpointRecord, PendingInputRecord};
use fin_debug_server::{ChatSendRequest, DebugBinding};
use fin_runtime::{
    ClosureRun, clear_waiting_state_if_due, dequeue_pending_input, failed_state, new_pending_input,
    paused_state, resumed_state, running_state, state_after_run, state_with_pending_count,
};
use serde::{Serialize, de::DeserializeOwned};
use serde_json::{Value, json};
use std::{
    fs,
    path::{Path, PathBuf},
};

const PENDING_INPUT_LIMIT: usize = 64;

#[derive(Debug, Clone)]
pub(crate) struct SessionExecutionPaths {
    session_dir: PathBuf,
}

impl SessionExecutionPaths {
    fn execution_state_path(&self) -> PathBuf {
        self.session_dir.join("control/execution_state.json")
    }

    fn pause_checkpoint_path(&self) -> PathBuf {
        self.session_dir.join("control/pause_checkpoint.json")
    }

    fn pending_inputs_path(&self) -> PathBuf {
        self.session_dir.join("queue/pending_inputs.json")
    }
}

pub(crate) fn load_execution_state(
    runtime_home: &Path,
    binding: &DebugBinding,
) -> Result<Option<ExecutionStateRecord>, CliError> {
    let Some(paths) = resolve_paths(runtime_home, binding)? else {
        return Ok(None);
    };
    read_json_if_exists(&paths.execution_state_path())
}

pub(crate) fn load_pending_inputs(
    runtime_home: &Path,
    binding: &DebugBinding,
) -> Result<Vec<PendingInputRecord>, CliError> {
    let Some(paths) = resolve_paths(runtime_home, binding)? else {
        return Ok(Vec::new());
    };
    read_json_or_empty(&paths.pending_inputs_path())
}

pub(crate) fn mark_running(
    runtime_home: &Path,
    binding: &DebugBinding,
    operation_id: &str,
    submitted_at: &str,
) -> Result<(), CliError> {
    let Some(paths) = resolve_paths(runtime_home, binding)? else {
        return Ok(());
    };
    ensure_session_runtime_dirs(&paths)?;
    let pending = read_json_or_empty::<PendingInputRecord>(&paths.pending_inputs_path())?;
    let refs = entity_refs(binding);
    let state = running_state(&refs, operation_id, submitted_at, pending.len());
    write_json(&paths.execution_state_path(), &state)?;
    write_json(
        &runtime_home.join("runtime/current/current_execution_state.json"),
        &state,
    )?;
    update_last_run_paths(
        runtime_home,
        json!({"current_execution_state_path": "runtime/current/current_execution_state.json"}),
    )?;
    Ok(())
}

pub(crate) fn finalize_after_run(
    runtime_home: &Path,
    binding: &DebugBinding,
    run: &ClosureRun,
) -> Result<(), CliError> {
    let Some(paths) = resolve_paths(runtime_home, binding)? else {
        return Ok(());
    };
    ensure_session_runtime_dirs(&paths)?;
    let pending = read_json_or_empty::<PendingInputRecord>(&paths.pending_inputs_path())?;
    let state = state_after_run(run, pending.len());
    write_json(&paths.execution_state_path(), &state)?;
    write_json(
        &runtime_home.join("runtime/current/current_execution_state.json"),
        &state,
    )?;
    update_last_run_paths(
        runtime_home,
        json!({"current_execution_state_path": "runtime/current/current_execution_state.json"}),
    )?;
    Ok(())
}

pub(crate) fn mark_failed(
    runtime_home: &Path,
    binding: &DebugBinding,
    operation_id: &str,
    now: &str,
    reason: &str,
) -> Result<(), CliError> {
    let Some(paths) = resolve_paths(runtime_home, binding)? else {
        return Ok(());
    };
    ensure_session_runtime_dirs(&paths)?;
    let pending = read_json_or_empty::<PendingInputRecord>(&paths.pending_inputs_path())?;
    let refs = entity_refs(binding);
    let state = failed_state(&refs, operation_id, now, reason, pending.len());
    write_json(&paths.execution_state_path(), &state)?;
    write_json(
        &runtime_home.join("runtime/current/current_execution_state.json"),
        &state,
    )?;
    update_last_run_paths(
        runtime_home,
        json!({"current_execution_state_path": "runtime/current/current_execution_state.json"}),
    )?;
    Ok(())
}

pub(crate) fn pause_execution(
    runtime_home: &Path,
    binding: &DebugBinding,
    now: &str,
    reason: Option<String>,
) -> Result<Option<(ExecutionStateRecord, PauseCheckpointRecord)>, CliError> {
    let Some(paths) = resolve_paths(runtime_home, binding)? else {
        return Ok(None);
    };
    ensure_session_runtime_dirs(&paths)?;
    let pending = read_json_or_empty::<PendingInputRecord>(&paths.pending_inputs_path())?;
    let current_state = read_json_if_exists::<ExecutionStateRecord>(&paths.execution_state_path())?;
    let refs = entity_refs(binding);
    let (state, checkpoint) = paused_state(
        &refs,
        binding.session_id.as_deref(),
        current_state.as_ref(),
        now,
        reason,
        pending.len(),
    );
    write_json(&paths.pause_checkpoint_path(), &checkpoint)?;
    write_json(&paths.execution_state_path(), &state)?;
    write_json(
        &runtime_home.join("runtime/current/current_pause_checkpoint.json"),
        &checkpoint,
    )?;
    write_json(
        &runtime_home.join("runtime/current/current_execution_state.json"),
        &state,
    )?;
    update_last_run_paths(
        runtime_home,
        json!({
            "current_execution_state_path": "runtime/current/current_execution_state.json",
            "current_pause_checkpoint_path": "runtime/current/current_pause_checkpoint.json"
        }),
    )?;
    Ok(Some((state, checkpoint)))
}

pub(crate) fn resume_execution(
    runtime_home: &Path,
    binding: &DebugBinding,
    now: &str,
) -> Result<Option<ExecutionStateRecord>, CliError> {
    let Some(paths) = resolve_paths(runtime_home, binding)? else {
        return Ok(None);
    };
    ensure_session_runtime_dirs(&paths)?;
    let pending = read_json_or_empty::<PendingInputRecord>(&paths.pending_inputs_path())?;
    let paused = read_json_if_exists::<PauseCheckpointRecord>(&paths.pause_checkpoint_path())?;
    let refs = entity_refs(binding);
    let state = resumed_state(
        &refs,
        binding.session_id.as_deref(),
        paused.as_ref(),
        now,
        pending.len(),
    );
    write_json(&paths.execution_state_path(), &state)?;
    write_json(
        &runtime_home.join("runtime/current/current_execution_state.json"),
        &state,
    )?;
    update_last_run_paths(
        runtime_home,
        json!({"current_execution_state_path": "runtime/current/current_execution_state.json"}),
    )?;
    Ok(Some(state))
}

pub(crate) fn enqueue_pending_input(
    runtime_home: &Path,
    binding: &DebugBinding,
    request: &ChatSendRequest,
    now: &str,
    enqueue_reason: &str,
) -> Result<Option<PendingInputRecord>, CliError> {
    let Some(paths) = resolve_paths(runtime_home, binding)? else {
        return Ok(None);
    };
    ensure_session_runtime_dirs(&paths)?;
    let mut pending = read_json_or_empty::<PendingInputRecord>(&paths.pending_inputs_path())?;
    let refs = entity_refs(binding);
    let record = new_pending_input(
        &refs,
        binding.session_id.as_deref(),
        pending.len() + 1,
        request.input_kind.as_deref().unwrap_or("chat"),
        &request.message,
        enqueue_reason,
        now,
    );
    pending.push(record.clone());
    trim_head(&mut pending, PENDING_INPUT_LIMIT);
    write_json(&paths.pending_inputs_path(), &pending)?;
    write_json(
        &runtime_home.join("runtime/current/current_pending_inputs.json"),
        &pending,
    )?;
    update_last_run_paths(
        runtime_home,
        json!({
            "current_pending_inputs_path": "runtime/current/current_pending_inputs.json",
            "current_execution_state_path": "runtime/current/current_execution_state.json"
        }),
    )?;
    if let Some(mut state) =
        read_json_if_exists::<ExecutionStateRecord>(&paths.execution_state_path())?
    {
        state = state_with_pending_count(&state, pending.len(), now);
        write_json(&paths.execution_state_path(), &state)?;
        write_json(
            &runtime_home.join("runtime/current/current_execution_state.json"),
            &state,
        )?;
        update_last_run_paths(
            runtime_home,
            json!({"current_execution_state_path": "runtime/current/current_execution_state.json"}),
        )?;
    }
    Ok(Some(record))
}

pub(crate) fn dequeue_next_pending_input(
    runtime_home: &Path,
    binding: &DebugBinding,
    now: &str,
) -> Result<Option<PendingInputRecord>, CliError> {
    let Some(paths) = resolve_paths(runtime_home, binding)? else {
        return Ok(None);
    };
    ensure_session_runtime_dirs(&paths)?;
    let pending = read_json_or_empty::<PendingInputRecord>(&paths.pending_inputs_path())?;
    let Some(dequeue) = dequeue_pending_input(&pending) else {
        return Ok(None);
    };
    write_json(&paths.pending_inputs_path(), &dequeue.remaining)?;
    write_json(
        &runtime_home.join("runtime/current/current_pending_inputs.json"),
        &dequeue.remaining,
    )?;
    update_last_run_paths(
        runtime_home,
        json!({
            "current_pending_inputs_path": "runtime/current/current_pending_inputs.json",
            "current_execution_state_path": "runtime/current/current_execution_state.json"
        }),
    )?;
    if let Some(mut state) =
        read_json_if_exists::<ExecutionStateRecord>(&paths.execution_state_path())?
    {
        state = state_with_pending_count(&state, dequeue.remaining.len(), now);
        write_json(&paths.execution_state_path(), &state)?;
        write_json(
            &runtime_home.join("runtime/current/current_execution_state.json"),
            &state,
        )?;
        update_last_run_paths(
            runtime_home,
            json!({"current_execution_state_path": "runtime/current/current_execution_state.json"}),
        )?;
    }
    Ok(Some(dequeue.dequeued))
}

pub(crate) fn clear_waiting_if_due(
    runtime_home: &Path,
    binding: &DebugBinding,
    now: &str,
) -> Result<(), CliError> {
    let Some(paths) = resolve_paths(runtime_home, binding)? else {
        return Ok(());
    };
    let Some(mut state) =
        read_json_if_exists::<ExecutionStateRecord>(&paths.execution_state_path())?
    else {
        return Ok(());
    };
    if let Some(updated) = clear_waiting_state_if_due(&state, now) {
        state = updated;
        write_json(&paths.execution_state_path(), &state)?;
        write_json(
            &runtime_home.join("runtime/current/current_execution_state.json"),
            &state,
        )?;
        update_last_run_paths(
            runtime_home,
            json!({"current_execution_state_path": "runtime/current/current_execution_state.json"}),
        )?;
    }
    Ok(())
}

fn resolve_paths(
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

fn ensure_session_runtime_dirs(paths: &SessionExecutionPaths) -> Result<(), CliError> {
    for rel in ["control", "queue"] {
        fs::create_dir_all(paths.session_dir.join(rel)).map_err(|source| CliError::WriteFile {
            path: paths.session_dir.join(rel).display().to_string(),
            source,
        })?;
    }
    Ok(())
}

fn entity_refs(binding: &DebugBinding) -> EntityRefs {
    EntityRefs {
        session_id: binding.session_id.clone(),
        task_id: binding.task_id.clone(),
        ..EntityRefs::default()
    }
}

fn trim_head<T>(items: &mut Vec<T>, limit: usize) {
    if items.len() > limit {
        let drain_count = items.len() - limit;
        items.drain(0..drain_count);
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
