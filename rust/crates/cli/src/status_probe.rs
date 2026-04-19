use crate::{
    CliError, runtime_home::read_last_run_value, scheduler_driver::load_latest_scheduler_decision,
};
use fin_contracts::{
    ControlFeedback, DaemonRecoveryActionRecord, DaemonStateRecord, ExecutionNote,
    ExecutionStateRecord, PendingInputRecord, ProgressBlock, RoutingActionRecord,
    SchedulerDecisionRecord, SchedulerTickRecord, SupervisorCycleRecord, SupervisorHeartbeatRecord,
};
use fin_debug_server::{ChatSendRequest, ChatSendResponse, DebugBinding};
use serde::de::DeserializeOwned;
use serde_json::Value;
use std::{
    fs,
    path::{Path, PathBuf},
};

pub(crate) fn build_status_probe_response(
    runtime_home: &Path,
    binding: DebugBinding,
    request: &ChatSendRequest,
) -> Result<ChatSendResponse, CliError> {
    let last_run = read_last_run_value(runtime_home).ok();
    let progress = sibling_json::<ProgressBlock>(
        runtime_home,
        &last_run,
        "session_messages_path",
        "conversation/messages.json",
        "progress/latest.json",
    )?;
    let note = sibling_json::<ExecutionNote>(
        runtime_home,
        &last_run,
        "session_messages_path",
        "conversation/messages.json",
        "notes/latest.json",
    )?;
    let control_feedback =
        runtime_json::<ControlFeedback>(runtime_home, &last_run, "session_control_feedback_path")?
            .or(runtime_json::<ControlFeedback>(
                runtime_home,
                &last_run,
                "current_control_feedback_path",
            )?);
    let execution_state = sibling_json::<ExecutionStateRecord>(
        runtime_home,
        &last_run,
        "session_messages_path",
        "conversation/messages.json",
        "control/execution_state.json",
    )?
    .or(runtime_json::<ExecutionStateRecord>(
        runtime_home,
        &last_run,
        "current_execution_state_path",
    )?);
    let pending_inputs = sibling_json::<Vec<PendingInputRecord>>(
        runtime_home,
        &last_run,
        "session_messages_path",
        "conversation/messages.json",
        "queue/pending_inputs.json",
    )?;
    let scheduler_decision = load_latest_scheduler_decision(runtime_home, &binding)?;
    let scheduler_tick = sibling_json::<SchedulerTickRecord>(
        runtime_home,
        &last_run,
        "session_messages_path",
        "conversation/messages.json",
        "control/scheduler/latest_tick.json",
    )?
    .or(runtime_json::<SchedulerTickRecord>(
        runtime_home,
        &last_run,
        "current_scheduler_tick_path",
    )?);
    let supervisor_cycle = sibling_json::<SupervisorCycleRecord>(
        runtime_home,
        &last_run,
        "session_messages_path",
        "conversation/messages.json",
        "control/supervisor/latest.json",
    )?
    .or(runtime_json::<SupervisorCycleRecord>(
        runtime_home,
        &last_run,
        "current_supervisor_cycle_path",
    )?);
    let supervisor_heartbeat = sibling_json::<SupervisorHeartbeatRecord>(
        runtime_home,
        &last_run,
        "session_messages_path",
        "conversation/messages.json",
        "control/supervisor/latest_heartbeat.json",
    )?
    .or(runtime_json::<SupervisorHeartbeatRecord>(
        runtime_home,
        &last_run,
        "current_supervisor_heartbeat_path",
    )?);
    let daemon_state = sibling_json::<DaemonStateRecord>(
        runtime_home,
        &last_run,
        "session_messages_path",
        "conversation/messages.json",
        "control/daemon/latest_state.json",
    )?
    .or(runtime_json::<DaemonStateRecord>(
        runtime_home,
        &last_run,
        "current_daemon_state_path",
    )?);
    let daemon_recovery = sibling_json::<DaemonRecoveryActionRecord>(
        runtime_home,
        &last_run,
        "session_messages_path",
        "conversation/messages.json",
        "control/daemon/latest_recovery_action.json",
    )?
    .or(runtime_json::<DaemonRecoveryActionRecord>(
        runtime_home,
        &last_run,
        "current_daemon_recovery_action_path",
    )?);
    let routing_action = sibling_json::<RoutingActionRecord>(
        runtime_home,
        &last_run,
        "session_messages_path",
        "conversation/messages.json",
        "tasks/routing/latest_action.json",
    )?
    .or(runtime_json::<RoutingActionRecord>(
        runtime_home,
        &last_run,
        "current_routing_action_path",
    )?);

    let freshness = probe_freshness(
        progress.as_ref(),
        note.as_ref(),
        control_feedback.as_ref(),
        execution_state.as_ref(),
    );
    let digest_id = last_run
        .as_ref()
        .and_then(|value| value.get("digest_id"))
        .and_then(Value::as_str)
        .unwrap_or("status-probe")
        .to_string();

    Ok(ChatSendResponse {
        binding: binding.clone(),
        answer: render_status_answer(
            &binding,
            &request.normalized_message(),
            &freshness,
            progress.as_ref(),
            note.as_ref(),
            control_feedback.as_ref(),
            execution_state.as_ref(),
            pending_inputs.as_deref(),
            scheduler_decision.as_ref(),
            scheduler_tick.as_ref(),
            supervisor_cycle.as_ref(),
            supervisor_heartbeat.as_ref(),
            daemon_state.as_ref(),
            daemon_recovery.as_ref(),
            routing_action.as_ref(),
        ),
        digest_id,
        events_count: 0,
        response_kind: "status_probe".into(),
        freshness: Some(freshness),
        control_feedback,
        progress,
        note,
        routing_action,
    })
}

fn render_status_answer(
    binding: &DebugBinding,
    probe_message: &str,
    freshness: &str,
    progress: Option<&ProgressBlock>,
    note: Option<&ExecutionNote>,
    control_feedback: Option<&ControlFeedback>,
    execution_state: Option<&ExecutionStateRecord>,
    pending_inputs: Option<&[PendingInputRecord]>,
    scheduler_decision: Option<&SchedulerDecisionRecord>,
    scheduler_tick: Option<&SchedulerTickRecord>,
    supervisor_cycle: Option<&SupervisorCycleRecord>,
    supervisor_heartbeat: Option<&SupervisorHeartbeatRecord>,
    daemon_state: Option<&DaemonStateRecord>,
    daemon_recovery: Option<&DaemonRecoveryActionRecord>,
    routing_action: Option<&RoutingActionRecord>,
) -> String {
    let phase = execution_state
        .map(|value| value.status.as_str())
        .or_else(|| progress.map(|value| value.phase.as_str()))
        .unwrap_or("unavailable");
    let blocker = progress
        .and_then(|value| value.blocker.as_deref())
        .unwrap_or("none");
    let next_step = progress
        .and_then(|value| value.next_step.as_deref())
        .or_else(|| note.and_then(|value| value.next_step.as_deref()))
        .unwrap_or("unknown");
    let note_summary = note
        .map(|value| value.summary.as_str())
        .unwrap_or("no execution note yet");
    let control_summary = control_feedback
        .map(|value| {
            format!(
                "continuity={} shift={} simple={} continuation={} reason={}",
                value.continuity_confidence,
                value.topic_shift_confidence,
                value.simple_query_confidence,
                value.is_continuation,
                if value.reason.trim().is_empty() {
                    "unknown"
                } else {
                    value.reason.as_str()
                }
            )
        })
        .unwrap_or_else(|| "control unavailable".into());
    let active_step = execution_state
        .and_then(|value| value.active_step_id.as_deref())
        .unwrap_or("-");
    let pending_count = pending_inputs
        .map(|value| value.len())
        .or_else(|| execution_state.map(|value| value.pending_input_count))
        .unwrap_or(0);
    let resume_from = execution_state
        .and_then(|value| value.resume_from_step_id.as_deref())
        .unwrap_or("-");
    let routing_summary = routing_action
        .map(|value| {
            format!(
                "{} confidence={} prompt_user={} reason={}",
                value.action_kind, value.confidence, value.prompt_user, value.reason
            )
        })
        .unwrap_or_else(|| "routing action unavailable".into());
    let scheduler_summary = scheduler_decision
        .map(|value| {
            format!(
                "{} pending={} blocked_by={} reason={}",
                value.action_kind,
                value.pending_input_count,
                value.blocked_by.as_deref().unwrap_or("-"),
                value.reason
            )
        })
        .unwrap_or_else(|| "scheduler unavailable".into());
    let tick_summary = scheduler_tick
        .map(|value| {
            format!(
                "{} source={} drove={} final={} blocked_by={}",
                value.status,
                value.source,
                value.drove_count,
                value.final_action_kind.as_deref().unwrap_or("-"),
                value.blocked_by.as_deref().unwrap_or("-")
            )
        })
        .unwrap_or_else(|| "tick unavailable".into());
    let supervisor_summary = supervisor_cycle
        .map(|value| {
            format!(
                "{} source={} tick_count={} drove={} blocked={} final={} wake={} next_check={}",
                value.status,
                value.source,
                value.tick_count,
                value.drove_count,
                value.blocked_kind.as_deref().unwrap_or("-"),
                value.final_action_kind.as_deref().unwrap_or("-"),
                value.next_wake_hint.as_deref().unwrap_or("-"),
                value.next_check_at.as_deref().unwrap_or("-")
            )
        })
        .unwrap_or_else(|| "supervisor unavailable".into());
    let heartbeat_summary = supervisor_heartbeat
        .map(|value| {
            format!(
                "{} source={} due={} stale={} blocked={} next_check={}",
                value.status,
                value.source,
                value.due_for_tick,
                value.stale_lease,
                value.blocked_kind.as_deref().unwrap_or("-"),
                value.next_check_at.as_deref().unwrap_or("-")
            )
        })
        .unwrap_or_else(|| "heartbeat unavailable".into());
    let daemon_summary = daemon_state
        .map(|value| {
            format!(
                "{} mode={} health={} recovery_needed={}",
                value.lifecycle_state,
                value.mode,
                value.health_state.as_deref().unwrap_or("-"),
                value.recovery_needed
            )
        })
        .unwrap_or_else(|| "daemon unavailable".into());
    let recovery_summary = daemon_recovery
        .map(|value| {
            format!(
                "{} apply={} reason={}",
                value.action_kind, value.apply_immediately, value.reason
            )
        })
        .unwrap_or_else(|| "recovery unavailable".into());

    format!(
        "status probe ({freshness})\nrequest={probe_message}\nsession={}\ntask={}\nphase={phase}\nblocker={blocker}\nnext_step={next_step}\nactive_step={active_step}\nresume_from={resume_from}\npending_inputs={pending_count}\nnote={note_summary}\ncontrol={control_summary}\nrouting_action={routing_summary}\nscheduler={scheduler_summary}\ntick={tick_summary}\nsupervisor={supervisor_summary}\nheartbeat={heartbeat_summary}\ndaemon={daemon_summary}\nrecovery={recovery_summary}",
        binding.session_id.as_deref().unwrap_or("tentative"),
        binding.task_id.as_deref().unwrap_or("-"),
    )
}

fn probe_freshness(
    progress: Option<&ProgressBlock>,
    note: Option<&ExecutionNote>,
    control_feedback: Option<&ControlFeedback>,
    execution_state: Option<&ExecutionStateRecord>,
) -> String {
    if progress
        .map(|value| matches!(value.phase.as_str(), "running" | "inference_started"))
        .unwrap_or(false)
    {
        return "live".into();
    }
    if execution_state
        .map(|value| matches!(value.status.as_str(), "running" | "paused"))
        .unwrap_or(false)
    {
        return "live".into();
    }
    if progress.is_some()
        || note.is_some()
        || control_feedback.is_some()
        || execution_state.is_some()
    {
        return "recent".into();
    }
    "unavailable".into()
}

fn sibling_json<T: DeserializeOwned>(
    runtime_home: &Path,
    last_run: &Option<Value>,
    field: &str,
    source_suffix: &str,
    target_suffix: &str,
) -> Result<Option<T>, CliError> {
    let Some(path) = sibling_path(runtime_home, last_run, field, source_suffix, target_suffix)
    else {
        return Ok(None);
    };
    read_json_optional(&path)
}

fn runtime_json<T: DeserializeOwned>(
    runtime_home: &Path,
    last_run: &Option<Value>,
    field: &str,
) -> Result<Option<T>, CliError> {
    let Some(path) = runtime_path(runtime_home, last_run, field) else {
        return Ok(None);
    };
    read_json_optional(&path)
}

fn runtime_path(runtime_home: &Path, last_run: &Option<Value>, field: &str) -> Option<PathBuf> {
    last_run
        .as_ref()
        .and_then(|value| value.get(field))
        .and_then(Value::as_str)
        .map(|relative| runtime_home.join(relative))
}

fn sibling_path(
    runtime_home: &Path,
    last_run: &Option<Value>,
    field: &str,
    source_suffix: &str,
    target_suffix: &str,
) -> Option<PathBuf> {
    let relative = last_run
        .as_ref()
        .and_then(|value| value.get(field))
        .and_then(Value::as_str)?;
    relative
        .strip_suffix(source_suffix)
        .map(|prefix| runtime_home.join(format!("{prefix}{target_suffix}")))
}

fn read_json_optional<T: DeserializeOwned>(path: &Path) -> Result<Option<T>, CliError> {
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
