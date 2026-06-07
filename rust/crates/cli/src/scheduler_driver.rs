use crate::{
    CliError,
    execution_checkpoint::{consume_execution_checkpoint, load_open_execution_checkpoint},
    execution_segments::latest_open_segment,
    execution_state::{dequeue_next_pending_input, load_execution_state, load_pending_inputs},
    time::local_timestamp_now,
};
use fin_config::RuntimeRetentionConfig;
use fin_contracts::{
    EntityRefs, InputAttachmentSummary, OwnerLoopActionRecord, RoutingActionRecord,
    SchedulerDecisionRecord,
};
use fin_debug_server::{ChatSendResponse, DebugBinding};
use fin_runtime::{derive_owner_loop_action_for_runtime, derive_scheduler_decision};
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
    pub(crate) owner_loop_actions: Vec<OwnerLoopActionRecord>,
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

    fn latest_owner_loop_action_path(&self) -> PathBuf {
        self.session_dir.join("control/owner_loop/latest.json")
    }

    fn recent_owner_loop_actions_path(&self) -> PathBuf {
        self.session_dir
            .join("control/owner_loop/recent_actions.json")
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
            owner_loop_actions: Vec::new(),
            drove_count: 0,
        });
    };
    ensure_scheduler_dirs(&paths)?;
    let mut current_binding = binding.clone();
    let mut merge_segment = latest_open_segment(runtime_home, &current_binding)?;
    let mut last_response = None;
    let mut decisions = Vec::new();
    let mut owner_loop_actions = Vec::new();
    let mut drove_count = 0usize;
    let mut owner_loop_executed = false;

    for _ in 0..MAX_SCHEDULER_AUTO_STEPS {
        let owner_loop_action =
            derive_owner_loop_action_for_binding(runtime_home, &current_binding)?;
        persist_owner_loop_action(runtime_home, &paths, &owner_loop_action, recent_limit)?;
        owner_loop_actions.push(owner_loop_action.clone());
        let decision =
            scheduler_decision_for_binding(runtime_home, &current_binding, &owner_loop_action)?;
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
        if is_executable_owner_loop_action(&decision.action_kind) {
            if owner_loop_executed {
                break;
            }
            let response = run_next(
                current_binding.clone(),
                owner_loop_framework_prompt(&owner_loop_action),
                format!("framework.owner_loop.{}", decision.action_kind),
                Vec::new(),
                merge_segment.as_ref(),
            )?;
            current_binding = response.binding.clone();
            merge_segment = None;
            last_response = Some(response);
            drove_count = drove_count.saturating_add(1);
            owner_loop_executed = true;
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
        let stop_after_framework_planning = next.input_kind == "framework_planning"
            && next.source.starts_with("framework.task_kickoff.");
        let stop_after_parallel_user =
            next.source == "cli.parallel_user" || next.source == "channel.parallel_user";
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
        if stop_after_framework_planning || stop_after_parallel_user {
            break;
        }
    }

    Ok(SchedulerDriveResult {
        last_response,
        decisions,
        owner_loop_actions,
        drove_count,
    })
}

fn is_executable_owner_loop_action(action_kind: &str) -> bool {
    matches!(action_kind, "review_submitted_task" | "dispatch_ready_task")
}

fn owner_loop_framework_prompt(action: &OwnerLoopActionRecord) -> String {
    match action.action_kind.as_str() {
        "review_submitted_task" => format!(
            "Framework owner-loop directive: review submitted managed tasks now.\npriority=highest\ntarget_task_ids={}\nactive_task={}\nstatus_counts={}\nrequired_behavior=inspect the submitted task(s), review the latest submission, use project.task.review when appropriate, and report the outcome with concise owner-level status.",
            render_csv(&action.target_task_ids),
            action.active_task_id.as_deref().unwrap_or("-"),
            render_csv(&action.task_status_counts),
        ),
        "dispatch_ready_task" => format!(
            "Framework owner-loop directive: dispatch ready managed tasks now.\npriority=highest\ntarget_task_ids={}\nactive_task={}\nstatus_counts={}\nrequired_behavior=inspect the ready task(s), claim/assign as appropriate using project.task.claim or agent.assign, and report the owner-level dispatch result.",
            render_csv(&action.target_task_ids),
            action.active_task_id.as_deref().unwrap_or("-"),
            render_csv(&action.task_status_counts),
        ),
        _ => format!(
            "Framework owner-loop directive: action={}\ntarget_task_ids={}\nreason={}",
            action.action_kind,
            render_csv(&action.target_task_ids),
            action.reason
        ),
    }
}

fn render_csv(items: &[String]) -> String {
    if items.is_empty() {
        "-".into()
    } else {
        items.join(", ")
    }
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

pub(crate) fn load_latest_owner_loop_action(
    runtime_home: &Path,
    binding: &DebugBinding,
) -> Result<Option<OwnerLoopActionRecord>, CliError> {
    let Some(paths) = resolve_paths(runtime_home, binding)? else {
        return Ok(None);
    };
    read_json_if_exists(&paths.latest_owner_loop_action_path())
}

fn scheduler_decision_for_binding(
    runtime_home: &Path,
    binding: &DebugBinding,
    owner_loop_action: &OwnerLoopActionRecord,
) -> Result<SchedulerDecisionRecord, CliError> {
    let state = load_execution_state(runtime_home, binding)?;
    let pending = load_pending_inputs(runtime_home, binding)?;
    let routing_action = load_latest_routing_action(runtime_home, binding)?;
    Ok(derive_scheduler_decision(
        &entity_refs(binding),
        state.as_ref(),
        pending.as_slice(),
        routing_action.as_ref(),
        Some(owner_loop_action),
        &local_timestamp_now(),
    ))
}

fn derive_owner_loop_action_for_binding(
    runtime_home: &Path,
    binding: &DebugBinding,
) -> Result<OwnerLoopActionRecord, CliError> {
    derive_owner_loop_action_for_runtime(
        runtime_home,
        &entity_refs(binding),
        binding.session_id.as_deref(),
        binding.task_id.as_deref(),
        &local_timestamp_now(),
    )
    .map_err(CliError::Runtime)
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

fn persist_owner_loop_action(
    runtime_home: &Path,
    paths: &SchedulerPaths,
    action: &OwnerLoopActionRecord,
    recent_limit: usize,
) -> Result<(), CliError> {
    let mut recent =
        read_json_or_empty::<OwnerLoopActionRecord>(&paths.recent_owner_loop_actions_path())?;
    recent.push(action.clone());
    trim_head(&mut recent, recent_limit);
    write_json(&paths.recent_owner_loop_actions_path(), &recent)?;
    write_json(&paths.latest_owner_loop_action_path(), action)?;
    write_json(
        &runtime_home.join("runtime/current/current_owner_loop_action.json"),
        action,
    )?;
    update_last_run_paths(
        runtime_home,
        json!({
            "current_owner_loop_action_path": "runtime/current/current_owner_loop_action.json",
            "session_recent_owner_loop_actions_path": session_recent_owner_loop_relative(paths, runtime_home)?
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

fn session_recent_owner_loop_relative(
    paths: &SchedulerPaths,
    runtime_home: &Path,
) -> Result<String, CliError> {
    paths
        .recent_owner_loop_actions_path()
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
    })?;
    fs::create_dir_all(paths.session_dir.join("control/owner_loop")).map_err(|source| {
        CliError::WriteFile {
            path: paths
                .session_dir
                .join("control/owner_loop")
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
