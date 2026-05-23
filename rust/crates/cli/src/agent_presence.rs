use crate::CliError;
use fin_config::{ProjectAgentStartupConfig, SystemConfig};
use fin_contracts::ExecutionStateRecord;
use fin_runtime::{
    AgentControlStore, AgentKind, CapabilityDescriptor, RegisterPrimaryAgentInput,
    allocate_local_agent_identity, resolve_device_name,
};
use serde::{Deserialize, Serialize};
use std::path::Path;

#[path = "agent_presence_project.rs"]
mod agent_presence_project;
#[path = "agent_presence_store.rs"]
mod agent_presence_store;
use agent_presence_project::project_mode_name;
pub(crate) use agent_presence_project::{find_project_agent_config, project_agent_name};
use agent_presence_store::{presence_path, read_presence, write_presence};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct AgentPresenceRecord {
    pub(crate) agent_id: String,
    pub(crate) agent_name: String,
    pub(crate) device_name: String,
    #[serde(default)]
    pub(crate) worker_id: Option<String>,
    pub(crate) role_id: String,
    pub(crate) agent_kind: String,
    #[serde(default)]
    pub(crate) project_id: Option<String>,
    pub(crate) status: String,
    #[serde(default)]
    pub(crate) current_task_id: Option<String>,
    #[serde(default)]
    pub(crate) current_operation_id: Option<String>,
    #[serde(default)]
    pub(crate) current_session_id: Option<String>,
    #[serde(default)]
    pub(crate) current_phase: Option<String>,
    pub(crate) is_reasoning: bool,
    pub(crate) updated_at: String,
    #[serde(default)]
    pub(crate) last_heartbeat_at: Option<String>,
    pub(crate) progress_summary: String,
    #[serde(default)]
    pub(crate) pending_input_count: usize,
    #[serde(default)]
    pub(crate) waiting_reason: Option<String>,
    #[serde(default)]
    pub(crate) mode: Option<String>,
    #[serde(default)]
    pub(crate) project_root: Option<String>,
    #[serde(default)]
    pub(crate) endpoint: Option<String>,
    #[serde(default)]
    pub(crate) always_on: Option<bool>,
    #[serde(default)]
    pub(crate) auto_resume: Option<bool>,
    #[serde(default)]
    pub(crate) worker_budget: Option<usize>,
    pub(crate) source: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
struct AgentPresenceRegistry {
    #[serde(default)]
    agents: Vec<AgentPresenceRecord>,
}

pub(crate) fn ensure_entry_agent_presence(
    system: &SystemConfig,
    runtime_home: &Path,
    updated_at: &str,
) -> Result<AgentPresenceRecord, CliError> {
    let identity = allocate_local_agent_identity(
        system,
        runtime_home,
        None,
        "cli",
        Some(system.policy.entry_role.as_str()),
    )?;
    let record = AgentPresenceRecord {
        agent_id: identity.agent_id,
        agent_name: identity.agent_name,
        device_name: identity.device_name,
        worker_id: Some(identity.worker_id),
        role_id: system.policy.entry_role.clone(),
        agent_kind: "system_entry".into(),
        project_id: None,
        status: "idle".into(),
        current_task_id: None,
        current_operation_id: None,
        current_session_id: None,
        current_phase: Some("frontstage_ready".into()),
        is_reasoning: false,
        updated_at: updated_at.into(),
        last_heartbeat_at: Some(updated_at.into()),
        progress_summary: "system entry ready".into(),
        pending_input_count: 0,
        waiting_reason: None,
        mode: Some("local".into()),
        project_root: None,
        endpoint: None,
        always_on: Some(true),
        auto_resume: Some(system.runtime.startup.system_agent.auto_resume),
        worker_budget: Some(system.runtime.startup.system_agent.local_worker_budget),
        source: "framework.startup".into(),
    };
    write_presence(runtime_home, &record)?;
    ensure_system_agent_identity(runtime_home, &record, updated_at)?;
    ensure_system_worker_pool(system, runtime_home, updated_at)?;
    Ok(record)
}

fn ensure_system_agent_identity(
    runtime_home: &Path,
    record: &AgentPresenceRecord,
    updated_at: &str,
) -> Result<(), CliError> {
    AgentControlStore::new(runtime_home)
        .register_primary_agent(RegisterPrimaryAgentInput {
            agent_id: record.agent_id.clone(),
            kind: AgentKind::SystemAgent,
            project_id: None,
            device_binding: record.device_name.clone(),
            auth_subject: format!("local-system-agent:{}", record.agent_id),
            auth_lease_id: format!("local-system-agent-{}", updated_at),
            capability_descriptor: CapabilityDescriptor {
                capability_ids: vec!["system_orchestration".into(), "mailbox".into()],
                tool_allowlist: vec!["mailbox.send".into(), "agent.assign".into()],
            },
            now: updated_at.into(),
        })
        .map(|_| ())
        .map_err(CliError::InvalidInstallState)
}

pub(crate) fn mark_entry_agent_busy(
    system: &SystemConfig,
    runtime_home: &Path,
    session_id: &str,
    task_id: Option<&str>,
    operation_id: &str,
    updated_at: &str,
    progress_summary: &str,
) -> Result<AgentPresenceRecord, CliError> {
    let mut record = ensure_entry_agent_presence(system, runtime_home, updated_at)?;
    record.status = "busy".into();
    record.current_session_id = Some(session_id.into());
    record.current_task_id = task_id.map(str::to_string);
    record.current_operation_id = Some(operation_id.into());
    record.current_phase = Some("reasoning".into());
    record.is_reasoning = true;
    record.updated_at = updated_at.into();
    record.last_heartbeat_at = Some(updated_at.into());
    record.progress_summary = progress_summary.into();
    record.source = "framework.inference".into();
    write_presence(runtime_home, &record)?;
    Ok(record)
}

pub(crate) fn mark_entry_agent_idle(
    system: &SystemConfig,
    runtime_home: &Path,
    session_id: Option<&str>,
    task_id: Option<&str>,
    operation_id: Option<&str>,
    updated_at: &str,
    progress_summary: &str,
) -> Result<AgentPresenceRecord, CliError> {
    let mut record = ensure_entry_agent_presence(system, runtime_home, updated_at)?;
    record.status = "idle".into();
    record.current_session_id = session_id.map(str::to_string);
    record.current_task_id = task_id.map(str::to_string);
    record.current_operation_id = operation_id.map(str::to_string);
    record.current_phase = Some("frontstage_ready".into());
    record.is_reasoning = false;
    record.updated_at = updated_at.into();
    record.last_heartbeat_at = Some(updated_at.into());
    record.progress_summary = progress_summary.into();
    record.waiting_reason = None;
    record.source = "framework.inference".into();
    write_presence(runtime_home, &record)?;
    Ok(record)
}

pub(crate) fn mark_entry_agent_failed(
    system: &SystemConfig,
    runtime_home: &Path,
    session_id: Option<&str>,
    task_id: Option<&str>,
    operation_id: Option<&str>,
    updated_at: &str,
    reason: &str,
) -> Result<AgentPresenceRecord, CliError> {
    let mut record = ensure_entry_agent_presence(system, runtime_home, updated_at)?;
    record.status = "idle".into();
    record.current_session_id = session_id.map(str::to_string);
    record.current_task_id = task_id.map(str::to_string);
    record.current_operation_id = operation_id.map(str::to_string);
    record.current_phase = Some("error".into());
    record.is_reasoning = false;
    record.updated_at = updated_at.into();
    record.last_heartbeat_at = Some(updated_at.into());
    record.progress_summary = format!("last run failed: {reason}");
    record.waiting_reason = Some(reason.into());
    record.source = "framework.inference".into();
    write_presence(runtime_home, &record)?;
    Ok(record)
}

pub(crate) fn ensure_project_agent_presence(
    system: &SystemConfig,
    runtime_home: &Path,
    project: &ProjectAgentStartupConfig,
    updated_at: &str,
) -> Result<AgentPresenceRecord, CliError> {
    let agent_name = project_agent_name(project);
    let device_name = resolve_device_name(system);
    let agent_id = format!("{device_name}.{agent_name}");
    let path = presence_path(runtime_home, &agent_id);
    if let Some(existing) = read_presence(&path)? {
        return Ok(existing);
    }
    let record = AgentPresenceRecord {
        agent_id,
        agent_name,
        device_name,
        worker_id: Some(format!("worker-{}", project_agent_name(project))),
        role_id: "project".into(),
        agent_kind: "project_agent".into(),
        project_id: Some(project.project_id.clone()),
        status: "offline".into(),
        current_task_id: None,
        current_operation_id: None,
        current_session_id: None,
        current_phase: Some("await_startup_wake".into()),
        is_reasoning: false,
        updated_at: updated_at.into(),
        last_heartbeat_at: None,
        progress_summary: "configured project agent; awaiting framework wake".into(),
        pending_input_count: 0,
        waiting_reason: None,
        mode: Some(project_mode_name(project)),
        project_root: project.project_root.clone(),
        endpoint: project.endpoint.clone(),
        always_on: Some(project.always_on),
        auto_resume: Some(project.auto_resume),
        worker_budget: Some(project.worker_budget),
        source: "framework.startup".into(),
    };
    write_presence(runtime_home, &record)?;
    ensure_project_worker_pool(system, runtime_home, project, updated_at)?;
    Ok(record)
}

pub(crate) fn project_agent_id(
    system: &SystemConfig,
    project: &ProjectAgentStartupConfig,
) -> String {
    format!(
        "{}.{}",
        resolve_device_name(system),
        project_agent_name(project)
    )
}

pub(crate) fn mark_project_agent_woken_local(
    system: &SystemConfig,
    runtime_home: &Path,
    project: &ProjectAgentStartupConfig,
    resume_task_id: Option<&str>,
    updated_at: &str,
    progress_summary: &str,
) -> Result<AgentPresenceRecord, CliError> {
    let mut record = ensure_project_agent_presence(system, runtime_home, project, updated_at)?;
    record.status = "idle".into();
    record.current_task_id = resume_task_id.map(str::to_string);
    record.current_phase = Some(if resume_task_id.is_some() {
        "resume_ready".into()
    } else {
        "project_ready".into()
    });
    record.is_reasoning = false;
    record.updated_at = updated_at.into();
    record.last_heartbeat_at = Some(updated_at.into());
    record.progress_summary = progress_summary.into();
    record.waiting_reason = None;
    record.source = "framework.startup_wakeup".into();
    write_presence(runtime_home, &record)?;
    Ok(record)
}

pub(crate) fn ensure_system_worker_pool(
    system: &SystemConfig,
    runtime_home: &Path,
    updated_at: &str,
) -> Result<Vec<AgentPresenceRecord>, CliError> {
    let mut records = Vec::new();
    for slot in 1..=system.runtime.startup.system_agent.local_worker_budget {
        let requested_agent_name = format!("system-worker-{slot:02}");
        let identity = allocate_local_agent_identity(
            system,
            runtime_home,
            Some(requested_agent_name.as_str()),
            format!("framework.worker_pool.system.{slot:02}").as_str(),
            Some("project"),
        )?;
        let record = AgentPresenceRecord {
            agent_id: identity.agent_id,
            agent_name: identity.agent_name,
            device_name: identity.device_name,
            worker_id: Some(identity.worker_id),
            role_id: "project".into(),
            agent_kind: "system_worker".into(),
            project_id: None,
            status: "idle".into(),
            current_task_id: None,
            current_operation_id: None,
            current_session_id: None,
            current_phase: Some("worker_ready".into()),
            is_reasoning: false,
            updated_at: updated_at.into(),
            last_heartbeat_at: Some(updated_at.into()),
            progress_summary: "system worker pool ready".into(),
            pending_input_count: 0,
            waiting_reason: None,
            mode: Some("local".into()),
            project_root: None,
            endpoint: None,
            always_on: Some(true),
            auto_resume: Some(system.runtime.startup.system_agent.auto_resume),
            worker_budget: None,
            source: "framework.worker_pool".into(),
        };
        write_presence(runtime_home, &record)?;
        records.push(record);
    }
    Ok(records)
}

pub(crate) fn ensure_project_worker_pool(
    system: &SystemConfig,
    runtime_home: &Path,
    project: &ProjectAgentStartupConfig,
    updated_at: &str,
) -> Result<Vec<AgentPresenceRecord>, CliError> {
    let mut records = Vec::new();
    let base = project_agent_name(project);
    for slot in 1..=project.worker_budget {
        let requested_agent_name = format!("{base}-worker-{slot:02}");
        let identity = allocate_local_agent_identity(
            system,
            runtime_home,
            Some(requested_agent_name.as_str()),
            format!(
                "framework.worker_pool.project.{}.{}",
                project.project_id, slot
            )
            .as_str(),
            Some("project"),
        )?;
        let record = AgentPresenceRecord {
            agent_id: identity.agent_id,
            agent_name: identity.agent_name,
            device_name: identity.device_name,
            worker_id: Some(identity.worker_id),
            role_id: "project".into(),
            agent_kind: "project_worker".into(),
            project_id: Some(project.project_id.clone()),
            status: "idle".into(),
            current_task_id: None,
            current_operation_id: None,
            current_session_id: None,
            current_phase: Some("worker_ready".into()),
            is_reasoning: false,
            updated_at: updated_at.into(),
            last_heartbeat_at: Some(updated_at.into()),
            progress_summary: format!("project worker pool ready for {}", project.project_id),
            pending_input_count: 0,
            waiting_reason: None,
            mode: Some(project_mode_name(project)),
            project_root: project.project_root.clone(),
            endpoint: project.endpoint.clone(),
            always_on: Some(project.always_on),
            auto_resume: Some(project.auto_resume),
            worker_budget: None,
            source: "framework.worker_pool".into(),
        };
        write_presence(runtime_home, &record)?;
        records.push(record);
    }
    Ok(records)
}

pub(crate) fn mark_project_agent_waiting_remote(
    system: &SystemConfig,
    runtime_home: &Path,
    project: &ProjectAgentStartupConfig,
    resume_task_id: Option<&str>,
    updated_at: &str,
    progress_summary: &str,
) -> Result<AgentPresenceRecord, CliError> {
    let mut record = ensure_project_agent_presence(system, runtime_home, project, updated_at)?;
    record.status = "waiting".into();
    record.current_task_id = resume_task_id.map(str::to_string);
    record.current_phase = Some("await_remote_connect".into());
    record.is_reasoning = false;
    record.updated_at = updated_at.into();
    record.last_heartbeat_at = Some(updated_at.into());
    record.progress_summary = progress_summary.into();
    record.waiting_reason = Some("remote_connect_pending".into());
    record.source = "framework.startup_wakeup".into();
    write_presence(runtime_home, &record)?;
    Ok(record)
}

pub(crate) fn mark_project_agent_runtime_presence(
    system: &SystemConfig,
    runtime_home: &Path,
    project: &ProjectAgentStartupConfig,
    session_id: Option<&str>,
    task_id: Option<&str>,
    state: Option<&ExecutionStateRecord>,
    updated_at: &str,
    progress_summary: &str,
) -> Result<AgentPresenceRecord, CliError> {
    let mut record = ensure_project_agent_presence(system, runtime_home, project, updated_at)?;
    record.current_session_id = session_id.map(str::to_string);
    record.current_task_id = task_id.map(str::to_string);
    record.updated_at = updated_at.into();
    record.last_heartbeat_at = Some(updated_at.into());
    record.progress_summary = progress_summary.into();
    record.source = "framework.project_runtime".into();
    record.pending_input_count = state.map(|value| value.pending_input_count).unwrap_or(0);

    match state.map(|value| value.status.as_str()) {
        Some("running") => {
            record.status = "busy".into();
            record.current_phase = Some("reasoning".into());
            record.is_reasoning = true;
            record.waiting_reason = None;
        }
        Some("waiting_external") => {
            record.status = "waiting".into();
            record.current_phase = Some("waiting_external".into());
            record.is_reasoning = false;
            record.waiting_reason = state.and_then(|value| value.reason.clone());
        }
        Some("paused") => {
            record.status = "waiting".into();
            record.current_phase = Some("paused".into());
            record.is_reasoning = false;
            record.waiting_reason = state.and_then(|value| value.reason.clone());
        }
        _ => {
            record.status = "idle".into();
            record.current_phase = Some("project_ready".into());
            record.is_reasoning = false;
            record.waiting_reason = None;
        }
    }

    write_presence(runtime_home, &record)?;
    Ok(record)
}

pub(crate) fn mark_project_agent_runtime_error(
    system: &SystemConfig,
    runtime_home: &Path,
    project: &ProjectAgentStartupConfig,
    session_id: Option<&str>,
    task_id: Option<&str>,
    operation_id: &str,
    updated_at: &str,
    progress_summary: &str,
) -> Result<AgentPresenceRecord, CliError> {
    let mut record = ensure_project_agent_presence(system, runtime_home, project, updated_at)?;
    record.status = "idle".into();
    record.current_session_id = session_id.map(str::to_string);
    record.current_task_id = task_id.map(str::to_string);
    record.current_operation_id = Some(operation_id.into());
    record.current_phase = Some("error".into());
    record.updated_at = updated_at.into();
    record.last_heartbeat_at = Some(updated_at.into());
    record.progress_summary = progress_summary.into();
    record.waiting_reason = Some("project_turn_failed".into());
    record.source = "framework.project_runtime".into();
    write_presence(runtime_home, &record)?;
    Ok(record)
}
