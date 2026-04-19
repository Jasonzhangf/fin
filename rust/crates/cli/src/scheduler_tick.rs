use crate::{
    CliError,
    scheduler_driver::{SchedulerDriveResult, drive_scheduler},
};
use fin_config::RuntimeRetentionConfig;
use fin_contracts::{DebugVisibility, EntityRefs, EventEnvelope, SchedulerTickRecord, Severity};
use fin_debug_server::{ChatSendResponse, DebugBinding};
use fin_runtime::append_framework_events;
use serde::{Serialize, de::DeserializeOwned};
use serde_json::{Value, json};
use std::{
    fs,
    path::{Path, PathBuf},
};

#[derive(Debug, Clone)]
struct SchedulerTickPaths {
    session_dir: PathBuf,
}

impl SchedulerTickPaths {
    fn latest_tick_path(&self) -> PathBuf {
        self.session_dir.join("control/scheduler/latest_tick.json")
    }

    fn recent_ticks_path(&self) -> PathBuf {
        self.session_dir.join("control/scheduler/recent_ticks.json")
    }
}

pub(crate) fn run_scheduler_tick<F>(
    runtime_home: &Path,
    binding: &DebugBinding,
    source: &str,
    retention: &RuntimeRetentionConfig,
    recent_limit: usize,
    mut run_next: F,
) -> Result<SchedulerTickOutcome, CliError>
where
    F: FnMut(
        DebugBinding,
        String,
        Option<&fin_contracts::InterruptedSegmentRecord>,
    ) -> Result<ChatSendResponse, CliError>,
{
    let started_at = crate::time::local_timestamp_now();
    let Some(paths) = resolve_paths(runtime_home, binding)? else {
        return Ok(SchedulerTickOutcome {
            record: None,
            drive: SchedulerDriveResult {
                last_response: None,
                decisions: Vec::new(),
                drove_count: 0,
            },
        });
    };
    ensure_dirs(&paths)?;

    let refs = entity_refs(binding);
    let tick_id = format!(
        "tick-{}-{}",
        refs.session_id.as_deref().unwrap_or("tentative"),
        sanitize_id(&started_at)
    );
    let started_event = tick_event(
        &tick_id,
        1,
        "scheduler.tick_started",
        &started_at,
        &refs,
        None,
        json!({"tick_id": tick_id, "source": source}),
    );
    let mut framework_events = vec![started_event];

    let drive = drive_scheduler(runtime_home, binding, recent_limit, &mut run_next)?;
    for (index, decision) in drive.decisions.iter().enumerate() {
        let event = tick_event(
            &tick_id,
            (index as u64) + 2,
            "scheduler.tick_decision_recorded",
            &decision.created_at,
            &refs,
            None,
            serde_json::to_value(decision).map_err(CliError::Serialize)?,
        );
        framework_events.push(event);
    }

    let completed_at = crate::time::local_timestamp_now();
    let record = build_tick_record(&tick_id, source, &started_at, &completed_at, &refs, &drive);

    if drive.drove_count > 0 {
        let event = tick_event(
            &tick_id,
            (drive.decisions.len() as u64) + 2,
            "scheduler.tick_drove_pending",
            &completed_at,
            &refs,
            None,
            json!({
                "tick_id": tick_id,
                "drove_count": drive.drove_count,
                "final_decision_id": record.final_decision_id,
                "final_action_kind": record.final_action_kind,
            }),
        );
        framework_events.push(event);
    } else {
        let event = tick_event(
            &tick_id,
            (drive.decisions.len() as u64) + 2,
            "scheduler.tick_blocked",
            &completed_at,
            &refs,
            None,
            json!({
                "tick_id": tick_id,
                "blocked_by": record.blocked_by,
                "final_action_kind": record.final_action_kind,
                "result_summary": record.result_summary,
            }),
        );
        framework_events.push(event);
    }

    let completed_event = tick_event(
        &tick_id,
        (drive.decisions.len() as u64) + 3,
        "scheduler.tick_completed",
        &completed_at,
        &refs,
        None,
        serde_json::to_value(&record).map_err(CliError::Serialize)?,
    );
    framework_events.push(completed_event);

    append_framework_events(
        runtime_home,
        &paths.session_dir,
        &framework_events,
        retention,
    )?;

    persist_tick_record(runtime_home, &paths, &record, recent_limit)?;

    Ok(SchedulerTickOutcome {
        record: Some(record),
        drive,
    })
}

#[derive(Debug, Clone)]
pub(crate) struct SchedulerTickOutcome {
    #[allow(dead_code)]
    pub(crate) record: Option<SchedulerTickRecord>,
    pub(crate) drive: SchedulerDriveResult,
}

fn build_tick_record(
    tick_id: &str,
    source: &str,
    started_at: &str,
    completed_at: &str,
    refs: &EntityRefs,
    drive: &SchedulerDriveResult,
) -> SchedulerTickRecord {
    let first_pending = drive
        .decisions
        .first()
        .map(|value| value.pending_input_count)
        .unwrap_or(0);
    let final_pending = drive
        .decisions
        .last()
        .map(|value| value.pending_input_count)
        .unwrap_or(first_pending);
    let final_decision = drive.decisions.last();
    let final_action_kind = final_decision.map(|value| value.action_kind.clone());
    let blocked_by = final_decision
        .and_then(|value| value.blocked_by.clone())
        .or_else(|| {
            if drive.drove_count == 0 {
                Some("no_work_or_blocked".into())
            } else {
                None
            }
        });
    let status = if drive.drove_count > 0 {
        "completed"
    } else if final_action_kind.as_deref() == Some("stay_idle") {
        "idle_observed"
    } else {
        "blocked"
    };
    let result_summary = if let Some(decision) = final_decision {
        format!(
            "tick source={} action={} drove_count={} reason={}",
            source, decision.action_kind, drive.drove_count, decision.reason
        )
    } else {
        format!("tick source={} skipped: no scheduler decision", source)
    };

    SchedulerTickRecord {
        tick_id: tick_id.into(),
        created_at: started_at.into(),
        completed_at: Some(completed_at.into()),
        refs: refs.clone(),
        source: source.into(),
        status: status.into(),
        decisions_recorded: drive.decisions.len(),
        drove_count: drive.drove_count,
        initial_pending_input_count: first_pending,
        final_pending_input_count: final_pending,
        final_decision_id: final_decision.map(|value| value.decision_id.clone()),
        final_action_kind,
        blocked_by,
        last_response_kind: drive
            .last_response
            .as_ref()
            .map(|value| value.response_kind.clone()),
        result_summary,
    }
}

fn persist_tick_record(
    runtime_home: &Path,
    paths: &SchedulerTickPaths,
    record: &SchedulerTickRecord,
    recent_limit: usize,
) -> Result<(), CliError> {
    let mut recent = read_json_or_empty::<SchedulerTickRecord>(&paths.recent_ticks_path())?;
    recent.push(record.clone());
    trim_head(&mut recent, recent_limit);
    write_json(&paths.recent_ticks_path(), &recent)?;
    write_json(&paths.latest_tick_path(), record)?;
    write_json(
        &runtime_home.join("runtime/current/current_scheduler_tick.json"),
        record,
    )?;
    update_last_run_paths(
        runtime_home,
        json!({
            "current_scheduler_tick_path": "runtime/current/current_scheduler_tick.json",
            "session_recent_scheduler_ticks_path": session_recent_ticks_relative(paths, runtime_home)?,
        }),
    )
}

fn tick_event(
    tick_id: &str,
    sequence: u64,
    event_type: &str,
    occurred_at: &str,
    refs: &EntityRefs,
    operation_id: Option<String>,
    payload: Value,
) -> EventEnvelope<Value> {
    let mut event = EventEnvelope::new(
        format!("{tick_id}-evt-{sequence:02}"),
        event_type,
        occurred_at.to_string(),
        "cli.scheduler_tick",
        tick_id.to_string(),
        sequence,
        payload,
    );
    event.refs = refs.clone();
    event.operation_id = operation_id;
    event.severity = Severity::Info;
    event.debug_visibility = if event_type == "scheduler.tick_decision_recorded" {
        DebugVisibility::Verbose
    } else {
        DebugVisibility::Important
    };
    event
}

fn resolve_paths(
    runtime_home: &Path,
    binding: &DebugBinding,
) -> Result<Option<SchedulerTickPaths>, CliError> {
    if let Some(relative) = &binding.session_messages_path {
        if let Some(prefix) = relative.strip_suffix("conversation/messages.json") {
            return Ok(Some(SchedulerTickPaths {
                session_dir: runtime_home.join(prefix.trim_end_matches('/')),
            }));
        }
    }
    let Some(session_id) = binding.session_id.as_deref() else {
        return Ok(None);
    };
    Ok(find_session_dir(runtime_home, session_id)
        .map(|session_dir| SchedulerTickPaths { session_dir }))
}

fn ensure_dirs(paths: &SchedulerTickPaths) -> Result<(), CliError> {
    fs::create_dir_all(paths.session_dir.join("control/scheduler")).map_err(|source| {
        CliError::WriteFile {
            path: paths
                .session_dir
                .join("control/scheduler")
                .display()
                .to_string(),
            source,
        }
    })?;
    fs::create_dir_all(paths.session_dir.join("events")).map_err(|source| CliError::WriteFile {
        path: paths.session_dir.join("events").display().to_string(),
        source,
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

fn session_recent_ticks_relative(
    paths: &SchedulerTickPaths,
    runtime_home: &Path,
) -> Result<String, CliError> {
    paths
        .recent_ticks_path()
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
