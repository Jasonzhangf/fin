use crate::{
    CliError, process_utils::append_log, runtime_home::read_last_run_value,
    session_binding::build_binding_for_session, web_debug_support::build_debug_binding,
};
use fin_config::SystemConfig;
use fin_contracts::{
    ContextSnapshotRecord, DaemonRecoveryActionRecord, DaemonStateRecord, EntityRefs,
};
use fin_debug_server::DebugBinding;
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command as ProcessCommand, Stdio},
};

pub(super) const SERVICE_KIND: &str = "headless_daemon";
pub(super) const MODE: &str = "detached";

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(super) struct HeadlessDaemonLeaseRecord {
    pub(super) daemon_id: String,
    pub(super) pid: u32,
    pub(super) heartbeat_interval_ms: u64,
    pub(super) lease_ttl_ms: u64,
    pub(super) started_at: String,
    pub(super) updated_at: String,
    pub(super) lifecycle_state: String,
    pub(super) active_session_ids: Vec<String>,
    pub(super) processed_sessions: usize,
    pub(super) drove_count: usize,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct HeadlessCycleSummary {
    pub(super) processed_sessions: usize,
    pub(super) drove_count: usize,
    pub(super) active_session_ids: Vec<String>,
}

#[derive(Debug, Clone)]
pub(super) struct ManagedSession {
    pub(super) session_id: String,
    pub(super) binding: DebugBinding,
    pub(super) role_id: String,
}

#[derive(Debug, Clone)]
pub(super) struct HeadlessDaemonPaths {
    pub(super) pid_path: PathBuf,
    pub(super) lease_path: PathBuf,
    pub(super) state_path: PathBuf,
    pub(super) recovery_path: PathBuf,
    pub(super) stop_request_path: PathBuf,
    pub(super) log_path: PathBuf,
}

impl HeadlessDaemonPaths {
    pub(super) fn new(runtime_home: &Path) -> Self {
        Self {
            pid_path: runtime_home.join("runtime/pids/headless-daemon.pid"),
            lease_path: runtime_home.join("runtime/leases/headless-daemon.json"),
            state_path: runtime_home.join("runtime/current/current_daemon_state.json"),
            recovery_path: runtime_home.join("runtime/current/current_daemon_recovery_action.json"),
            stop_request_path: runtime_home.join("runtime/locks/headless-daemon.stop"),
            log_path: runtime_home.join("logs/runtime/headless-daemon.log"),
        }
    }
}

pub(super) fn discover_sessions_with_work(
    runtime_home: &Path,
    system: &SystemConfig,
) -> Result<Vec<ManagedSession>, CliError> {
    let mut managed = Vec::new();
    let base = build_debug_binding(runtime_home, &read_last_run_value(runtime_home).ok());
    for session_dir in session_dirs(runtime_home)? {
        let session_id = session_dir
            .file_name()
            .and_then(|value| value.to_str())
            .ok_or(CliError::Usage)?
            .to_string();
        if !session_has_work(&session_dir)? {
            continue;
        }
        let binding = build_binding_for_session(runtime_home, &base, &session_id, None)?;
        let role_id = infer_session_role(runtime_home, system, &session_id)?;
        managed.push(ManagedSession {
            session_id,
            binding,
            role_id,
        });
    }
    managed.sort_by(|a, b| a.session_id.cmp(&b.session_id));
    Ok(managed)
}

pub(super) fn daemon_id(system: &SystemConfig) -> String {
    format!(
        "{}.headless-daemon",
        fin_runtime::resolve_device_name(system)
    )
}

pub(super) fn daemon_state(
    daemon_id: &str,
    session_id: Option<String>,
    lifecycle_state: &str,
    source: &str,
    supervision_state: &str,
    health_state: Option<&str>,
    startup: &crate::startup_control_summary::StartupControlSummary,
    cycle: &HeadlessCycleSummary,
) -> DaemonStateRecord {
    DaemonStateRecord {
        daemon_id: daemon_id.into(),
        created_at: crate::time::local_timestamp_now(),
        updated_at: crate::time::local_timestamp_now(),
        refs: EntityRefs {
            session_id,
            ..EntityRefs::default()
        },
        service_kind: SERVICE_KIND.into(),
        lifecycle_state: lifecycle_state.into(),
        supervision_state: supervision_state.into(),
        mode: MODE.into(),
        pid: Some(std::process::id()),
        last_heartbeat_id: None,
        last_cycle_id: None,
        health_state: health_state.map(str::to_string),
        recovery_needed: startup.recoverable_offline_count > 0,
        recovery_action_kind: Some(if startup.recoverable_offline_count > 0 {
            "recover_project_agents".into()
        } else {
            "observe_only".into()
        }),
        active_binding: cycle
            .active_session_ids
            .first()
            .map(|value| format!("session={value}")),
        status_summary: format!(
            "daemon source={} startup={} processed_sessions={} drove={} active_sessions={}",
            source,
            startup.status_summary(),
            cycle.processed_sessions,
            cycle.drove_count,
            cycle.active_session_ids.join(",")
        ),
    }
}

pub(super) fn daemon_recovery(
    daemon_id: &str,
    startup: &crate::startup_control_summary::StartupControlSummary,
    created_at: &str,
) -> DaemonRecoveryActionRecord {
    DaemonRecoveryActionRecord {
        action_id: format!("{}-recovery-{}", daemon_id, sanitize_id(created_at)),
        created_at: created_at.into(),
        refs: EntityRefs::default(),
        source: SERVICE_KIND.into(),
        action_kind: if startup.recoverable_offline_count > 0 {
            "recover_project_agents".into()
        } else {
            "observe_only".into()
        },
        apply_immediately: startup.recoverable_offline_count > 0,
        target_heartbeat_id: None,
        target_cycle_id: None,
        reason: startup.status_summary(),
    }
}

pub(super) fn persist_lease(
    paths: &HeadlessDaemonPaths,
    lease: &HeadlessDaemonLeaseRecord,
) -> Result<(), CliError> {
    write_json(&paths.lease_path, lease)?;
    fs::write(&paths.pid_path, lease.pid.to_string()).map_err(|source| CliError::WriteFile {
        path: paths.pid_path.display().to_string(),
        source,
    })?;
    append_log(
        &paths.log_path,
        format!(
            "[{}] heartbeat pid={} sessions={} drove={}\n",
            lease.updated_at, lease.pid, lease.processed_sessions, lease.drove_count
        )
        .as_str(),
    )
}

pub(super) fn persist_daemon_state(
    paths: &HeadlessDaemonPaths,
    state: &DaemonStateRecord,
) -> Result<(), CliError> {
    write_json(&paths.state_path, state)
}

pub(super) fn persist_recovery_action(
    paths: &HeadlessDaemonPaths,
    recovery: &DaemonRecoveryActionRecord,
) -> Result<(), CliError> {
    write_json(&paths.recovery_path, recovery)
}

pub(super) fn stop_requested(paths: &HeadlessDaemonPaths) -> bool {
    paths.stop_request_path.exists()
}

pub(super) fn process_alive(pid: u32) -> bool {
    ProcessCommand::new("kill")
        .args(["-0", &pid.to_string()])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

pub(super) fn read_json_if_exists<T: for<'de> Deserialize<'de>>(
    path: &Path,
) -> Result<Option<T>, CliError> {
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

fn infer_session_role(
    runtime_home: &Path,
    system: &SystemConfig,
    session_id: &str,
) -> Result<String, CliError> {
    let Some((_, _, session_dir)) =
        crate::session_binding::find_session_dir(runtime_home, session_id)
    else {
        return Ok(system.policy.entry_role.clone());
    };
    let contexts = read_json_or_empty::<ContextSnapshotRecord>(
        &session_dir.join("context/recent_contexts.json"),
    )?;
    Ok(contexts
        .last()
        .map(|value| value.role.role_id.as_str().to_string())
        .unwrap_or_else(|| system.policy.entry_role.clone()))
}

fn session_has_work(session_dir: &Path) -> Result<bool, CliError> {
    let state = read_json_if_exists::<fin_contracts::ExecutionStateRecord>(
        &session_dir.join("control/execution_state.json"),
    )?;
    if state
        .as_ref()
        .is_some_and(|value| value.status != "idle")
    {
        return Ok(true);
    }
    let pending = read_json_or_empty::<fin_contracts::PendingInputRecord>(
        &session_dir.join("queue/pending_inputs.json"),
    )?;
    if !pending.is_empty() {
        return Ok(true);
    }
    let checkpoint = read_json_if_exists::<fin_contracts::ExecutionCheckpointRecord>(
        &session_dir.join("control/execution_checkpoint.json"),
    )?;
    Ok(checkpoint
        .as_ref()
        .is_some_and(|value| value.status == "open"))
}

fn session_dirs(runtime_home: &Path) -> Result<Vec<PathBuf>, CliError> {
    let mut dirs = Vec::new();
    let root = runtime_home.join("sessions");
    if !root.exists() {
        return Ok(dirs);
    }
    for year in fs::read_dir(&root).map_err(|source| CliError::ReadFile {
        path: root.display().to_string(),
        source,
    })? {
        let year = year.map_err(|source| CliError::ReadFile {
            path: root.display().to_string(),
            source,
        })?;
        for month in fs::read_dir(year.path()).map_err(|source| CliError::ReadFile {
            path: year.path().display().to_string(),
            source,
        })? {
            let month = month.map_err(|source| CliError::ReadFile {
                path: year.path().display().to_string(),
                source,
            })?;
            for session in fs::read_dir(month.path()).map_err(|source| CliError::ReadFile {
                path: month.path().display().to_string(),
                source,
            })? {
                dirs.push(
                    session
                        .map_err(|source| CliError::ReadFile {
                            path: month.path().display().to_string(),
                            source,
                        })?
                        .path(),
                );
            }
        }
    }
    Ok(dirs)
}

fn sanitize_id(value: &str) -> String {
    crate::demo::sanitize_id_fragment(value)
}

fn read_json_or_empty<T: for<'de> Deserialize<'de>>(path: &Path) -> Result<Vec<T>, CliError> {
    match fs::read_to_string(path) {
        Ok(content) => serde_json::from_str(&content).map_err(CliError::Serialize),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(source) => Err(CliError::ReadFile {
            path: path.display().to_string(),
            source,
        }),
    }
}
