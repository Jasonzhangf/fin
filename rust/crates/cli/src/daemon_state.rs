use crate::CliError;
use fin_config::RuntimeRetentionConfig;
use fin_contracts::{
    DaemonRecoveryActionRecord, DaemonStateRecord, DebugVisibility, EntityRefs, EventEnvelope,
    Severity, SupervisorCycleRecord, SupervisorHeartbeatRecord,
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
struct DaemonPaths {
    session_dir: PathBuf,
}

impl DaemonPaths {
    fn latest_daemon_state_path(&self) -> PathBuf {
        self.session_dir.join("control/daemon/latest_state.json")
    }

    fn recent_daemon_states_path(&self) -> PathBuf {
        self.session_dir.join("control/daemon/recent_states.json")
    }

    fn latest_recovery_action_path(&self) -> PathBuf {
        self.session_dir
            .join("control/daemon/latest_recovery_action.json")
    }

    fn recent_recovery_actions_path(&self) -> PathBuf {
        self.session_dir
            .join("control/daemon/recent_recovery_actions.json")
    }

    fn latest_supervisor_cycle_path(&self) -> PathBuf {
        self.session_dir.join("control/supervisor/latest.json")
    }

    fn latest_supervisor_heartbeat_path(&self) -> PathBuf {
        self.session_dir
            .join("control/supervisor/latest_heartbeat.json")
    }
}

#[derive(Debug, Clone)]
pub(crate) struct DaemonStateOutcome {
    #[allow(dead_code)]
    pub(crate) state: Option<DaemonStateRecord>,
    #[allow(dead_code)]
    pub(crate) recovery_action: Option<DaemonRecoveryActionRecord>,
}

pub(crate) fn refresh_attached_daemon_state(
    runtime_home: &Path,
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
        latest_cycle.as_ref(),
        latest_heartbeat.as_ref(),
    );
    let state = derive_daemon_state(
        &daemon_id,
        &now,
        source,
        binding,
        &refs,
        latest_cycle.as_ref(),
        latest_heartbeat.as_ref(),
        recovery_action.as_ref(),
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
    latest_cycle: Option<&SupervisorCycleRecord>,
    latest_heartbeat: Option<&SupervisorHeartbeatRecord>,
    recovery_action: Option<&DaemonRecoveryActionRecord>,
) -> DaemonStateRecord {
    let health_state = if latest_heartbeat.is_some_and(|value| value.stale_lease) {
        Some("stale".into())
    } else if latest_heartbeat.is_some() {
        Some("healthy".into())
    } else {
        Some("unknown".into())
    };
    let recovery_needed = recovery_action
        .as_ref()
        .is_some_and(|value| value.apply_immediately || value.action_kind != "observe_only");
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
        supervision_state: latest_cycle
            .and_then(|value| value.blocked_kind.clone())
            .unwrap_or_else(|| "observing".into()),
        mode: "attached".into(),
        pid: Some(std::process::id()),
        last_heartbeat_id: latest_heartbeat.map(|value| value.heartbeat_id.clone()),
        last_cycle_id: latest_cycle.map(|value| value.cycle_id.clone()),
        health_state,
        recovery_needed,
        recovery_action_kind: recovery_action.map(|value| value.action_kind.clone()),
        active_binding,
        status_summary: format!(
            "daemon source={} blocked_kind={} stale_lease={} recovery={}",
            source,
            latest_cycle
                .and_then(|value| value.blocked_kind.as_deref())
                .unwrap_or("-"),
            latest_heartbeat.is_some_and(|value| value.stale_lease),
            recovery_action
                .as_ref()
                .map(|value| value.action_kind.as_str())
                .unwrap_or("none")
        ),
    }
}

fn derive_recovery_action(
    source: &str,
    now: &str,
    refs: &EntityRefs,
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

fn resolve_paths(
    runtime_home: &Path,
    binding: &DebugBinding,
) -> Result<Option<DaemonPaths>, CliError> {
    if let Some(relative) = &binding.session_messages_path {
        if let Some(prefix) = relative.strip_suffix("conversation/messages.json") {
            return Ok(Some(DaemonPaths {
                session_dir: runtime_home.join(prefix.trim_end_matches('/')),
            }));
        }
    }
    let Some(session_id) = binding.session_id.as_deref() else {
        return Ok(None);
    };
    Ok(find_session_dir(runtime_home, session_id).map(|session_dir| DaemonPaths { session_dir }))
}

fn ensure_dirs(paths: &DaemonPaths) -> Result<(), CliError> {
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

fn session_recent_states_relative(
    paths: &DaemonPaths,
    runtime_home: &Path,
) -> Result<String, CliError> {
    paths
        .recent_daemon_states_path()
        .strip_prefix(runtime_home)
        .map(|path| path.to_string_lossy().trim_start_matches('/').to_string())
        .map_err(|_| CliError::Usage)
}

fn session_recent_recovery_actions_relative(
    paths: &DaemonPaths,
    runtime_home: &Path,
) -> Result<String, CliError> {
    paths
        .recent_recovery_actions_path()
        .strip_prefix(runtime_home)
        .map(|path| path.to_string_lossy().trim_start_matches('/').to_string())
        .map_err(|_| CliError::Usage)
}

fn trim_head<T>(items: &mut Vec<T>, limit: usize) {
    if items.len() > limit {
        let drain_count = items.len() - limit;
        items.drain(0..drain_count);
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

fn sanitize_id(input: &str) -> String {
    input
        .chars()
        .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '-' })
        .collect()
}
