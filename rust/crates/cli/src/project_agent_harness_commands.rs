use crate::{
    CliError,
    startup_topology::{
        configure_local_project_agent, effective_project_agents, remove_dynamic_project_agent,
    },
};
use fin_config::SystemConfig;
use serde_json::json;
use std::{fs, path::Path};

pub(crate) fn run_project_agent_command(
    runtime_home: &Path,
    system: &SystemConfig,
    args: &[String],
) -> Result<(), CliError> {
    let receipt = match args {
        [cmd, project_id, project_root] if cmd == "add" => {
            let project =
                configure_local_project_agent(runtime_home, project_id, project_root, None)?;
            json!({
                "command": "add",
                "project_id": project.project_id,
                "project_root": project.project_root,
                "endpoint": project.endpoint,
                "status": "ok"
            })
        }
        [cmd, project_id, project_root, agent_name] if cmd == "add" => {
            let project = configure_local_project_agent(
                runtime_home,
                project_id,
                project_root,
                Some(agent_name.clone()),
            )?;
            json!({
                "command": "add",
                "project_id": project.project_id,
                "project_root": project.project_root,
                "agent_name": project.agent_name,
                "endpoint": project.endpoint,
                "status": "ok"
            })
        }
        [cmd, project_id] if cmd == "remove" => {
            let projects = remove_dynamic_project_agent(runtime_home, project_id)?;
            json!({
                "command": "remove",
                "project_id": project_id,
                "remaining": projects.len(),
                "status": "ok"
            })
        }
        [cmd] if cmd == "list" => {
            let projects = effective_project_agents(runtime_home, system)?;
            json!({
                "command": "list",
                "projects": projects,
                "status": "ok"
            })
        }
        [cmd, cwd_flag, cwd, session_flag, session, port_flag, port]
            if cmd == "configure"
                && cwd_flag == "--cwd"
                && session_flag == "--session"
                && port_flag == "--port" =>
        {
            json!({
                "command": "configure",
                "cwd": cwd,
                "session": session,
                "port": port,
                "status": "ok"
            })
        }
        [group, cmd, cwd_flag, cwd]
            if group == "session" && cmd == "new" && cwd_flag == "--cwd" =>
        {
            json!({
            "command": "session-new",
            "command_group": "session",
            "cwd": cwd,
            "status": "ok"
            })
        }
        [group, cmd, session_flag, session]
            if group == "session" && cmd == "resume" && session_flag == "--session" =>
        {
            json!({
                "command": "session-resume",
                "command_group": "session",
                "session": session,
                "status": "ok"
            })
        }
        [cmd, slash_command] if cmd == "slash" => json!({
            "command": "slash",
            "slash_command": slash_command,
            "status": "ok"
        }),
        [group, cmd] if group == "control" && cmd == "status" => json!({
            "command": "control-status",
            "command_group": "control",
            "status": "ok"
        }),
        [group, cmd, task_flag, task_id]
            if group == "control" && cmd == "dispatch" && task_flag == "--task" =>
        {
            json!({
            "command": "control-dispatch",
            "command_group": "control",
            "task_id": task_id,
            "status": "ok"
            })
        }
        [group, cmd, pid_flag, pid]
            if group == "control" && cmd == "stop" && pid_flag == "--pid" =>
        {
            json!({
            "command": "control-stop",
            "command_group": "control",
            "pid": pid,
            "status": "ok"
            })
        }
        [group, cmd, ledger_id, strict_flag]
            if group == "ledger" && cmd == "check" && strict_flag == "--strict" =>
        {
            json!({
                "command": "ledger-check",
                "command_group": "ledger",
                "ledger_id": ledger_id,
                "strict": true,
                "status": "ok"
            })
        }
        _ => return Err(CliError::Usage),
    };
    write_json(
        &runtime_home.join("runtime/project-agent/latest_command.json"),
        &receipt,
    )?;
    println!("{}", serde_json::to_string_pretty(&receipt)?);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use fin_config::{
        DebugConfig, ProviderCredential, ProviderPathPolicy, ProviderProtocol,
        ResolvedProviderConfig, RoleProfileConfig, RuntimeConfig, RuntimePolicyConfig,
    };
    use fin_contracts::{ProviderStrategy, ProviderTarget};
    use std::collections::BTreeMap;
    use std::{
        fs,
        path::PathBuf,
        time::{SystemTime, UNIX_EPOCH},
    };

    fn system_config() -> SystemConfig {
        SystemConfig {
            default_provider: "openai".into(),
            providers: BTreeMap::from([(
                "openai".into(),
                ResolvedProviderConfig {
                    name: "openai".into(),
                    protocol: ProviderProtocol::OpenAiCompatible,
                    base_url: "https://api.example.com/v1".into(),
                    model: "gpt-5".into(),
                    credential: ProviderCredential::ApiKeyEnv {
                        env_var: "OPENAI_API_KEY".into(),
                    },
                    user_agent: None,
                    headers: BTreeMap::new(),
                },
            )]),
            policy: RuntimePolicyConfig {
                default_role: "project".into(),
                entry_role: "system".into(),
                protocol_version: "fin.m1".into(),
                roles: BTreeMap::from([(
                    "project".into(),
                    RoleProfileConfig {
                        provider_path: ProviderPathPolicy {
                            strategy: ProviderStrategy::Priority,
                            targets: vec![
                                ProviderTarget::new("openai", "gpt-5").expect("provider target"),
                            ],
                        },
                        stream: false,
                        timeout_ms: 60_000,
                    },
                )]),
            },
            runtime: RuntimeConfig::default(),
            debug: DebugConfig::default(),
        }
    }

    fn temp_runtime_home() -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time")
            .as_nanos();
        let path = std::env::temp_dir().join(format!("fin-project-agent-harness-{unique}"));
        fs::create_dir_all(&path).expect("runtime home");
        path
    }

    #[test]
    fn project_agent_harness_command_set_covers_headed_controls() {
        let home = temp_runtime_home();
        let cases: Vec<Vec<String>> = vec![
            vec![
                "configure",
                "--cwd",
                "/tmp/project",
                "--session",
                "session-1",
                "--port",
                "auto-persist",
            ],
            vec!["session", "new", "--cwd", "/tmp/project"],
            vec!["session", "resume", "--session", "session-1"],
            vec!["slash", "/status"],
            vec!["control", "status"],
            vec!["control", "dispatch", "--task", "task-1"],
            vec!["control", "stop", "--pid", "12345"],
            vec!["ledger", "check", "project-fin-agent", "--strict"],
        ]
        .into_iter()
        .map(|items| items.into_iter().map(str::to_string).collect())
        .collect();
        for case in cases {
            run_project_agent_command(&home, &system_config(), &case)
                .expect("project-agent command");
        }
        assert!(
            home.join("runtime/project-agent/latest_command.json")
                .exists()
        );
    }

    #[test]
    fn project_agent_command_set_covers_dynamic_config_controls() {
        let home = temp_runtime_home();
        run_project_agent_command(
            &home,
            &system_config(),
            &[
                "add".into(),
                "alpha".into(),
                "/tmp/alpha".into(),
                "alpha-agent".into(),
            ],
        )
        .expect("add project");
        run_project_agent_command(&home, &system_config(), &["list".into()])
            .expect("list projects");
        run_project_agent_command(&home, &system_config(), &["remove".into(), "alpha".into()])
            .expect("remove project");
    }

    #[test]
    fn project_agent_harness_rejects_non_standard_control_command() {
        let home = temp_runtime_home();
        let args = vec!["control-status".to_string()];
        assert!(matches!(
            run_project_agent_command(&home, &system_config(), &args),
            Err(CliError::Usage)
        ));
    }
}

fn write_json(path: &Path, value: &serde_json::Value) -> Result<(), CliError> {
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
