use crate::{
    CliError,
    execution_checkpoint::{consume_execution_checkpoint, load_open_execution_checkpoint},
    execution_segments::latest_open_segment,
    execution_state::{dequeue_next_pending_input, load_execution_state, load_pending_inputs},
    time::local_timestamp_now,
};
use fin_config::RuntimeRetentionConfig;
use fin_contracts::{
    EntityRefs, InputAttachmentSummary, RoutingActionRecord, SchedulerDecisionRecord,
};
use fin_debug_server::{ChatSendResponse, DebugBinding};
use fin_runtime::derive_scheduler_decision;
use serde::{Serialize, de::DeserializeOwned};
use serde_json::{Value, json};
use std::{
    fs,
    path::{Path, PathBuf},
};

const MAX_SCHEDULER_AUTO_STEPS: usize = 8;

#[derive(Debug, Clone)]
pub(crate) struct SchedulerDriveResult {
    pub(crate) last_response: Option<ChatSendResponse>,
    pub(crate) decisions: Vec<SchedulerDecisionRecord>,
    pub(crate) drove_count: usize,
}

#[derive(Debug, Clone)]
struct SchedulerPaths {
    session_dir: PathBuf,
}

impl SchedulerPaths {
    fn latest_scheduler_path(&self) -> PathBuf {
        self.session_dir.join("control/scheduler/latest.json")
    }

    fn recent_scheduler_path(&self) -> PathBuf {
        self.session_dir
            .join("control/scheduler/recent_decisions.json")
    }

    fn latest_routing_action_path(&self) -> PathBuf {
        self.session_dir.join("tasks/routing/latest_action.json")
    }
}

pub(crate) fn drive_scheduler<F>(
    runtime_home: &Path,
    binding: &DebugBinding,
    retention: &RuntimeRetentionConfig,
    recent_limit: usize,
    mut run_next: F,
) -> Result<SchedulerDriveResult, CliError>
where
    F: FnMut(
        DebugBinding,
        String,
        String,
        Vec<InputAttachmentSummary>,
        Option<&fin_contracts::InterruptedSegmentRecord>,
    ) -> Result<ChatSendResponse, CliError>,
{
    let Some(paths) = resolve_paths(runtime_home, binding)? else {
        return Ok(SchedulerDriveResult {
            last_response: None,
            decisions: Vec::new(),
            drove_count: 0,
        });
    };
    ensure_scheduler_dirs(&paths)?;
    let mut current_binding = binding.clone();
    let mut merge_segment = latest_open_segment(runtime_home, &current_binding)?;
    let mut last_response = None;
    let mut decisions = Vec::new();
    let mut drove_count = 0usize;

    for _ in 0..MAX_SCHEDULER_AUTO_STEPS {
        let decision = scheduler_decision_for_binding(runtime_home, &current_binding)?;
        persist_scheduler_decision(runtime_home, &paths, &decision, recent_limit)?;
        decisions.push(decision.clone());
        if decision.action_kind == "resume_checkpoint" {
            let Some(checkpoint) = load_open_execution_checkpoint(runtime_home, &current_binding)?
            else {
                break;
            };
            let response = run_next(
                current_binding.clone(),
                checkpoint.resume_input.clone(),
                format!("framework.resume_checkpoint.{}", checkpoint.checkpoint_kind),
                Vec::new(),
                merge_segment.as_ref(),
            )?;
            current_binding = response.binding.clone();
            merge_segment = None;
            let _ = consume_execution_checkpoint(
                runtime_home,
                &current_binding,
                &checkpoint,
                retention,
                &local_timestamp_now(),
            )?;
            last_response = Some(response);
            drove_count = drove_count.saturating_add(1);
            continue;
        }
        if !matches!(
            decision.action_kind.as_str(),
            "run_next_pending" | "run_next_parallel"
        ) {
            break;
        }
        let Some(next) =
            dequeue_next_pending_input(runtime_home, &current_binding, &local_timestamp_now())?
        else {
            break;
        };
        let response = run_next(
            current_binding.clone(),
            next.message,
            next.source,
            next.attachments,
            merge_segment.as_ref(),
        )?;
        current_binding = response.binding.clone();
        merge_segment = None;
        last_response = Some(response);
        drove_count = drove_count.saturating_add(1);
    }

    Ok(SchedulerDriveResult {
        last_response,
        decisions,
        drove_count,
    })
}

pub(crate) fn load_latest_scheduler_decision(
    runtime_home: &Path,
    binding: &DebugBinding,
) -> Result<Option<SchedulerDecisionRecord>, CliError> {
    let Some(paths) = resolve_paths(runtime_home, binding)? else {
        return Ok(None);
    };
    read_json_if_exists(&paths.latest_scheduler_path())
}

fn scheduler_decision_for_binding(
    runtime_home: &Path,
    binding: &DebugBinding,
) -> Result<SchedulerDecisionRecord, CliError> {
    let state = load_execution_state(runtime_home, binding)?;
    let pending = load_pending_inputs(runtime_home, binding)?;
    let routing_action = load_latest_routing_action(runtime_home, binding)?;
    Ok(derive_scheduler_decision(
        &entity_refs(binding),
        state.as_ref(),
        pending.as_slice(),
        routing_action.as_ref(),
        &local_timestamp_now(),
    ))
}

fn load_latest_routing_action(
    runtime_home: &Path,
    binding: &DebugBinding,
) -> Result<Option<RoutingActionRecord>, CliError> {
    let Some(paths) = resolve_paths(runtime_home, binding)? else {
        return Ok(None);
    };
    read_json_if_exists(&paths.latest_routing_action_path())
}

fn persist_scheduler_decision(
    runtime_home: &Path,
    paths: &SchedulerPaths,
    decision: &SchedulerDecisionRecord,
    recent_limit: usize,
) -> Result<(), CliError> {
    let mut recent = read_json_or_empty::<SchedulerDecisionRecord>(&paths.recent_scheduler_path())?;
    recent.push(decision.clone());
    trim_head(&mut recent, recent_limit);
    write_json(&paths.recent_scheduler_path(), &recent)?;
    write_json(&paths.latest_scheduler_path(), decision)?;
    write_json(
        &runtime_home.join("runtime/current/current_scheduler_decision.json"),
        decision,
    )?;
    update_last_run_paths(
        runtime_home,
        json!({
            "current_scheduler_decision_path": "runtime/current/current_scheduler_decision.json",
            "session_recent_scheduler_decisions_path": session_recent_scheduler_relative(paths, runtime_home)?
        }),
    )
}

fn session_recent_scheduler_relative(
    paths: &SchedulerPaths,
    runtime_home: &Path,
) -> Result<String, CliError> {
    paths
        .recent_scheduler_path()
        .strip_prefix(runtime_home)
        .map(|path| path.to_string_lossy().trim_start_matches('/').to_string())
        .map_err(|_| CliError::Usage)
}

fn resolve_paths(
    runtime_home: &Path,
    binding: &DebugBinding,
) -> Result<Option<SchedulerPaths>, CliError> {
    if let Some(relative) = &binding.session_messages_path {
        if let Some(prefix) = relative.strip_suffix("conversation/messages.json") {
            return Ok(Some(SchedulerPaths {
                session_dir: runtime_home.join(prefix.trim_end_matches('/')),
            }));
        }
    }
    let Some(session_id) = binding.session_id.as_deref() else {
        return Ok(None);
    };
    Ok(find_session_dir(runtime_home, session_id).map(|dir| SchedulerPaths { session_dir: dir }))
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

fn ensure_scheduler_dirs(paths: &SchedulerPaths) -> Result<(), CliError> {
    fs::create_dir_all(paths.session_dir.join("control/scheduler")).map_err(|source| {
        CliError::WriteFile {
            path: paths
                .session_dir
                .join("control/scheduler")
                .display()
                .to_string(),
            source,
        }
    })
}

fn entity_refs(binding: &DebugBinding) -> EntityRefs {
    EntityRefs {
        session_id: binding.session_id.clone(),
        task_id: binding.task_id.clone(),
        ..EntityRefs::default()
    }
}

fn trim_head<T>(items: &mut Vec<T>, limit: usize) {
    if items.len() > limit {
        let drain_count = items.len() - limit;
        items.drain(0..drain_count);
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
