use crate::CliError;
use fin_contracts::{
    ExecutionStateRecord, InputAttachmentSummary, PauseCheckpointRecord, PendingInputRecord,
};
use fin_debug_server::{ChatSendRequest, DebugBinding};
use fin_runtime::{
    ClosureRun, clear_waiting_state_if_due, dequeue_pending_input, failed_state, new_pending_input,
    paused_state, resumed_state, running_state, state_after_run, state_with_pending_count,
};
use serde_json::json;
use std::path::Path;

#[path = "execution_state_support.rs"]
mod support;
use support::{
    ensure_session_runtime_dirs, entity_refs, pending_input_source, read_json_if_exists,
    read_json_or_empty, resolve_paths, trim_head, update_last_run_paths, write_json,
};

const PENDING_INPUT_LIMIT: usize = 64;

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
    enqueue_pending_input_record(
        runtime_home,
        binding,
        request.input_kind.as_deref().unwrap_or("chat"),
        pending_input_source(request),
        &request.message,
        &request.attachments,
        enqueue_reason,
        now,
    )
}

pub(crate) fn restore_execution_state(
    runtime_home: &Path,
    binding: &DebugBinding,
    base_state: &ExecutionStateRecord,
    now: &str,
) -> Result<(), CliError> {
    let Some(paths) = resolve_paths(runtime_home, binding)? else {
        return Ok(());
    };
    ensure_session_runtime_dirs(&paths)?;
    let pending = read_json_or_empty::<PendingInputRecord>(&paths.pending_inputs_path())?;
    let restored = state_with_pending_count(base_state, pending.len(), now);
    write_json(&paths.execution_state_path(), &restored)?;
    write_json(
        &runtime_home.join("runtime/current/current_execution_state.json"),
        &restored,
    )?;
    update_last_run_paths(
        runtime_home,
        json!({"current_execution_state_path": "runtime/current/current_execution_state.json"}),
    )?;
    Ok(())
}

pub(crate) fn enqueue_framework_pending_input(
    runtime_home: &Path,
    binding: &DebugBinding,
    input_kind: &str,
    source: &str,
    message: &str,
    enqueue_reason: &str,
    now: &str,
) -> Result<Option<PendingInputRecord>, CliError> {
    enqueue_pending_input_record(
        runtime_home,
        binding,
        input_kind,
        source,
        message,
        &[],
        enqueue_reason,
        now,
    )
}

fn enqueue_pending_input_record(
    runtime_home: &Path,
    binding: &DebugBinding,
    input_kind: &str,
    source: &str,
    message: &str,
    attachments: &[InputAttachmentSummary],
    enqueue_reason: &str,
    now: &str,
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
        input_kind,
        source,
        message,
        attachments,
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
