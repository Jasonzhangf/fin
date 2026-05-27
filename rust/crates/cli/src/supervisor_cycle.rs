use crate::{
    CliError,
    scheduler_tick::{SchedulerTickOutcome, run_scheduler_tick},
};
use chrono::{DateTime, Duration, FixedOffset};
use fin_config::RuntimeRetentionConfig;
use fin_contracts::{
    DebugVisibility, EntityRefs, EventEnvelope, InputAttachmentSummary, Severity,
    SupervisorCycleRecord,
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

    fn recent_cycles_path(&self) -> PathBuf {
        self.session_dir
            .join("control/supervisor/recent_cycles.json")
    }
}

#[derive(Debug, Clone)]
pub(crate) struct SupervisorCycleOutcome {
    #[allow(dead_code)]
    pub(crate) cycle: Option<SupervisorCycleRecord>,
    pub(crate) tick: SchedulerTickOutcome,
}

pub(crate) fn run_supervisor_cycle<F>(
    runtime_home: &Path,
    binding: &DebugBinding,
    source: &str,
    heartbeat_interval_ms: u64,
    retention: &RuntimeRetentionConfig,
    recent_limit: usize,
    mut run_next: F,
) -> Result<SupervisorCycleOutcome, CliError>
where
    F: FnMut(
        DebugBinding,
        String,
        String,
        Vec<InputAttachmentSummary>,
        Option<&fin_contracts::InterruptedSegmentRecord>,
    ) -> Result<ChatSendResponse, CliError>,
{
    let started_at = crate::time::local_timestamp_now();
    let Some(paths) = resolve_paths(runtime_home, binding)? else {
        return Ok(SupervisorCycleOutcome {
            cycle: None,
            tick: SchedulerTickOutcome {
                record: None,
                drive: crate::scheduler_driver::SchedulerDriveResult {
                    last_response: None,
                    decisions: Vec::new(),
                    owner_loop_actions: Vec::new(),
                    drove_count: 0,
                },
            },
        });
    };
    ensure_dirs(&paths)?;

    let refs = entity_refs(binding);
    let cycle_id = format!(
        "supervisor-{}-{}",
        refs.session_id.as_deref().unwrap_or("tentative"),
        sanitize_id(&started_at)
    );

    let tick = run_scheduler_tick(
        runtime_home,
        binding,
        source,
        retention,
        recent_limit,
        &mut run_next,
    )?;

    let completed_at = crate::time::local_timestamp_now();
    let cycle = build_cycle_record(
        &cycle_id,
        source,
        &started_at,
        &completed_at,
        &refs,
        heartbeat_interval_ms,
        &tick,
    );

    let events = vec![
        cycle_event(
            &cycle_id,
            1,
            "supervisor.cycle_started",
            &started_at,
            &refs,
            json!({"cycle_id": cycle_id, "source": source}),
        ),
        cycle_event(
            &cycle_id,
            2,
            "supervisor.cycle_completed",
            &completed_at,
            &refs,
            serde_json::to_value(&cycle).map_err(CliError::Serialize)?,
        ),
    ];
    append_framework_events(runtime_home, &paths.session_dir, &events, retention)?;
    persist_cycle_record(runtime_home, &paths, &cycle, recent_limit)?;

    Ok(SupervisorCycleOutcome {
        cycle: Some(cycle),
        tick,
    })
}

fn build_cycle_record(
    cycle_id: &str,
    source: &str,
    started_at: &str,
    completed_at: &str,
    refs: &EntityRefs,
    heartbeat_interval_ms: u64,
    tick: &SchedulerTickOutcome,
) -> SupervisorCycleRecord {
    let tick_record = tick.record.as_ref();
    let pending_before = tick_record
        .map(|value| value.initial_pending_input_count)
        .unwrap_or(0);
    let pending_after = tick_record
        .map(|value| value.final_pending_input_count)
        .unwrap_or(pending_before);
    let blocked_by = tick_record
        .and_then(|value| value.blocked_by.clone())
        .or_else(|| {
            tick.drive
                .decisions
                .last()
                .and_then(|value| value.blocked_by.clone())
        });
    let final_action_kind = tick_record
        .and_then(|value| value.final_action_kind.clone())
        .or_else(|| {
            tick.drive
                .decisions
                .last()
                .map(|value| value.action_kind.clone())
        });
    let blocked_kind = derive_blocked_kind(
        final_action_kind.as_deref(),
        blocked_by.as_deref(),
        pending_after,
    );
    let next_wake_hint = next_wake_hint(blocked_kind.as_deref());
    let next_check_at = next_check_at(blocked_kind.as_deref(), completed_at, heartbeat_interval_ms);
    let lease_ttl_ms = Some(heartbeat_interval_ms.saturating_mul(3));
    let status = if tick.record.is_some() {
        "completed"
    } else {
        "skipped"
    };

    SupervisorCycleRecord {
        cycle_id: cycle_id.into(),
        created_at: started_at.into(),
        completed_at: Some(completed_at.into()),
        refs: refs.clone(),
        source: source.into(),
        status: status.into(),
        tick_id: tick_record.map(|value| value.tick_id.clone()),
        tick_count: usize::from(tick_record.is_some()),
        drove_count: tick.drive.drove_count,
        pending_input_count_before: pending_before,
        pending_input_count_after: pending_after,
        final_tick_status: tick_record.map(|value| value.status.clone()),
        final_action_kind,
        blocked_by,
        blocked_kind: blocked_kind.clone(),
        next_wake_hint,
        next_check_at,
        heartbeat_interval_ms: Some(heartbeat_interval_ms),
        lease_ttl_ms,
        result_summary: if let Some(record) = tick_record {
            format!(
                "cycle source={} tick_status={} drove_count={} final_action={} blocked_kind={}",
                source,
                record.status,
                tick.drive.drove_count,
                record.final_action_kind.as_deref().unwrap_or("-"),
                blocked_kind.as_deref().unwrap_or("-"),
            )
        } else {
            format!("cycle source={} skipped: no active session binding", source)
        },
    }
}

fn derive_blocked_kind(
    final_action_kind: Option<&str>,
    blocked_by: Option<&str>,
    pending_after: usize,
) -> Option<String> {
    match (final_action_kind, blocked_by) {
        (Some("review_submitted_task"), _) | (_, Some("review_submitted_task")) => {
            Some("owner_loop_review".into())
        }
        (Some("dispatch_ready_task"), _) | (_, Some("dispatch_ready_task")) => {
            Some("owner_loop_dispatch".into())
        }
        (Some("wait_worker_feedback"), _) | (_, Some("wait_worker_feedback")) => {
            Some("owner_loop_wait_feedback".into())
        }
        (Some("await_user_confirmation"), _) | (_, Some("routing_prompt_user")) => {
            Some("await_user_confirmation".into())
        }
        (Some("wait_external"), _) | (_, Some("waiting_external")) => Some("wait_external".into()),
        (Some("wait_running"), _) | (_, Some("running")) => Some("wait_running".into()),
        (Some("wait_paused"), _) | (_, Some("paused")) => Some("wait_paused".into()),
        (Some("observe_only"), _) | (_, Some("state_unavailable")) => Some("observe_only".into()),
        (Some("run_next_pending"), _) if pending_after > 0 => Some("auto_step_limit".into()),
        (Some("stay_idle"), _) if pending_after == 0 => Some("idle_no_work".into()),
        _ => None,
    }
}

fn next_wake_hint(blocked_kind: Option<&str>) -> Option<String> {
    match blocked_kind {
        Some("await_user_confirmation") => Some("user_confirmation".into()),
        Some("wait_external") => Some("external_event_or_reminder".into()),
        Some("wait_running") => Some("supervisor_heartbeat".into()),
        Some("wait_paused") => Some("resume_or_interrupt".into()),
        Some("owner_loop_review") => Some("review_submitted_task".into()),
        Some("owner_loop_dispatch") => Some("dispatch_ready_task".into()),
        Some("owner_loop_wait_feedback") => Some("worker_progress_or_submission".into()),
        Some("idle_no_work") => Some("new_input_or_reminder".into()),
        Some("auto_step_limit") => Some("next_supervisor_heartbeat".into()),
        _ => None,
    }
}

fn next_check_at(
    blocked_kind: Option<&str>,
    completed_at: &str,
    heartbeat_interval_ms: u64,
) -> Option<String> {
    match blocked_kind {
        Some("wait_running") | Some("auto_step_limit") => {
            add_ms_to_timestamp(completed_at, heartbeat_interval_ms)
        }
        _ => None,
    }
}

fn add_ms_to_timestamp(input: &str, millis: u64) -> Option<String> {
    let parsed = DateTime::<FixedOffset>::parse_from_rfc3339(input).ok()?;
    let millis_i64 = i64::try_from(millis).ok()?;
    let shifted = parsed.checked_add_signed(Duration::milliseconds(millis_i64))?;
    Some(shifted.format("%Y-%m-%dT%H:%M:%S%:z").to_string())
}

fn cycle_event(
    cycle_id: &str,
    sequence: u64,
    event_type: &str,
    occurred_at: &str,
    refs: &EntityRefs,
    payload: Value,
) -> EventEnvelope<Value> {
    let mut event = EventEnvelope::new(
        format!("{cycle_id}-evt-{sequence:02}"),
        event_type,
        occurred_at.to_string(),
        "cli.supervisor_cycle",
        cycle_id.to_string(),
        sequence,
        payload,
    );
    event.refs = refs.clone();
    event.severity = Severity::Info;
    event.debug_visibility = DebugVisibility::Important;
    event
}

fn persist_cycle_record(
    runtime_home: &Path,
    paths: &SupervisorPaths,
    record: &SupervisorCycleRecord,
    recent_limit: usize,
) -> Result<(), CliError> {
    let mut recent = read_json_or_empty::<SupervisorCycleRecord>(&paths.recent_cycles_path())?;
    recent.push(record.clone());
    trim_head(&mut recent, recent_limit);
    write_json(&paths.recent_cycles_path(), &recent)?;
    write_json(&paths.latest_cycle_path(), record)?;
    write_json(
        &runtime_home.join("runtime/current/current_supervisor_cycle.json"),
        record,
    )?;
    update_last_run_paths(
        runtime_home,
        json!({
            "current_supervisor_cycle_path": "runtime/current/current_supervisor_cycle.json",
            "session_recent_supervisor_cycles_path": session_recent_cycles_relative(paths, runtime_home)?,
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
        let year_path = year.path();
        if !year_path.is_dir() {
            continue;
        }
        let year_name = year.file_name().to_string_lossy().to_string();
        if year_name.len() != 4 || !year_name.chars().all(|ch| ch.is_ascii_digit()) {
            continue;
        }
        let months = fs::read_dir(year_path).ok()?;
        for month in months.flatten() {
            let month_path = month.path();
            if !month_path.is_dir() {
                continue;
            }
            let month_name = month.file_name().to_string_lossy().to_string();
            if month_name.len() != 2 || !month_name.chars().all(|ch| ch.is_ascii_digit()) {
                continue;
            }
            let dir = month_path.join(session_id);
            if dir.is_dir() {
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

fn session_recent_cycles_relative(
    paths: &SupervisorPaths,
    runtime_home: &Path,
) -> Result<String, CliError> {
    paths
        .recent_cycles_path()
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
