use crate::{
    CliError,
    headless_daemon::headless_daemon_support::{
        HeadlessDaemonLeaseRecord, HeadlessDaemonPaths, lease_is_stale, persist_daemon_state,
        persist_recovery_action, process_alive, read_json_if_exists,
    },
    startup_control_summary::read_startup_control_summary,
};
use fin_contracts::{DaemonRecoveryActionRecord, DaemonStateRecord, EntityRefs};
use std::{fs, path::Path};

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub(crate) struct HeadlessDaemonReconcileOutcome {
    pub(crate) state: DaemonStateRecord,
    pub(crate) recovery: Option<DaemonRecoveryActionRecord>,
}

pub(crate) fn reconcile_headless_daemon_state(
    runtime_home: &Path,
) -> Result<Option<HeadlessDaemonReconcileOutcome>, CliError> {
    let paths = HeadlessDaemonPaths::new(runtime_home);
    let state = read_json_if_exists::<DaemonStateRecord>(&paths.state_path)?;
    let lease = read_json_if_exists::<HeadlessDaemonLeaseRecord>(&paths.lease_path)?;
    let pid = state
        .as_ref()
        .and_then(|value| value.pid)
        .or_else(|| lease.as_ref().map(|value| value.pid))
        .or_else(|| {
            fs::read_to_string(&paths.pid_path)
                .ok()
                .and_then(|value| value.trim().parse::<u32>().ok())
        });
    let Some(pid) = pid else {
        return Ok(None);
    };
    let alive = process_alive(pid);
    let stale_lease = lease.as_ref().is_some_and(lease_is_stale);
    if alive && !stale_lease {
        return Ok(None);
    }

    let startup = read_startup_control_summary(runtime_home).unwrap_or_default();
    let daemon_id = state
        .as_ref()
        .map(|value| value.daemon_id.clone())
        .or_else(|| lease.as_ref().map(|value| value.daemon_id.clone()))
        .unwrap_or_else(|| "headless-daemon".into());
    let active_session_ids = lease
        .as_ref()
        .map(|value| value.active_session_ids.clone())
        .unwrap_or_default();
    let assignment_pending_count = fin_runtime::read_assignment_queue(runtime_home)
        .map_err(CliError::Runtime)?
        .len();
    let recovery_needed =
        stale_lease || !active_session_ids.is_empty() || assignment_pending_count > 0;
    let health_state = if alive { "stale" } else { "dead" };
    let lifecycle_state = if alive { "active" } else { "stopped" };
    let supervision_state = if recovery_needed {
        "restart_required"
    } else {
        "idle_watch"
    };
    let recovery_action_kind = if recovery_needed {
        "restart_headless_daemon"
    } else {
        "observe_only"
    };
    let now = crate::time::local_timestamp_now();
    let summary = format!(
        "daemon reconciled pid={} alive={} stale_lease={} startup={} active_sessions={} pending_assignments={}",
        pid,
        alive,
        stale_lease,
        startup.status_summary(),
        active_session_ids.join(","),
        assignment_pending_count,
    );
    let reconciled_state = DaemonStateRecord {
        daemon_id: daemon_id.clone(),
        created_at: now.clone(),
        updated_at: now.clone(),
        refs: EntityRefs {
            session_id: active_session_ids.first().cloned(),
            ..EntityRefs::default()
        },
        service_kind: "headless_daemon".into(),
        lifecycle_state: lifecycle_state.into(),
        supervision_state: supervision_state.into(),
        mode: "detached".into(),
        pid: Some(pid),
        last_heartbeat_id: None,
        last_cycle_id: None,
        health_state: Some(health_state.into()),
        recovery_needed,
        recovery_action_kind: Some(recovery_action_kind.into()),
        active_binding: active_session_ids
            .first()
            .map(|session_id| format!("session={session_id}")),
        status_summary: summary.clone(),
    };
    persist_daemon_state(&paths, &reconciled_state)?;
    let recovery = DaemonRecoveryActionRecord {
        action_id: format!(
            "{}-recovery-{}",
            daemon_id,
            crate::session_run::sanitize_id_fragment(&now)
        ),
        created_at: now,
        refs: EntityRefs::default(),
        source: "headless_daemon_reconcile".into(),
        action_kind: recovery_action_kind.into(),
        apply_immediately: recovery_needed,
        target_heartbeat_id: None,
        target_cycle_id: None,
        reason: summary,
    };
    persist_recovery_action(&paths, &recovery)?;
    if !alive {
        let _ = fs::remove_file(&paths.pid_path);
    }
    Ok(Some(HeadlessDaemonReconcileOutcome {
        state: reconciled_state,
        recovery: Some(recovery),
    }))
}
