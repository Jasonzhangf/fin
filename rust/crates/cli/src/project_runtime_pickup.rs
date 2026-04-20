use crate::{
    CliError,
    agent_presence::mark_project_agent_runtime_presence,
    execution_state::{load_execution_state, load_pending_inputs},
    project_execution_handoff::{ProjectExecutionHandoffRecord, read_project_execution_handoffs},
    session_binding::build_binding_for_session,
};
use fin_config::{ProjectAgentMode, ProjectAgentStartupConfig, SystemConfig};
use fin_debug_server::DebugBinding;
use serde::{Deserialize, Serialize};
use std::{fs, path::Path};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct ProjectRuntimePickupRecord {
    pub(crate) project_id: String,
    pub(crate) agent_id: String,
    #[serde(default)]
    pub(crate) session_id: Option<String>,
    #[serde(default)]
    pub(crate) task_id: Option<String>,
    pub(crate) pickup_state: String,
    pub(crate) next_action: String,
    pub(crate) updated_at: String,
    pub(crate) summary: String,
    #[serde(default)]
    pub(crate) artifact_refs: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct ProjectRuntimePickupSnapshot {
    pub(crate) updated_at: String,
    pub(crate) project_count: usize,
    pub(crate) running_count: usize,
    pub(crate) waiting_count: usize,
    pub(crate) ready_count: usize,
    pub(crate) missing_binding_count: usize,
    #[serde(default)]
    pub(crate) projects: Vec<ProjectRuntimePickupRecord>,
}

impl ProjectRuntimePickupSnapshot {
    pub(crate) fn status_summary(&self) -> String {
        format!(
            "running={} waiting={} ready={} missing_binding={}",
            self.running_count, self.waiting_count, self.ready_count, self.missing_binding_count
        )
    }
}

pub(crate) fn materialize_project_runtime_pickups(
    runtime_home: &Path,
    system: &SystemConfig,
    updated_at: &str,
) -> Result<ProjectRuntimePickupSnapshot, CliError> {
    let handoffs = read_project_execution_handoffs(runtime_home)?.unwrap_or_default();
    let mut snapshot = ProjectRuntimePickupSnapshot {
        updated_at: updated_at.into(),
        project_count: handoffs.projects.len(),
        ..ProjectRuntimePickupSnapshot::default()
    };
    let base_binding = DebugBinding {
        project_id: String::new(),
        project_label: "fin".into(),
        runtime_home: runtime_home.display().to_string(),
        session_id: None,
        task_id: None,
        session_messages_path: None,
        recent_contexts_path: None,
        recent_digests_path: None,
    };

    for handoff in handoffs.projects.iter().filter(|item| should_pickup(item)) {
        let Some(project) = project_config(system, &handoff.project_id) else {
            continue;
        };
        let record = materialize_single_pickup(
            runtime_home,
            system,
            &base_binding,
            project,
            handoff,
            updated_at,
        )?;
        match record.pickup_state.as_str() {
            "running" => snapshot.running_count += 1,
            "waiting_external" | "paused" => snapshot.waiting_count += 1,
            "missing_binding" | "missing_session" => snapshot.missing_binding_count += 1,
            _ => snapshot.ready_count += 1,
        }
        snapshot.projects.push(record);
    }

    persist_snapshot(runtime_home, &snapshot)?;
    Ok(snapshot)
}

pub(crate) fn read_project_runtime_pickups(
    runtime_home: &Path,
) -> Result<Option<ProjectRuntimePickupSnapshot>, CliError> {
    read_json_optional(&runtime_home.join("runtime/current/current_project_runtime_pickups.json"))
}

fn should_pickup(handoff: &ProjectExecutionHandoffRecord) -> bool {
    matches!(handoff.handoff_state.as_str(), "prepared" | "noop")
}

fn project_config<'a>(
    system: &'a SystemConfig,
    project_id: &str,
) -> Option<&'a ProjectAgentStartupConfig> {
    system
        .runtime
        .startup
        .project_agents
        .iter()
        .find(|item| item.project_id == project_id && item.mode == ProjectAgentMode::Local)
}

fn materialize_single_pickup(
    runtime_home: &Path,
    system: &SystemConfig,
    base_binding: &DebugBinding,
    project: &ProjectAgentStartupConfig,
    handoff: &ProjectExecutionHandoffRecord,
    updated_at: &str,
) -> Result<ProjectRuntimePickupRecord, CliError> {
    let Some(session_id) = handoff.session_id.as_deref() else {
        let summary =
            "execution handoff missing session_id; cannot resolve project runtime binding";
        let _ = mark_project_agent_runtime_presence(
            system,
            runtime_home,
            project,
            None,
            handoff.task_id.as_deref(),
            None,
            updated_at,
            summary,
        )?;
        return Ok(ProjectRuntimePickupRecord {
            project_id: handoff.project_id.clone(),
            agent_id: handoff.agent_id.clone(),
            session_id: None,
            task_id: handoff.task_id.clone(),
            pickup_state: "missing_session".into(),
            next_action: "repair_handoff".into(),
            updated_at: updated_at.into(),
            summary: summary.into(),
            artifact_refs: Vec::new(),
        });
    };

    let binding = match build_binding_for_session(
        runtime_home,
        base_binding,
        session_id,
        handoff.task_id.as_deref(),
    ) {
        Ok(binding) => binding,
        Err(_) => {
            let summary = format!(
                "session binding missing for project={} session={session_id}",
                handoff.project_id
            );
            let _ = mark_project_agent_runtime_presence(
                system,
                runtime_home,
                project,
                Some(session_id),
                handoff.task_id.as_deref(),
                None,
                updated_at,
                &summary,
            )?;
            return Ok(ProjectRuntimePickupRecord {
                project_id: handoff.project_id.clone(),
                agent_id: handoff.agent_id.clone(),
                session_id: Some(session_id.into()),
                task_id: handoff.task_id.clone(),
                pickup_state: "missing_binding".into(),
                next_action: "repair_binding".into(),
                updated_at: updated_at.into(),
                summary,
                artifact_refs: Vec::new(),
            });
        }
    };

    let state = load_execution_state(runtime_home, &binding)?;
    let pending_inputs = load_pending_inputs(runtime_home, &binding)?;
    let (pickup_state, next_action, summary) =
        derive_pickup_state(handoff, state.as_ref(), pending_inputs.len());
    let _ = mark_project_agent_runtime_presence(
        system,
        runtime_home,
        project,
        Some(session_id),
        handoff.task_id.as_deref(),
        state.as_ref(),
        updated_at,
        &summary,
    )?;

    Ok(ProjectRuntimePickupRecord {
        project_id: handoff.project_id.clone(),
        agent_id: handoff.agent_id.clone(),
        session_id: Some(session_id.into()),
        task_id: handoff.task_id.clone(),
        pickup_state,
        next_action,
        updated_at: updated_at.into(),
        summary,
        artifact_refs: artifact_refs(&binding),
    })
}

fn derive_pickup_state(
    handoff: &ProjectExecutionHandoffRecord,
    state: Option<&fin_contracts::ExecutionStateRecord>,
    pending_input_count: usize,
) -> (String, String, String) {
    match state.map(|value| value.status.as_str()) {
        Some("running") => (
            "running".into(),
            "observe_running".into(),
            format!(
                "project runtime already running task {} after handoff={}",
                handoff.task_id.as_deref().unwrap_or("-"),
                handoff.handoff_state
            ),
        ),
        Some("waiting_external") => (
            "waiting_external".into(),
            "await_external_event".into(),
            format!(
                "project runtime waiting external for task {}",
                handoff.task_id.as_deref().unwrap_or("-")
            ),
        ),
        Some("paused") => (
            "paused".into(),
            "resume_or_interrupt".into(),
            format!(
                "project runtime paused on task {}; pending_inputs={pending_input_count}",
                handoff.task_id.as_deref().unwrap_or("-")
            ),
        ),
        _ if pending_input_count > 0 => (
            "ready_to_resume".into(),
            "scheduler_tick_needed".into(),
            format!(
                "project runtime ready to resume task {}; pending_inputs={pending_input_count}",
                handoff.task_id.as_deref().unwrap_or("-")
            ),
        ),
        _ => (
            "claimed_idle".into(),
            "await_manual_work".into(),
            format!(
                "task {} claimed for project runtime but no pending input is queued yet",
                handoff.task_id.as_deref().unwrap_or("-")
            ),
        ),
    }
}

fn artifact_refs(binding: &DebugBinding) -> Vec<String> {
    let mut refs = Vec::new();
    if let Some(path) = &binding.session_messages_path {
        refs.push(path.clone());
        if let Some(prefix) = path.strip_suffix("conversation/messages.json") {
            refs.push(format!("{prefix}control/execution_state.json"));
            refs.push(format!("{prefix}queue/pending_inputs.json"));
        }
    }
    refs.sort();
    refs.dedup();
    refs
}

fn persist_snapshot(
    runtime_home: &Path,
    snapshot: &ProjectRuntimePickupSnapshot,
) -> Result<(), CliError> {
    let dir = runtime_home.join("runtime/projects/runtime_pickups");
    fs::create_dir_all(&dir).map_err(|source| CliError::WriteFile {
        path: dir.display().to_string(),
        source,
    })?;
    for project in &snapshot.projects {
        write_json(&dir.join(format!("{}.json", project.project_id)), project)?;
    }
    write_json(
        &runtime_home.join("runtime/projects/runtime_pickups.json"),
        snapshot,
    )?;
    write_json(
        &runtime_home.join("runtime/current/current_project_runtime_pickups.json"),
        snapshot,
    )
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        project_execution_handoff::materialize_project_execution_handoffs,
        project_supervision::{ProjectSupervisionRecord, ProjectSupervisionSnapshot},
    };
    use fin_config::{
        ConfigMapper, ProviderProtocol, UserConfig, UserProviderConfig, UserRuntimeConfig,
    };
    use fin_contracts::{EntityRefs, ExecutionStateRecord};
    use serde_json::json;
    use std::{
        collections::BTreeMap,
        path::PathBuf,
        time::{SystemTime, UNIX_EPOCH},
    };

    fn temp_runtime_home(prefix: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time")
            .as_nanos();
        let path =
            std::env::temp_dir().join(format!("fin-project-runtime-pickup-{prefix}-{unique}"));
        fs::create_dir_all(path.join("sessions/2026/04/session-1/tasks/registry"))
            .expect("temp runtime home");
        fs::create_dir_all(path.join("sessions/2026/04/session-1/control")).expect("control");
        fs::create_dir_all(path.join("sessions/2026/04/session-1/queue")).expect("queue");
        fs::create_dir_all(path.join("sessions/2026/04/session-1/conversation"))
            .expect("conversation");
        path
    }

    fn system() -> SystemConfig {
        let mut system = ConfigMapper::map_user_to_system(&UserConfig {
            default_provider: "openai".into(),
            providers: BTreeMap::from([(
                "openai".into(),
                UserProviderConfig {
                    protocol: ProviderProtocol::OpenAiCompatible,
                    base_url: "https://api.example.com/v1".into(),
                    model: "gpt-5".into(),
                    api_key: None,
                    api_key_env: Some("OPENAI_API_KEY".into()),
                    user_agent: None,
                    headers: BTreeMap::new(),
                },
            )]),
            runtime: UserRuntimeConfig::default(),
        })
        .expect("system");
        system.runtime.device_name = Some("mbp".into());
        system
            .runtime
            .startup
            .project_agents
            .push(ProjectAgentStartupConfig {
                project_id: "fin".into(),
                mode: ProjectAgentMode::Local,
                project_root: Some("/tmp/fin".into()),
                endpoint: None,
                agent_name: Some("builder".into()),
                worker_budget: 2,
                always_on: true,
                auto_resume: true,
                auto_connect: true,
            });
        system
    }

    #[test]
    fn materialize_marks_ready_to_resume_when_claimed_task_has_pending_inputs() {
        let home = temp_runtime_home("ready");
        fs::write(
            home.join("sessions/2026/04/session-1/tasks/registry/task-1.json"),
            serde_json::to_vec_pretty(&json!({
                "task_id":"task-1",
                "session_id":"session-1",
                "title":"task",
                "summary":"task",
                "status":"ready",
                "created_at":"2026-04-20T23:00:00+08:00",
                "updated_at":"2026-04-20T23:00:00+08:00"
            }))
            .expect("task"),
        )
        .expect("task");
        fs::write(
            home.join("sessions/2026/04/session-1/control/execution_state.json"),
            serde_json::to_vec_pretty(&ExecutionStateRecord {
                state_id: "state-1".into(),
                refs: EntityRefs {
                    session_id: Some("session-1".into()),
                    task_id: Some("task-1".into()),
                    ..EntityRefs::default()
                },
                status: "idle".into(),
                pending_input_count: 1,
                accepts_user_input: true,
                updated_at: "2026-04-20T23:00:00+08:00".into(),
                ..ExecutionStateRecord::default()
            })
            .expect("state"),
        )
        .expect("state");
        fs::write(
            home.join("sessions/2026/04/session-1/queue/pending_inputs.json"),
            serde_json::to_vec_pretty(&json!([{
                "pending_input_id":"pending-1",
                "refs":{"session_id":"session-1","task_id":"task-1"},
                "input_kind":"chat",
                "source":"user.chat",
                "message":"continue",
                "status":"queued",
                "enqueue_reason":"test",
                "enqueued_at":"2026-04-20T23:00:00+08:00"
            }]))
            .expect("pending"),
        )
        .expect("pending");
        fs::write(
            home.join("sessions/2026/04/session-1/conversation/messages.json"),
            serde_json::to_vec_pretty(&json!([])).expect("messages"),
        )
        .expect("messages");

        let supervision = ProjectSupervisionSnapshot {
            updated_at: "2026-04-20T23:01:00+08:00".into(),
            project_count: 1,
            resume_ready_count: 1,
            projects: vec![ProjectSupervisionRecord {
                project_id: "fin".into(),
                agent_id: "mbp.builder".into(),
                mode: "local".into(),
                presence_state: "idle".into(),
                unfinished_task_count: 1,
                resume_task_id: Some("task-1".into()),
                supervision_state: "resume_ready".into(),
                desired_action: "resume_project_task".into(),
                updated_at: "2026-04-20T23:01:00+08:00".into(),
                summary: "resume".into(),
            }],
            ..ProjectSupervisionSnapshot::default()
        };
        let _ =
            materialize_project_execution_handoffs(&home, &supervision, &supervision.updated_at)
                .expect("handoff");

        let snapshot =
            materialize_project_runtime_pickups(&home, &system(), "2026-04-20T23:02:00+08:00")
                .expect("pickup");
        assert_eq!(snapshot.ready_count, 1);
        assert_eq!(snapshot.projects[0].pickup_state, "ready_to_resume");
        assert_eq!(snapshot.projects[0].next_action, "scheduler_tick_needed");
        let presence = fs::read_to_string(home.join("runtime/agents/state/mbp.builder.json"))
            .expect("presence");
        assert!(presence.contains("\"status\": \"idle\""));
        assert!(presence.contains("\"current_task_id\": \"task-1\""));
    }
}
