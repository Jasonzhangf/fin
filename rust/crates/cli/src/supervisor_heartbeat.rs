use crate::{CliError, supervisor_cycle::run_supervisor_cycle};
use chrono::{DateTime, FixedOffset};
use fin_config::RuntimeRetentionConfig;
use fin_contracts::{
    DebugVisibility, EntityRefs, EventEnvelope, Severity, SupervisorCycleRecord,
    SupervisorHeartbeatRecord,
};
use fin_debug_server::{ChatSendResponse, DebugBinding};
use fin_runtime::append_framework_events;
use serde::{Serialize, de::DeserializeOwned};
use serde_json::{Value, json};
use std::{
    fs,
    path::{Path, PathBuf},
};

#[derive(Debug, Clone)]
struct SupervisorPaths {
    session_dir: PathBuf,
}

impl SupervisorPaths {
    fn latest_cycle_path(&self) -> PathBuf {
        self.session_dir.join("control/supervisor/latest.json")
    }

    fn latest_heartbeat_path(&self) -> PathBuf {
        self.session_dir
            .join("control/supervisor/latest_heartbeat.json")
    }

    fn recent_heartbeats_path(&self) -> PathBuf {
        self.session_dir
            .join("control/supervisor/recent_heartbeats.json")
    }
}

#[derive(Debug, Clone)]
pub(crate) struct SupervisorHeartbeatOutcome {
    #[allow(dead_code)]
    pub(crate) heartbeat: Option<SupervisorHeartbeatRecord>,
}

pub(crate) fn run_supervisor_heartbeat<F>(
    runtime_home: &Path,
    binding: &DebugBinding,
    source: &str,
    heartbeat_interval_ms: u64,
    retention: &RuntimeRetentionConfig,
    recent_limit: usize,
    mut run_next: F,
) -> Result<SupervisorHeartbeatOutcome, CliError>
where
    F: FnMut(
        DebugBinding,
        String,
        Option<&fin_contracts::InterruptedSegmentRecord>,
    ) -> Result<ChatSendResponse, CliError>,
{
    let Some(paths) = resolve_paths(runtime_home, binding)? else {
        return Ok(SupervisorHeartbeatOutcome { heartbeat: None });
    };
    ensure_dirs(&paths)?;
    let created_at = crate::time::local_timestamp_now();
    let refs = entity_refs(binding);
    let heartbeat_id = format!(
        "heartbeat-{}-{}",
        refs.session_id.as_deref().unwrap_or("tentative"),
        sanitize_id(&created_at)
    );

    let observed_cycle = read_json_if_exists::<SupervisorCycleRecord>(&paths.latest_cycle_path())?;
    let due_for_tick = observed_cycle
        .as_ref()
        .and_then(|value| value.next_check_at.as_deref())
        .is_some_and(|value| is_timestamp_due(value, &created_at));
    let stale_lease = observed_cycle
        .as_ref()
        .and_then(lease_deadline_at)
        .as_deref()
        .is_some_and(|value| is_timestamp_due(value, &created_at));

    let triggered = if due_for_tick {
        Some(run_supervisor_cycle(
            runtime_home,
            binding,
            "supervisor_heartbeat_due",
            heartbeat_interval_ms,
            retention,
            recent_limit,
            &mut run_next,
        )?)
    } else {
        None
    };

    let final_cycle = if let Some(outcome) = &triggered {
        outcome.cycle.clone().or(observed_cycle.clone())
    } else {
        observed_cycle.clone()
    };
    let blocked_kind = final_cycle
        .as_ref()
        .and_then(|value| value.blocked_kind.clone());
    let next_check_at = final_cycle
        .as_ref()
        .and_then(|value| value.next_check_at.clone());
    let lease_deadline = final_cycle.as_ref().and_then(lease_deadline_at);
    let heartbeat = SupervisorHeartbeatRecord {
        heartbeat_id: heartbeat_id.clone(),
        created_at: created_at.clone(),
        refs: refs.clone(),
        source: source.into(),
        status: if stale_lease {
            "stale_detected".into()
        } else if due_for_tick {
            "triggered_cycle".into()
        } else if observed_cycle.is_some() {
            "observed".into()
        } else {
            "no_cycle".into()
        },
        observed_cycle_id: observed_cycle.as_ref().map(|value| value.cycle_id.clone()),
        triggered_cycle_id: triggered
            .as_ref()
            .and_then(|value| value.cycle.as_ref().map(|cycle| cycle.cycle_id.clone())),
        due_for_tick,
        stale_lease,
        blocked_kind: blocked_kind.clone(),
        next_check_at: next_check_at.clone(),
        lease_deadline_at: lease_deadline.clone(),
        result_summary: format!(
            "heartbeat source={} due_for_tick={} stale_lease={} blocked_kind={} next_check={}",
            source,
            due_for_tick,
            stale_lease,
            blocked_kind.as_deref().unwrap_or("-"),
            next_check_at.as_deref().unwrap_or("-"),
        ),
    };

    let mut events = vec![heartbeat_event(
        &heartbeat_id,
        1,
        "supervisor.heartbeat_recorded",
        &created_at,
        &refs,
        serde_json::to_value(&heartbeat).map_err(CliError::Serialize)?,
    )];
    if stale_lease {
        events.push(heartbeat_event(
            &heartbeat_id,
            2,
            "supervisor.stale_cycle_detected",
            &created_at,
            &refs,
            json!({
                "heartbeat_id": heartbeat_id,
                "observed_cycle_id": heartbeat.observed_cycle_id,
                "blocked_kind": heartbeat.blocked_kind,
                "lease_deadline_at": heartbeat.lease_deadline_at,
            }),
        ));
    }
    append_framework_events(runtime_home, &paths.session_dir, &events, retention)?;
    persist_heartbeat_record(runtime_home, &paths, &heartbeat, recent_limit)?;

    Ok(SupervisorHeartbeatOutcome {
        heartbeat: Some(heartbeat),
    })
}

fn heartbeat_event(
    heartbeat_id: &str,
    sequence: u64,
    event_type: &str,
    occurred_at: &str,
    refs: &EntityRefs,
    payload: Value,
) -> EventEnvelope<Value> {
    let mut event = EventEnvelope::new(
        format!("{heartbeat_id}-evt-{sequence:02}"),
        event_type,
        occurred_at.to_string(),
        "cli.supervisor_heartbeat",
        heartbeat_id.to_string(),
        sequence,
        payload,
    );
    event.refs = refs.clone();
    event.severity = Severity::Info;
    event.debug_visibility = DebugVisibility::Important;
    event
}

fn lease_deadline_at(cycle: &SupervisorCycleRecord) -> Option<String> {
    let completed_at = cycle.completed_at.as_deref()?;
    let lease_ms = cycle.lease_ttl_ms?;
    add_ms_to_timestamp(completed_at, lease_ms)
}

fn is_timestamp_due(target: &str, now: &str) -> bool {
    let Some(target_ts) = parse_timestamp(target) else {
        return false;
    };
    let Some(now_ts) = parse_timestamp(now) else {
        return false;
    };
    target_ts <= now_ts
}

fn parse_timestamp(input: &str) -> Option<DateTime<FixedOffset>> {
    DateTime::<FixedOffset>::parse_from_rfc3339(input).ok()
}

fn add_ms_to_timestamp(input: &str, millis: u64) -> Option<String> {
    let parsed = parse_timestamp(input)?;
    let millis_i64 = i64::try_from(millis).ok()?;
    let shifted = parsed + chrono::Duration::milliseconds(millis_i64);
    Some(shifted.format("%Y-%m-%dT%H:%M:%S%:z").to_string())
}

fn persist_heartbeat_record(
    runtime_home: &Path,
    paths: &SupervisorPaths,
    record: &SupervisorHeartbeatRecord,
    recent_limit: usize,
) -> Result<(), CliError> {
    let mut recent =
        read_json_or_empty::<SupervisorHeartbeatRecord>(&paths.recent_heartbeats_path())?;
    recent.push(record.clone());
    trim_head(&mut recent, recent_limit);
    write_json(&paths.recent_heartbeats_path(), &recent)?;
    write_json(&paths.latest_heartbeat_path(), record)?;
    write_json(
        &runtime_home.join("runtime/current/current_supervisor_heartbeat.json"),
        record,
    )?;
    update_last_run_paths(
        runtime_home,
        json!({
            "current_supervisor_heartbeat_path": "runtime/current/current_supervisor_heartbeat.json",
            "session_recent_supervisor_heartbeats_path": session_recent_heartbeats_relative(paths, runtime_home)?,
        }),
    )
}

fn resolve_paths(
    runtime_home: &Path,
    binding: &DebugBinding,
) -> Result<Option<SupervisorPaths>, CliError> {
    if let Some(relative) = &binding.session_messages_path {
        if let Some(prefix) = relative.strip_suffix("conversation/messages.json") {
            return Ok(Some(SupervisorPaths {
                session_dir: runtime_home.join(prefix.trim_end_matches('/')),
            }));
        }
    }
    let Some(session_id) = binding.session_id.as_deref() else {
        return Ok(None);
    };
    Ok(find_session_dir(runtime_home, session_id)
        .map(|session_dir| SupervisorPaths { session_dir }))
}

fn ensure_dirs(paths: &SupervisorPaths) -> Result<(), CliError> {
    fs::create_dir_all(paths.session_dir.join("control/supervisor")).map_err(|source| {
        CliError::WriteFile {
            path: paths
                .session_dir
                .join("control/supervisor")
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

fn session_recent_heartbeats_relative(
    paths: &SupervisorPaths,
    runtime_home: &Path,
) -> Result<String, CliError> {
    paths
        .recent_heartbeats_path()
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
