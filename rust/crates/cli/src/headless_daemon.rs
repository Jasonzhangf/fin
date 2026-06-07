use crate::{
    CliError, agent_presence::ensure_entry_agent_presence,
    headless_daemon_bridge::maybe_start_builtin_qqbot_bridge,
    headless_daemon_project_resume::drive_headless_project_runtime_resumes,
    reminder_scheduler::inject_due_reminders, runtime_home::init_runtime_home,
    execution_state::clear_waiting_if_due,
    startup_control_summary::read_startup_control_summary,
    startup_wakeup::refresh_startup_control_plane, supervisor_cycle::run_supervisor_cycle,
    web_debug::CliDebugActionHandler,
};
use fin_config::SystemConfig;
use fin_debug_server::{ChatSendResponse, DebugBinding};
use fin_provider::InferenceProvider;
use headless_daemon_support::{
    HeadlessCycleSummary, HeadlessDaemonLeaseRecord, HeadlessDaemonPaths, ManagedSession,
    daemon_id, daemon_recovery, daemon_state, discover_sessions_with_work, persist_daemon_state,
    persist_lease, persist_recovery_action, process_alive, read_json_if_exists, stop_requested,
};
use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command as ProcessCommand, Stdio},
    thread,
    time::Duration,
};

#[path = "headless_daemon_support.rs"]
mod headless_daemon_support;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct HeadlessDaemonStartReport {
    pub(crate) daemon_id: String,
    pub(crate) status: String,
    pub(crate) pid: Option<u32>,
    pub(crate) runtime_home: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct HeadlessDaemonStopReport {
    pub(crate) daemon_id: String,
    pub(crate) status: String,
    pub(crate) pid: Option<u32>,
    pub(crate) runtime_home: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct HeadlessDaemonRunReport {
    pub(crate) daemon_id: String,
    pub(crate) cycles_completed: usize,
    pub(crate) processed_sessions: usize,
    pub(crate) drove_count: usize,
    pub(crate) runtime_home: PathBuf,
}

pub(crate) fn start_headless_daemon(
    user_toml: &str,
    system: &SystemConfig,
    override_path: Option<&Path>,
) -> Result<HeadlessDaemonStartReport, CliError> {
    let runtime_home = init_runtime_home(user_toml, system, override_path)?;
    let daemon_id = daemon_id(system);
    let paths = HeadlessDaemonPaths::new(&runtime_home);
    if let Some(record) = read_json_if_exists::<HeadlessDaemonLeaseRecord>(&paths.lease_path)? {
        if process_alive(record.pid) {
            return Ok(HeadlessDaemonStartReport {
                daemon_id,
                status: "already_running".into(),
                pid: Some(record.pid),
                runtime_home,
            });
        }
    }
    let _ = fs::remove_file(&paths.stop_request_path);
    let _ = fs::remove_file(&paths.pid_path);
    let stdout = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&paths.log_path)
        .map_err(|source| CliError::WriteFile {
            path: paths.log_path.display().to_string(),
            source,
        })?;
    let stderr = stdout.try_clone().map_err(|source| CliError::WriteFile {
        path: paths.log_path.display().to_string(),
        source,
    })?;
    let child =
        ProcessCommand::new(
            std::env::current_exe().map_err(|source| CliError::ReadFile {
                path: "current_exe".into(),
                source,
            })?,
        )
        .arg("daemon-run")
        .arg(runtime_home.join("config/user.toml"))
        .stdin(Stdio::null())
        .stdout(Stdio::from(stdout))
        .stderr(Stdio::from(stderr))
        .spawn()
        .map_err(|source| CliError::ReadFile {
            path: "spawn daemon-run".into(),
            source,
        })?;
    fs::write(&paths.pid_path, child.id().to_string()).map_err(|source| CliError::WriteFile {
        path: paths.pid_path.display().to_string(),
        source,
    })?;
    Ok(HeadlessDaemonStartReport {
        daemon_id,
        status: "started".into(),
        pid: Some(child.id()),
        runtime_home,
    })
}

pub(crate) fn stop_headless_daemon(
    user_toml: &str,
    system: &SystemConfig,
    override_path: Option<&Path>,
) -> Result<HeadlessDaemonStopReport, CliError> {
    let runtime_home = init_runtime_home(user_toml, system, override_path)?;
    let daemon_id = daemon_id(system);
    let paths = HeadlessDaemonPaths::new(&runtime_home);
    let pid = fs::read_to_string(&paths.pid_path)
        .ok()
        .and_then(|value| value.trim().parse::<u32>().ok());
    fs::write(&paths.stop_request_path, crate::time::local_timestamp_now()).map_err(|source| {
        CliError::WriteFile {
            path: paths.stop_request_path.display().to_string(),
            source,
        }
    })?;
    Ok(HeadlessDaemonStopReport {
        daemon_id,
        status: "stop_requested".into(),
        pid,
        runtime_home,
    })
}

pub(crate) fn run_headless_daemon(
    user_toml: &str,
    system: &SystemConfig,
    provider: &impl InferenceProvider,
    override_path: Option<&Path>,
) -> Result<HeadlessDaemonRunReport, CliError> {
    let runtime_home = init_runtime_home(user_toml, system, override_path)?;
    run_headless_daemon_with_provider(user_toml, system, provider, runtime_home)
}

pub(crate) fn run_headless_daemon_with_provider(
    user_toml: &str,
    system: &SystemConfig,
    provider: &impl InferenceProvider,
    runtime_home: PathBuf,
) -> Result<HeadlessDaemonRunReport, CliError> {
    let daemon_id = daemon_id(system);
    let paths = HeadlessDaemonPaths::new(&runtime_home);
    let heartbeat_interval_ms = system.runtime.heartbeat_interval_ms.max(50);
    let lease_ttl_ms = heartbeat_interval_ms.saturating_mul(3);
    let max_cycles = std::env::var("FIN_HEADLESS_DAEMON_MAX_CYCLES")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(usize::MAX);
    let started_at = crate::time::local_timestamp_now();
    let handler = CliDebugActionHandler::new(user_toml.to_string(), system.clone())?;
    let _ = ensure_entry_agent_presence(system, &runtime_home, &started_at)?;
    let _qqbot_bridge = match maybe_start_builtin_qqbot_bridge(&runtime_home, &handler) {
        Ok(bridge) => bridge,
        Err(e) => {
            eprintln!("[daemon] qqbot bridge start failed: {e}");
            None
        }
    };
    let mut cycles_completed = 0usize;
    let mut processed_sessions = 0usize;
    let mut drove_count = 0usize;

    loop {
        if stop_requested(&paths) {
            persist_daemon_state(
                &paths,
                &daemon_state(
                    &daemon_id,
                    None,
                    "stopped",
                    "stop_requested",
                    "idle_watch",
                    Some("healthy"),
                    &read_startup_control_summary(&runtime_home).unwrap_or_default(),
                    &HeadlessCycleSummary::default(),
                ),
            )?;
            break;
        }
        let cycle_now = crate::time::local_timestamp_now();
        let startup = refresh_startup_control_plane(&runtime_home, system, &cycle_now)?;
        let project_resume_drove = drive_headless_project_runtime_resumes(
            &runtime_home,
            system,
            provider,
            &handler,
            &cycle_now,
        )?;
        let sessions = discover_sessions_with_work(&runtime_home, system)?;
        let mut cycle = run_headless_cycle(
            &runtime_home,
            system,
            provider,
            &handler,
            sessions.as_slice(),
        )?;
        cycle.drove_count = cycle.drove_count.saturating_add(project_resume_drove);
        processed_sessions = processed_sessions.saturating_add(cycle.processed_sessions);
        drove_count = drove_count.saturating_add(cycle.drove_count);
        cycles_completed = cycles_completed.saturating_add(1);
        persist_lease(
            &paths,
            &HeadlessDaemonLeaseRecord {
                daemon_id: daemon_id.clone(),
                pid: std::process::id(),
                heartbeat_interval_ms,
                lease_ttl_ms,
                started_at: started_at.clone(),
                updated_at: cycle_now.clone(),
                lifecycle_state: "active".into(),
                active_session_ids: cycle.active_session_ids.clone(),
                processed_sessions: cycle.processed_sessions,
                drove_count: cycle.drove_count,
            },
        )?;
        let startup_summary = read_startup_control_summary(&runtime_home).unwrap_or_else(|_| {
            let mut summary = crate::startup_control_summary::StartupControlSummary::default();
            summary.updated_at = startup.updated_at;
            summary
        });
        persist_daemon_state(
            &paths,
            &daemon_state(
                &daemon_id,
                cycle.active_session_ids.first().cloned(),
                "active",
                "headless_loop",
                if cycle.processed_sessions > 0 {
                    "running_sessions"
                } else {
                    "idle_watch"
                },
                Some("healthy"),
                &startup_summary,
                &cycle,
            ),
        )?;
        persist_recovery_action(
            &paths,
            &daemon_recovery(&daemon_id, &startup_summary, &cycle_now),
        )?;
        if cycles_completed >= max_cycles {
            persist_daemon_state(
                &paths,
                &daemon_state(
                    &daemon_id,
                    cycle.active_session_ids.first().cloned(),
                    "stopped",
                    "max_cycles_reached",
                    "idle_watch",
                    Some("healthy"),
                    &startup_summary,
                    &cycle,
                ),
            )?;
            break;
        }
        thread::sleep(Duration::from_millis(heartbeat_interval_ms));
    }

    let _ = fs::remove_file(&paths.pid_path);
    let _ = fs::remove_file(&paths.stop_request_path);
    Ok(HeadlessDaemonRunReport {
        daemon_id,
        cycles_completed,
        processed_sessions,
        drove_count,
        runtime_home,
    })
}

fn run_headless_cycle(
    runtime_home: &Path,
    system: &SystemConfig,
    provider: &impl InferenceProvider,
    handler: &CliDebugActionHandler,
    sessions: &[ManagedSession],
) -> Result<HeadlessCycleSummary, CliError> {
    let mut summary = HeadlessCycleSummary::default();
    for session in sessions {
        let fired = inject_due_reminders(runtime_home, &session.binding)?;
        if fired > 0 {
            clear_waiting_if_due(runtime_home, &session.binding, &crate::time::local_timestamp_now())?;
        }
        let outcome = run_supervisor_cycle(
            runtime_home,
            &session.binding,
            "daemon_headless",
            system.runtime.heartbeat_interval_ms,
            &system.runtime.retention,
            system.runtime.retention.recent_routing_decision_limit,
            |binding, message, source, attachments, merge_segment| {
                run_binding_turn(
                    runtime_home,
                    provider,
                    handler,
                    &session.role_id,
                    binding,
                    message,
                    &source,
                    attachments,
                    merge_segment,
                )
            },
        )?;
        if outcome.cycle.is_some() {
            summary.processed_sessions += 1;
            summary.drove_count += outcome.tick.drive.drove_count;
            summary.active_session_ids.push(session.session_id.clone());
        }
    }
    summary.active_session_ids.sort();
    summary.active_session_ids.dedup();
    Ok(summary)
}

fn run_binding_turn(
    runtime_home: &Path,
    provider: &impl InferenceProvider,
    handler: &CliDebugActionHandler,
    role_id: &str,
    binding: DebugBinding,
    message: String,
    source: &str,
    attachments: Vec<fin_contracts::InputAttachmentSummary>,
    merge_segment: Option<&fin_contracts::InterruptedSegmentRecord>,
) -> Result<ChatSendResponse, CliError> {
    if role_id == "project" {
        handler.run_project_turn_with_provider(
            runtime_home,
            binding,
            message,
            source,
            attachments,
            provider,
            merge_segment,
            None,
        )
    } else {
        handler.run_chat_turn_with_provider(
            runtime_home,
            binding,
            message,
            source,
            attachments,
            provider,
            merge_segment,
        )
    }
}
