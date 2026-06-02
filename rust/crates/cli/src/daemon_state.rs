use crate::{
    CliError,
    daemon_state_support::{
        DaemonPaths, ensure_dirs, read_json_if_exists, read_json_or_empty, resolve_paths,
        sanitize_id, session_recent_recovery_actions_relative, session_recent_states_relative,
        trim_head, update_last_run_paths, write_json,
    },
    project_recovery::execute_project_recovery_if_needed,
    project_runtime_pickup::materialize_project_runtime_pickups,
    startup_control_summary::read_startup_control_summary,
};
use fin_config::{RuntimeRetentionConfig, SystemConfig};
use fin_contracts::{
    DaemonRecoveryActionRecord, DaemonStateRecord, DebugVisibility, EntityRefs, EventEnvelope,
    Severity, SupervisorCycleRecord, SupervisorHeartbeatRecord,
};
use fin_debug_server::DebugBinding;
use fin_runtime::append_framework_events;
use serde_json::{Value, json};
use std::path::Path;

#[derive(Debug, Clone)]
pub(crate) struct DaemonStateOutcome {
    #[allow(dead_code)]
    pub(crate) state: Option<DaemonStateRecord>,
    #[allow(dead_code)]
    pub(crate) recovery_action: Option<DaemonRecoveryActionRecord>,
}

pub(crate) fn refresh_attached_daemon_state(
    runtime_home: &Path,
    system: &SystemConfig,
    binding: &DebugBinding,
    source: &str,
    retention: &RuntimeRetentionConfig,
    recent_limit: usize,
) -> Result<DaemonStateOutcome, CliError> {
    let Some(paths) = resolve_paths(runtime_home, binding)? else {
        return Ok(DaemonStateOutcome {
            state: None,
            recovery_action: None,
        });
    };
    ensure_dirs(&paths)?;

    let now = crate::time::local_timestamp_now();
    let refs = entity_refs(binding);
    let startup_before = read_startup_control_summary(runtime_home)?;
    let daemon_id = format!(
        "daemon-web-debug-attached-{}",
        refs.session_id.as_deref().unwrap_or("tentative")
    );
    let latest_cycle =
        read_json_if_exists::<SupervisorCycleRecord>(&paths.latest_supervisor_cycle_path())?;
    let latest_heartbeat = read_json_if_exists::<SupervisorHeartbeatRecord>(
        &paths.latest_supervisor_heartbeat_path(),
    )?;
    let recovery_action = derive_recovery_action(
        source,
        &now,
        &refs,
        &startup_before,
        latest_cycle.as_ref(),
        latest_heartbeat.as_ref(),
    );
    let recovery_execution = execute_project_recovery_if_needed(
        runtime_home,
        system,
        source,
        recovery_action.as_ref(),
        &now,
    )?;
    let runtime_pickup = materialize_project_runtime_pickups(runtime_home, system, &now)?;
    let startup_after = read_startup_control_summary(runtime_home)?;
    let state = derive_daemon_state(
        &daemon_id,
        &now,
        source,
        binding,
        &refs,
        &startup_after,
        latest_cycle.as_ref(),
        latest_heartbeat.as_ref(),
        recovery_action.as_ref(),
        recovery_execution
            .as_ref()
            .map(|item| item.summary.as_str()),
        Some(runtime_pickup.status_summary().as_str()),
    );

    let mut events = vec![daemon_event(
        &daemon_id,
        1,
        "daemon.state_recorded",
        &now,
        &refs,
        serde_json::to_value(&state).map_err(CliError::Serialize)?,
    )];
    if let Some(action) = &recovery_action {
        events.push(daemon_event(
            &daemon_id,
            2,
            "daemon.recovery_action_derived",
            &now,
            &refs,
            serde_json::to_value(action).map_err(CliError::Serialize)?,
        ));
    }
    append_framework_events(runtime_home, &paths.session_dir, &events, retention)?;
    persist_daemon_state(
        runtime_home,
        &paths,
        &state,
        recovery_action.as_ref(),
        recent_limit,
    )?;

    Ok(DaemonStateOutcome {
        state: Some(state),
        recovery_action,
    })
}

fn derive_daemon_state(
    daemon_id: &str,
    now: &str,
    source: &str,
    binding: &DebugBinding,
    refs: &EntityRefs,
    startup: &crate::startup_control_summary::StartupControlSummary,
    latest_cycle: Option<&SupervisorCycleRecord>,
    latest_heartbeat: Option<&SupervisorHeartbeatRecord>,
    recovery_action: Option<&DaemonRecoveryActionRecord>,
    recovery_execution_summary: Option<&str>,
    runtime_pickup_summary: Option<&str>,
) -> DaemonStateRecord {
    let health_state = if latest_heartbeat.is_some_and(|value| value.stale_lease) {
        Some("stale".into())
    } else if latest_heartbeat.is_some() {
        Some("healthy".into())
    } else {
        Some("unknown".into())
    };
    let recovery_needed = if recovery_execution_summary.is_some()
        && recovery_action
            .as_ref()
            .is_some_and(|value| value.action_kind == "recover_project_agents")
    {
        startup.recoverable_offline_count > 0
    } else {
        recovery_action
            .as_ref()
            .is_some_and(|value| value.apply_immediately || value.action_kind != "observe_only")
    };
    let active_binding = Some(format!(
        "session={} task={}",
        binding.session_id.as_deref().unwrap_or("tentative"),
        binding.task_id.as_deref().unwrap_or("-")
    ));

    DaemonStateRecord {
        daemon_id: daemon_id.into(),
        created_at: now.into(),
        updated_at: now.into(),
        refs: refs.clone(),
        service_kind: "web_debug_attached".into(),
        lifecycle_state: "attached_active".into(),
        supervision_state: derive_supervision_state(startup, latest_cycle),
        mode: "attached".into(),
        pid: Some(std::process::id()),
        last_heartbeat_id: latest_heartbeat.map(|value| value.heartbeat_id.clone()),
        last_cycle_id: latest_cycle.map(|value| value.cycle_id.clone()),
        health_state,
        recovery_needed,
        recovery_action_kind: recovery_action.map(|value| value.action_kind.clone()),
        active_binding,
        status_summary: format!(
            "daemon source={} blocked_kind={} stale_lease={} recovery={} startup={} recovery_exec={} runtime_pickup={}",
            source,
            latest_cycle
                .and_then(|value| value.blocked_kind.as_deref())
                .unwrap_or("-"),
            latest_heartbeat.is_some_and(|value| value.stale_lease),
            recovery_action
                .as_ref()
                .map(|value| value.action_kind.as_str())
                .unwrap_or("none"),
            startup.status_summary(),
            recovery_execution_summary.unwrap_or("-"),
            runtime_pickup_summary.unwrap_or("-"),
        ),
    }
}

fn derive_recovery_action(
    source: &str,
    now: &str,
    refs: &EntityRefs,
    startup: &crate::startup_control_summary::StartupControlSummary,
    latest_cycle: Option<&SupervisorCycleRecord>,
    latest_heartbeat: Option<&SupervisorHeartbeatRecord>,
) -> Option<DaemonRecoveryActionRecord> {
    let (action_kind, apply_immediately, reason) =
        if latest_heartbeat.is_some_and(|value| value.stale_lease) {
            (
                "recover_stale_cycle",
                true,
                "supervisor heartbeat detected stale lease".to_string(),
            )
        } else if latest_cycle
            .is_some_and(|value| value.blocked_kind.as_deref() == Some("wait_running"))
        {
            (
                "continue_heartbeat_monitoring",
                false,
                "cycle is waiting on running closure".to_string(),
            )
        } else if latest_cycle
            .is_some_and(|value| value.blocked_kind.as_deref() == Some("wait_external"))
        {
            (
                "await_external_event",
                false,
                "cycle is waiting on external event or reminder".to_string(),
            )
        } else if latest_cycle
            .is_some_and(|value| value.blocked_kind.as_deref() == Some("await_user_confirmation"))
        {
            (
                "await_user_confirmation",
                false,
                "cycle requires explicit user confirmation".to_string(),
            )
        } else if startup.recoverable_offline_count > 0 {
            (
                "recover_project_agents",
                true,
                format!(
                    "{} project agent(s) are offline but recoverable",
                    startup.recoverable_offline_count
                ),
            )
        } else if startup.waiting_project_count > 0 || startup.last_wake_had_remote_wait {
            (
                "monitor_project_remote_connectivity",
                false,
                format!(
                    "{} project agent(s) are waiting on remote connectivity",
                    startup.waiting_project_count
                ),
            )
        } else {
            ("observe_only", false, "daemon state observed".to_string())
        };

    Some(DaemonRecoveryActionRecord {
        action_id: format!(
            "daemon-recovery-{}-{}",
            refs.session_id.as_deref().unwrap_or("tentative"),
            sanitize_id(now)
        ),
        created_at: now.into(),
        refs: refs.clone(),
        source: source.into(),
        action_kind: action_kind.into(),
        apply_immediately,
        target_heartbeat_id: latest_heartbeat.map(|value| value.heartbeat_id.clone()),
        target_cycle_id: latest_cycle.map(|value| value.cycle_id.clone()),
        reason,
    })
}

fn derive_supervision_state(
    startup: &crate::startup_control_summary::StartupControlSummary,
    latest_cycle: Option<&SupervisorCycleRecord>,
) -> String {
    if startup.recoverable_offline_count > 0 {
        "project_recovery_needed".into()
    } else if startup.waiting_project_count > 0 || startup.last_wake_had_remote_wait {
        "project_remote_waiting".into()
    } else {
        latest_cycle
            .and_then(|value| value.blocked_kind.clone())
            .unwrap_or_else(|| "observing".into())
    }
}

fn daemon_event(
    daemon_id: &str,
    sequence: u64,
    event_type: &str,
    occurred_at: &str,
    refs: &EntityRefs,
    payload: Value,
) -> EventEnvelope<Value> {
    let mut event = EventEnvelope::new(
        format!("{daemon_id}-evt-{sequence:02}"),
        event_type,
        occurred_at.to_string(),
        "cli.daemon_state",
        daemon_id.to_string(),
        sequence,
        payload,
    );
    event.refs = refs.clone();
    event.severity = Severity::Info;
    event.debug_visibility = DebugVisibility::Important;
    event
}

fn persist_daemon_state(
    runtime_home: &Path,
    paths: &DaemonPaths,
    state: &DaemonStateRecord,
    recovery_action: Option<&DaemonRecoveryActionRecord>,
    recent_limit: usize,
) -> Result<(), CliError> {
    let mut states = read_json_or_empty::<DaemonStateRecord>(&paths.recent_daemon_states_path())?;
    states.push(state.clone());
    trim_head(&mut states, recent_limit);
    write_json(&paths.recent_daemon_states_path(), &states)?;
    write_json(&paths.latest_daemon_state_path(), state)?;
    write_json(
        &runtime_home.join("runtime/current/current_daemon_state.json"),
        state,
    )?;
    if let Some(action) = recovery_action {
        let mut actions = read_json_or_empty::<DaemonRecoveryActionRecord>(
            &paths.recent_recovery_actions_path(),
        )?;
        actions.push(action.clone());
        trim_head(&mut actions, recent_limit);
        write_json(&paths.recent_recovery_actions_path(), &actions)?;
        write_json(&paths.latest_recovery_action_path(), action)?;
        write_json(
            &runtime_home.join("runtime/current/current_daemon_recovery_action.json"),
            action,
        )?;
    }
    update_last_run_paths(
        runtime_home,
        json!({
            "current_daemon_state_path": "runtime/current/current_daemon_state.json",
            "session_recent_daemon_states_path": session_recent_states_relative(paths, runtime_home)?,
            "current_daemon_recovery_action_path": "runtime/current/current_daemon_recovery_action.json",
            "session_recent_daemon_recovery_actions_path": session_recent_recovery_actions_relative(paths, runtime_home)?,
        }),
    )
}

fn entity_refs(binding: &DebugBinding) -> EntityRefs {
    EntityRefs {
        session_id: binding.session_id.clone(),
        task_id: binding.task_id.clone(),
        ..EntityRefs::default()
    }
}
