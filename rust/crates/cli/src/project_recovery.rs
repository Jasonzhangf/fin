use crate::{
    CliError,
    project_execution_handoff::materialize_project_execution_handoffs,
    project_runtime_pickup::materialize_project_runtime_pickups,
    project_supervision::materialize_project_supervision,
    startup_control_summary::read_startup_control_summary,
    startup_topology::{ProjectWakeRequest, StartupTopologySnapshot, materialize_startup_topology},
    startup_wakeup::execute_wake_queue,
};
use fin_config::SystemConfig;
use fin_contracts::DaemonRecoveryActionRecord;
use serde::{Deserialize, Serialize};
use std::{fs, path::Path};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct ProjectRecoveryExecutionReport {
    pub(crate) executed_at: String,
    pub(crate) source: String,
    pub(crate) action_kind: String,
    pub(crate) requested_project_count: usize,
    pub(crate) recovered_project_count: usize,
    pub(crate) waiting_remote_count: usize,
    pub(crate) summary: String,
    #[serde(default)]
    pub(crate) project_ids: Vec<String>,
}

pub(crate) fn execute_project_recovery_if_needed(
    runtime_home: &Path,
    system: &SystemConfig,
    source: &str,
    action: Option<&DaemonRecoveryActionRecord>,
    executed_at: &str,
) -> Result<Option<ProjectRecoveryExecutionReport>, CliError> {
    let Some(action) = action else {
        return Ok(None);
    };
    if action.action_kind != "recover_project_agents" || !action.apply_immediately {
        return Ok(None);
    }

    let snapshot = materialize_startup_topology(runtime_home, system, executed_at)?;
    let targets = recoverable_wake_requests(&snapshot);
    let mut report = ProjectRecoveryExecutionReport {
        executed_at: executed_at.into(),
        source: source.into(),
        action_kind: action.action_kind.clone(),
        requested_project_count: targets.len(),
        ..ProjectRecoveryExecutionReport::default()
    };

    if targets.is_empty() {
        let summary = read_startup_control_summary(runtime_home)?.status_summary();
        report.summary = format!("no recoverable offline project agents; startup={summary}");
        persist_report(runtime_home, &report)?;
        return Ok(Some(report));
    }

    let target_ids = targets
        .iter()
        .map(|item| item.project_id.clone())
        .collect::<Vec<_>>();
    let scoped_snapshot = StartupTopologySnapshot {
        updated_at: snapshot.updated_at.clone(),
        entry_role: snapshot.entry_role.clone(),
        local_worker_budget: snapshot.local_worker_budget,
        projects: snapshot
            .projects
            .into_iter()
            .filter(|item| {
                target_ids
                    .iter()
                    .any(|project_id| project_id == &item.project_id)
            })
            .collect(),
        wake_queue: targets,
    };
    let wake_report = execute_wake_queue(runtime_home, system, &scoped_snapshot, executed_at)?;
    let final_snapshot = materialize_startup_topology(runtime_home, system, executed_at)?;
    let supervision = materialize_project_supervision(runtime_home, &final_snapshot)?;
    let _ = materialize_project_execution_handoffs(runtime_home, &supervision, executed_at)?;
    let _ = materialize_project_runtime_pickups(runtime_home, system, executed_at)?;
    let summary = read_startup_control_summary(runtime_home)?.status_summary();
    report.project_ids = wake_report
        .actions
        .iter()
        .map(|item| item.project_id.clone())
        .collect();
    report.recovered_project_count = wake_report
        .actions
        .iter()
        .filter(|item| item.status == "completed")
        .count();
    report.waiting_remote_count = wake_report
        .actions
        .iter()
        .filter(|item| item.mode == "remote" && item.status == "recorded")
        .count();
    report.summary = format!(
        "recovered={} waiting_remote={} startup={summary}",
        report.recovered_project_count, report.waiting_remote_count
    );
    persist_report(runtime_home, &report)?;
    Ok(Some(report))
}

fn recoverable_wake_requests(snapshot: &StartupTopologySnapshot) -> Vec<ProjectWakeRequest> {
    snapshot
        .wake_queue
        .iter()
        .filter(|request| {
            snapshot.projects.iter().any(|project| {
                project.project_id == request.project_id
                    && project.presence_state == "offline"
                    && (project.always_on || project.unfinished_task_count > 0)
            })
        })
        .cloned()
        .collect()
}

fn persist_report(
    runtime_home: &Path,
    report: &ProjectRecoveryExecutionReport,
) -> Result<(), CliError> {
    let recent_path = runtime_home.join("runtime/projects/recovery_reports.json");
    let mut reports = read_json_or_empty::<ProjectRecoveryExecutionReport>(&recent_path)?;
    reports.push(report.clone());
    trim_head(&mut reports, 16);
    write_json(&recent_path, &reports)?;
    write_json(
        &runtime_home.join("runtime/current/current_project_recovery.json"),
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
    use fin_config::{
        ConfigMapper, ProjectAgentMode, ProjectAgentStartupConfig, ProviderProtocol, UserConfig,
        UserProviderConfig, UserRuntimeConfig,
    };
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
        let path = std::env::temp_dir().join(format!("fin-project-recovery-{prefix}-{unique}"));
        fs::create_dir_all(&path).expect("temp runtime home");
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
    fn executes_local_project_recovery_and_persists_report() {
        let home = temp_runtime_home("local");
        let action = DaemonRecoveryActionRecord {
            action_id: "daemon-recovery-1".into(),
            created_at: "2026-04-20T18:00:00+08:00".into(),
            source: "daemon".into(),
            action_kind: "recover_project_agents".into(),
            apply_immediately: true,
            reason: "recover".into(),
            ..DaemonRecoveryActionRecord::default()
        };

        let report = execute_project_recovery_if_needed(
            &home,
            &system(),
            "daemon",
            Some(&action),
            "2026-04-20T18:00:00+08:00",
        )
        .expect("recovery")
        .expect("report");

        assert_eq!(report.requested_project_count, 1);
        assert_eq!(report.recovered_project_count, 1);
        let presence = fs::read_to_string(home.join("runtime/agents/state/mbp.builder.json"))
            .expect("presence");
        assert!(presence.contains("\"status\": \"idle\""));
        let current =
            fs::read_to_string(home.join("runtime/current/current_project_recovery.json"))
                .expect("current report");
        assert!(current.contains("\"recovered_project_count\": 1"));
    }
}
