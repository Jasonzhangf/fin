use crate::{
    CliError,
    execution_state::{
        dequeue_next_pending_input, enqueue_framework_pending_input, load_execution_state,
        load_pending_inputs, resume_execution,
    },
    session_binding::build_binding_for_session,
};
use fin_config::SystemConfig;
use fin_contracts::{InputAttachmentSummary, InterruptedSegmentRecord};
use fin_debug_server::{ChatSendResponse, DebugBinding};
use fin_runtime::{
    AgentAssignmentSummary, persist_assignment_summary, read_assignment_queue,
    update_assignment_record,
};
use serde::{Deserialize, Serialize};
use std::{fs, path::Path};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct AssignmentRuntimeResumeReport {
    pub(crate) executed_at: String,
    pub(crate) source: String,
    pub(crate) pending_count: usize,
    pub(crate) attempted_count: usize,
    pub(crate) drove_count: usize,
    pub(crate) completed_count: usize,
    pub(crate) failed_count: usize,
    #[serde(default)]
    pub(crate) assignment_ids: Vec<String>,
    pub(crate) summary: String,
}

pub(crate) fn drive_ready_assignment_resumes<F>(
    runtime_home: &Path,
    _system: &SystemConfig,
    source: &str,
    executed_at: &str,
    mut run_next: F,
) -> Result<AssignmentRuntimeResumeReport, CliError>
where
    F: FnMut(
        DebugBinding,
        Option<String>,
        String,
        String,
        Vec<InputAttachmentSummary>,
        Option<&InterruptedSegmentRecord>,
    ) -> Result<ChatSendResponse, CliError>,
{
    let assignments = read_assignment_queue(runtime_home).map_err(CliError::Runtime)?;
    let mut report = AssignmentRuntimeResumeReport {
        executed_at: executed_at.into(),
        source: source.into(),
        pending_count: assignments
            .iter()
            .filter(|item| item.status == "pending")
            .count(),
        ..AssignmentRuntimeResumeReport::default()
    };

    for assignment in assignments
        .into_iter()
        .filter(|item| item.status == "pending" && should_resume_locally(item.peer_id.as_str()))
    {
        let (Some(session_id), Some(task_id)) = (
            assignment.session_id.as_deref(),
            assignment.task_id.as_deref(),
        ) else {
            mark_assignment_failed(
                runtime_home,
                assignment.assignment_id.as_str(),
                executed_at,
                "assignment missing session_id or task_id",
            )?;
            report.failed_count += 1;
            continue;
        };
        let worker_id = assignment
            .target_worker_id
            .clone()
            .unwrap_or_else(|| "worker-project".into());
        let agent_name = assignment.target_agent_name.clone();
        let binding = build_binding_for_session(
            runtime_home,
            &base_binding(runtime_home),
            session_id,
            Some(task_id),
        )?;
        ensure_resumable_state(runtime_home, &binding, executed_at)?;
        ensure_assignment_pending_input(
            runtime_home,
            &binding,
            assignment.assignment_id.as_str(),
            assignment.task_summary.as_str(),
            executed_at,
        )?;
        persist_assignment_summary(
            runtime_home,
            &AgentAssignmentSummary {
                assignment_id: assignment.assignment_id.clone(),
                worker_id: worker_id.clone(),
                peer_id: assignment.peer_id.clone(),
                target_agent_id: agent_name.as_ref().map(|name| format!("local.{name}")),
                target_agent_name: agent_name.clone(),
                requested_role_id: assignment.requested_role_id.clone(),
                owner_worker_id: assignment.owner_worker_id.clone(),
                task_summary: assignment.task_summary.clone(),
                status: "running".into(),
                created_at: assignment.created_at.clone(),
            },
        )
        .map_err(CliError::Runtime)?;
        let _ =
            update_assignment_record(runtime_home, assignment.assignment_id.as_str(), |record| {
                record.status = "running".into();
                record.started_at = Some(executed_at.into());
                record.updated_at = Some(executed_at.into());
            })
            .map_err(CliError::Runtime)?;

        report.attempted_count += 1;
        report.assignment_ids.push(assignment.assignment_id.clone());
        let Some(next) = dequeue_next_pending_input(runtime_home, &binding, executed_at)? else {
            continue;
        };
        let _ = run_next(
            binding.clone(),
            agent_name.clone(),
            next.message,
            next.source,
            next.attachments,
            None,
        )?;
        report.drove_count += 1;
        let state = load_execution_state(runtime_home, &binding)?;
        let pending_inputs = load_pending_inputs(runtime_home, &binding)?;
        let (status, result_summary): (String, String) = if !pending_inputs.is_empty()
            || matches!(
                state.as_ref().map(|value| value.status.as_str()),
                Some("running")
            ) {
            (
                "running".to_string(),
                "assignment turn started; more work remains".to_string(),
            )
        } else {
            (
                "completed".to_string(),
                "assignment turn executed".to_string(),
            )
        };
        if status == "completed" {
            report.completed_count += 1;
        }
        let _ =
            update_assignment_record(runtime_home, assignment.assignment_id.as_str(), |record| {
                record.status = status.clone();
                record.completed_at = (status == "completed").then(|| executed_at.into());
                record.updated_at = Some(executed_at.into());
                record.result_summary = Some(result_summary.clone());
            })
            .map_err(CliError::Runtime)?;
    }

    report.summary = format!(
        "pending={} attempted={} drove={} completed={} failed={}",
        report.pending_count,
        report.attempted_count,
        report.drove_count,
        report.completed_count,
        report.failed_count
    );
    persist_report(runtime_home, &report)?;
    Ok(report)
}

pub(crate) fn read_assignment_runtime_resume_report(
    runtime_home: &Path,
) -> Result<Option<AssignmentRuntimeResumeReport>, CliError> {
    read_json_optional(&runtime_home.join("runtime/current/current_assignment_runtime_resume.json"))
}

fn base_binding(runtime_home: &Path) -> DebugBinding {
    DebugBinding {
        project_id: "fin".into(),
        project_label: "fin".into(),
        runtime_home: runtime_home.display().to_string(),
        session_id: None,
        task_id: None,
        session_messages_path: None,
        recent_contexts_path: None,
        recent_digests_path: None,
    }
}

fn should_resume_locally(peer_id: &str) -> bool {
    peer_id.starts_with("local-") || peer_id.starts_with("peer-local-")
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

fn ensure_assignment_pending_input(
    runtime_home: &Path,
    binding: &DebugBinding,
    assignment_id: &str,
    task_summary: &str,
    now: &str,
) -> Result<(), CliError> {
    let pending = load_pending_inputs(runtime_home, binding)?;
    if pending
        .iter()
        .any(|item| item.source == "project.assignment" || item.source == "project.resume")
    {
        return Ok(());
    }
    let message = format!(
        "Framework assignment: execute task now.\nassignment_id={assignment_id}\nrequired_behavior=inspect the claimed task, perform the needed work, and use project.task.submit with a concise result summary when ready.\nowner_instruction={task_summary}"
    );
    let _ = enqueue_framework_pending_input(
        runtime_home,
        binding,
        "framework_assignment",
        "project.assignment",
        &message,
        "assignment_resume",
        now,
    )?;
    Ok(())
}

fn mark_assignment_failed(
    runtime_home: &Path,
    assignment_id: &str,
    now: &str,
    summary: &str,
) -> Result<(), CliError> {
    let _ = update_assignment_record(runtime_home, assignment_id, |record| {
        record.status = "failed".into();
        record.updated_at = Some(now.into());
        record.result_summary = Some(summary.into());
    })
    .map_err(CliError::Runtime)?;
    Ok(())
}

fn persist_report(
    runtime_home: &Path,
    report: &AssignmentRuntimeResumeReport,
) -> Result<(), CliError> {
    let path = runtime_home.join("runtime/assignments/runtime_resume_reports.json");
    let mut reports =
        read_json_optional::<Vec<AssignmentRuntimeResumeReport>>(&path)?.unwrap_or_default();
    reports.push(report.clone());
    trim_head(&mut reports, 16);
    write_json(&path, &reports)?;
    write_json(
        &runtime_home.join("runtime/current/current_assignment_runtime_resume.json"),
        report,
    )
}

fn trim_head<T>(items: &mut Vec<T>, limit: usize) {
    if items.len() > limit {
        let drain_count = items.len() - limit;
        items.drain(0..drain_count);
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
