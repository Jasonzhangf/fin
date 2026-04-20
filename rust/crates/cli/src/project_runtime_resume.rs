use crate::{
    CliError,
    execution_state::{
        enqueue_framework_pending_input, load_execution_state, load_pending_inputs,
        resume_execution,
    },
    project_runtime_pickup::materialize_project_runtime_pickups,
    session_binding::build_binding_for_session,
    supervisor_cycle::run_supervisor_cycle,
};
use fin_config::SystemConfig;
use fin_contracts::{InputAttachmentSummary, InterruptedSegmentRecord};
use fin_debug_server::{ChatSendResponse, DebugBinding};
use serde::{Deserialize, Serialize};
use std::{fs, path::Path};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct ProjectRuntimeResumeReport {
    pub(crate) executed_at: String,
    pub(crate) source: String,
    pub(crate) attempted_count: usize,
    pub(crate) ticked_count: usize,
    pub(crate) drove_count: usize,
    pub(crate) summary: String,
    #[serde(default)]
    pub(crate) project_ids: Vec<String>,
}

pub(crate) fn drive_ready_project_runtime_resumes<F>(
    runtime_home: &Path,
    system: &SystemConfig,
    source: &str,
    executed_at: &str,
    mut run_next: F,
) -> Result<ProjectRuntimeResumeReport, CliError>
where
    F: FnMut(
        DebugBinding,
        String,
        String,
        Vec<InputAttachmentSummary>,
        Option<&InterruptedSegmentRecord>,
    ) -> Result<ChatSendResponse, CliError>,
{
    let snapshot = materialize_project_runtime_pickups(runtime_home, system, executed_at)?;
    seed_claimed_idle_project_resumes(runtime_home, system, &snapshot, executed_at)?;
    let snapshot = materialize_project_runtime_pickups(runtime_home, system, executed_at)?;
    let mut report = ProjectRuntimeResumeReport {
        executed_at: executed_at.into(),
        source: source.into(),
        ..ProjectRuntimeResumeReport::default()
    };

    for pickup in snapshot
        .projects
        .iter()
        .filter(|item| item.next_action == "scheduler_tick_needed")
    {
        let Some(session_id) = pickup.session_id.as_deref() else {
            continue;
        };
        report.attempted_count += 1;
        let base_binding = DebugBinding {
            project_id: pickup.project_id.clone(),
            project_label: pickup.project_id.clone(),
            runtime_home: runtime_home.display().to_string(),
            session_id: None,
            task_id: None,
            session_messages_path: None,
            recent_contexts_path: None,
            recent_digests_path: None,
        };
        let binding = build_binding_for_session(
            runtime_home,
            &base_binding,
            session_id,
            pickup.task_id.as_deref(),
        )?;
        ensure_resumable_state(runtime_home, &binding, executed_at)?;
        let cycle = run_supervisor_cycle(
            runtime_home,
            &binding,
            source,
            system.runtime.heartbeat_interval_ms,
            &system.runtime.retention,
            system.runtime.retention.recent_routing_decision_limit,
            &mut run_next,
        )?;
        report.ticked_count += usize::from(cycle.cycle.is_some());
        report.drove_count += cycle.tick.drive.drove_count;
        if cycle.tick.drive.drove_count > 0 {
            report.project_ids.push(pickup.project_id.clone());
        }
    }

    let refreshed = materialize_project_runtime_pickups(runtime_home, system, executed_at)?;
    report.summary = format!(
        "attempted={} ticked={} drove={} pickup={}",
        report.attempted_count,
        report.ticked_count,
        report.drove_count,
        refreshed.status_summary()
    );
    persist_report(runtime_home, &report)?;
    Ok(report)
}

pub(crate) fn read_project_runtime_resume_report(
    runtime_home: &Path,
) -> Result<Option<ProjectRuntimeResumeReport>, CliError> {
    read_json_optional(&runtime_home.join("runtime/current/current_project_runtime_resume.json"))
}

fn seed_claimed_idle_project_resumes(
    runtime_home: &Path,
    system: &SystemConfig,
    snapshot: &crate::project_runtime_pickup::ProjectRuntimePickupSnapshot,
    now: &str,
) -> Result<(), CliError> {
    for pickup in snapshot.projects.iter().filter(|item| {
        item.pickup_state == "claimed_idle" && item.next_action == "await_manual_work"
    }) {
        let Some(session_id) = pickup.session_id.as_deref() else {
            continue;
        };
        let Some(project) = system
            .runtime
            .startup
            .project_agents
            .iter()
            .find(|item| item.project_id == pickup.project_id)
        else {
            continue;
        };
        if !project.auto_resume {
            continue;
        }
        let base_binding = DebugBinding {
            project_id: pickup.project_id.clone(),
            project_label: pickup.project_id.clone(),
            runtime_home: runtime_home.display().to_string(),
            session_id: None,
            task_id: None,
            session_messages_path: None,
            recent_contexts_path: None,
            recent_digests_path: None,
        };
        let binding = build_binding_for_session(
            runtime_home,
            &base_binding,
            session_id,
            pickup.task_id.as_deref(),
        )?;
        if !load_pending_inputs(runtime_home, &binding)?.is_empty() {
            continue;
        }
        let state = load_execution_state(runtime_home, &binding)?;
        if matches!(
            state.as_ref().map(|value| value.status.as_str()),
            Some("running" | "waiting_external")
        ) {
            continue;
        }
        let _ = enqueue_framework_pending_input(
            runtime_home,
            &binding,
            "framework_resume",
            "project.resume",
            "continue work",
            "resume",
            now,
        )?;
    }
    Ok(())
}

fn ensure_resumable_state(
    runtime_home: &Path,
    binding: &DebugBinding,
    now: &str,
) -> Result<(), CliError> {
    if load_execution_state(runtime_home, binding)?.is_none() {
        let _ = resume_execution(runtime_home, binding, now)?;
    }
    Ok(())
}

fn persist_report(
    runtime_home: &Path,
    report: &ProjectRuntimeResumeReport,
) -> Result<(), CliError> {
    let path = runtime_home.join("runtime/projects/runtime_resume_reports.json");
    let mut reports = read_json_or_empty::<ProjectRuntimeResumeReport>(&path)?;
    reports.push(report.clone());
    trim_head(&mut reports, 16);
    write_json(&path, &reports)?;
    write_json(
        &runtime_home.join("runtime/current/current_project_runtime_resume.json"),
        report,
    )
}

fn trim_head<T>(items: &mut Vec<T>, limit: usize) {
    if items.len() > limit {
        let drain_count = items.len() - limit;
        items.drain(0..drain_count);
    }
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

fn read_json_optional<T: for<'de> Deserialize<'de>>(path: &Path) -> Result<Option<T>, CliError> {
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
