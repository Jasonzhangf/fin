use crate::{
    CliError,
    command::{Command, parse_command},
    config::{
        default_provider_facade, import_rcc_provider_profile, load_effective_system_config,
        load_system_config,
    },
    control_boundary_scenario::run_control_boundary_scenario,
    fs_utils::read_file,
    headless_daemon::{run_headless_daemon, start_headless_daemon, stop_headless_daemon},
    install_flow::{build_dev, promote_existing_build, rollback_install},
    local_multi_agent_lifecycle_harness::{
        run_local_multi_agent_lifecycle_harness_with_config, run_local_multi_agent_node,
    },
    mainline_scenario::run_mainline_scenario,
    project_agent_harness_commands::run_project_agent_command,
    provider_live_smoke::run_provider_live_smoke,
    qqbot_live_receipt::run_qqbot_live_receipt,
    runtime_home::{init_runtime_home, persist_runtime_session, resolved_runtime_home},
    session_run::{run_session, runtime_home_override_from_env},
    transcript::{load_transcript_scenario, run_transcript_session},
    web_debug_entry::serve_web_debug,
};
use fin_debug_server::build_projection;
use std::{path::Path, path::PathBuf};

pub fn run(args: Vec<String>) -> Result<(), CliError> {
    run_with_runtime_home(args, runtime_home_override_from_env())
}

pub fn run_with_runtime_home(
    args: Vec<String>,
    runtime_home_override: Option<PathBuf>,
) -> Result<(), CliError> {
    match parse_command(&args)? {
        Command::Start { path } => {
            let user_toml = read_file(Path::new(&path))?;
            let system =
                load_effective_system_config(&user_toml, runtime_home_override.as_deref())?;
            let report =
                start_headless_daemon(&user_toml, &system, runtime_home_override.as_deref())?;
            println!(
                "headless daemon {}: daemon_id={} pid={} runtime_home={}",
                report.status,
                report.daemon_id,
                report
                    .pid
                    .map(|value| value.to_string())
                    .unwrap_or_else(|| "-".into()),
                report.runtime_home.display()
            );
        }
        Command::Stop { path } => {
            let user_toml = read_file(Path::new(&path))?;
            let system =
                load_effective_system_config(&user_toml, runtime_home_override.as_deref())?;
            let report =
                stop_headless_daemon(&user_toml, &system, runtime_home_override.as_deref())?;
            println!(
                "headless daemon {}: daemon_id={} pid={} runtime_home={}",
                report.status,
                report.daemon_id,
                report
                    .pid
                    .map(|value| value.to_string())
                    .unwrap_or_else(|| "-".into()),
                report.runtime_home.display()
            );
        }
        Command::DaemonRun { path } => {
            let user_toml = read_file(Path::new(&path))?;
            let system =
                load_effective_system_config(&user_toml, runtime_home_override.as_deref())?;
            let provider = default_provider_facade(&system)?;
            let report = run_headless_daemon(
                &user_toml,
                &system,
                &provider,
                runtime_home_override.as_deref(),
            )?;
            println!(
                "headless daemon run ok: daemon_id={} cycles={} processed_sessions={} drove={} runtime_home={}",
                report.daemon_id,
                report.cycles_completed,
                report.processed_sessions,
                report.drove_count,
                report.runtime_home.display()
            );
        }
        Command::ConfigCheck { path } => {
            let system = load_system_config(Path::new(&path))?;
            println!(
                "config ok: default_provider={} providers={} runtime_home={}",
                system.default_provider,
                system.providers.len(),
                resolved_runtime_home(&system, runtime_home_override.as_deref()).display()
            );
        }
        Command::ConfigImportRcc {
            path,
            provider_json,
        } => {
            let system = import_rcc_provider_profile(Path::new(&path), Path::new(&provider_json))?;
            let provider = system.default_provider_config()?;
            println!(
                "config imported: provider={} model={} protocol={:?}",
                system.default_provider, provider.model, provider.protocol
            );
        }
        Command::HomeInit { path } => {
            let user_toml = read_file(Path::new(&path))?;
            let system =
                load_effective_system_config(&user_toml, runtime_home_override.as_deref())?;
            let runtime_home =
                init_runtime_home(&user_toml, &system, runtime_home_override.as_deref())?;
            println!("home init ok: {}", runtime_home.display());
        }
        Command::ControlBoundaryScenario { path } => {
            let user_toml = read_file(Path::new(&path))?;
            let system =
                load_effective_system_config(&user_toml, runtime_home_override.as_deref())?;
            let run = run_control_boundary_scenario(
                &user_toml,
                &system,
                runtime_home_override.as_deref(),
            )?;
            println!(
                "control boundary scenario ok: session={} task={} responses={} home={}",
                run.session_id,
                run.task_id,
                run.responses.len(),
                run.runtime_home.display()
            );
        }
        Command::MainlineScenario { path } => {
            let user_toml = read_file(Path::new(&path))?;
            let system =
                load_effective_system_config(&user_toml, runtime_home_override.as_deref())?;
            let transcript = run_mainline_scenario(&system)?;
            let mut artifacts = None;
            for run in &transcript.runs {
                artifacts = Some(persist_runtime_session(
                    &user_toml,
                    &system,
                    run,
                    runtime_home_override.as_deref(),
                )?);
            }
            let artifacts = artifacts.ok_or(CliError::Usage)?;
            let last_run = transcript.runs.last().ok_or(CliError::Usage)?;
            println!(
                "mainline scenario ok: turns={} session={} task={} last_rounds={} last_events={} home={}",
                transcript.runs.len(),
                transcript.session_id,
                transcript.task_id,
                last_run.round_records.len(),
                last_run.events.len(),
                artifacts.runtime_home.display()
            );
        }
        Command::RuntimeSession { path, input } => {
            let user_toml = read_file(Path::new(&path))?;
            let system =
                load_effective_system_config(&user_toml, runtime_home_override.as_deref())?;
            let provider = default_provider_facade(&system)?;
            let run = run_session(&system, &provider, &input)?;
            let artifacts = persist_runtime_session(
                &user_toml,
                &system,
                &run,
                runtime_home_override.as_deref(),
            )?;
            println!(
                "runtime session ok: provider={} model={} events={} digest={} home={}",
                run.prepared_request.provider_name,
                run.prepared_request.model,
                run.events.len(),
                run.digest.digest_id,
                artifacts.runtime_home.display()
            );
        }
        Command::DebugProjection { path, input } => {
            let user_toml = read_file(Path::new(&path))?;
            let system =
                load_effective_system_config(&user_toml, runtime_home_override.as_deref())?;
            let provider = default_provider_facade(&system)?;
            let run = run_session(&system, &provider, &input)?;
            persist_runtime_session(&user_toml, &system, &run, runtime_home_override.as_deref())?;
            println!(
                "{}",
                serde_json::to_string_pretty(&build_projection(&run.events))?
            );
        }
        Command::TranscriptSession {
            path,
            transcript_path,
        } => {
            let user_toml = read_file(Path::new(&path))?;
            let system =
                load_effective_system_config(&user_toml, runtime_home_override.as_deref())?;
            let provider = default_provider_facade(&system)?;
            let scenario = load_transcript_scenario(Path::new(&transcript_path))?;
            let transcript = run_transcript_session(&system, &provider, &scenario)?;
            let mut artifacts = None;
            for run in &transcript.runs {
                artifacts = Some(persist_runtime_session(
                    &user_toml,
                    &system,
                    run,
                    runtime_home_override.as_deref(),
                )?);
            }
            let artifacts = artifacts.ok_or(CliError::Usage)?;
            println!(
                "transcript session ok: turns={} session={} task={} recent_contexts={} home={}",
                transcript.runs.len(),
                transcript.session_id,
                transcript.task_id,
                artifacts
                    .session_dir
                    .join("context/recent_contexts.json")
                    .display(),
                artifacts.runtime_home.display()
            );
        }
        Command::ProviderLiveSmoke {
            path,
            transcript_path,
        } => {
            let user_toml = read_file(Path::new(&path))?;
            let system =
                load_effective_system_config(&user_toml, runtime_home_override.as_deref())?;
            let report = run_provider_live_smoke(
                &user_toml,
                &system,
                runtime_home_override.as_deref(),
                transcript_path.as_deref().map(Path::new),
            )?;
            println!(
                "provider live smoke ok: run_id={} session={} task={} turns={} report={} runtime_home={}",
                report.run_id,
                report.session_id,
                report.task_id,
                report.turn_count,
                report.receipt_path,
                report.runtime_home
            );
        }
        Command::QqbotLiveReceipt {
            path,
            target,
            run_id,
        } => {
            let user_toml = read_file(Path::new(&path))?;
            let system =
                load_effective_system_config(&user_toml, runtime_home_override.as_deref())?;
            let report = run_qqbot_live_receipt(
                &system,
                runtime_home_override.as_deref(),
                &target,
                run_id.as_deref(),
            )?;
            println!(
                "qqbot live receipt ok: target={} session={} status={} receipt={}",
                report.target, report.session_id, report.status, report.receipt_path
            );
        }
        Command::ProjectAgent { path, args } => {
            let user_toml = read_file(Path::new(&path))?;
            let system =
                load_effective_system_config(&user_toml, runtime_home_override.as_deref())?;
            let runtime_home =
                init_runtime_home(&user_toml, &system, runtime_home_override.as_deref())?;
            run_project_agent_command(&runtime_home, &system, &args)?;
        }
        Command::WebDebug { path, host, port } => {
            let user_toml = read_file(Path::new(&path))?;
            let system =
                load_effective_system_config(&user_toml, runtime_home_override.as_deref())?;
            let runtime_home =
                init_runtime_home(&user_toml, &system, runtime_home_override.as_deref())?;
            let bind_addr = format!("{host}:{port}");
            println!(
                "web debug serving: http://{bind_addr} (runtime_home={})",
                runtime_home.display()
            );
            serve_web_debug(
                Path::new(&path),
                user_toml,
                system,
                runtime_home,
                &bind_addr,
            )?;
        }
        Command::BuildDev {
            path,
            build_version,
        } => {
            let user_toml = read_file(Path::new(&path))?;
            let system =
                load_effective_system_config(&user_toml, runtime_home_override.as_deref())?;
            let artifacts = build_dev(
                &user_toml,
                &system,
                build_version,
                runtime_home_override.as_deref(),
                None,
                None,
            )?;
            println!(
                "build dev ok: build_version={} current={} bin={}",
                artifacts.build_version,
                artifacts.version_dir.display(),
                artifacts.bin_link.display()
            );
        }
        Command::InstallDev {
            path,
            build_version,
        } => {
            let user_toml = read_file(Path::new(&path))?;
            let system =
                load_effective_system_config(&user_toml, runtime_home_override.as_deref())?;
            let artifacts = build_dev(
                &user_toml,
                &system,
                build_version,
                runtime_home_override.as_deref(),
                None,
                None,
            )?;
            println!(
                "install dev ok: build_version={} current={} bin={}",
                artifacts.build_version,
                artifacts.version_dir.display(),
                artifacts.bin_link.display()
            );
        }
        Command::Promote {
            path,
            build_version,
        } => {
            let user_toml = read_file(Path::new(&path))?;
            let system =
                load_effective_system_config(&user_toml, runtime_home_override.as_deref())?;
            promote_existing_build(
                &user_toml,
                &system,
                &build_version,
                runtime_home_override.as_deref(),
                None,
            )?;
            println!(
                "promote ok: build_version={} current={}",
                build_version,
                resolved_runtime_home(&system, runtime_home_override.as_deref())
                    .join("install/current")
                    .display()
            );
        }
        Command::Rollback { path } => {
            let user_toml = read_file(Path::new(&path))?;
            let system =
                load_effective_system_config(&user_toml, runtime_home_override.as_deref())?;
            let runtime_home = rollback_install(&system, runtime_home_override.as_deref(), None)?;
            println!("rollback ok: {}", runtime_home.display());
        }
        Command::LocalMultiAgentHarness { path, project_cwd } => {
            let user_toml = read_file(Path::new(&path))?;
            let system =
                load_effective_system_config(&user_toml, runtime_home_override.as_deref())?;
            let runtime_home = resolved_runtime_home(&system, runtime_home_override.as_deref());
            let receipt = run_local_multi_agent_lifecycle_harness_with_config(
                &runtime_home,
                Some(Path::new(&path)),
                Path::new(&project_cwd),
                None,
            )?;
            println!("{}", serde_json::to_string_pretty(&receipt)?);
        }
        Command::LocalMultiAgentNode {
            runtime_home,
            role,
            agent_id,
            config_path,
        } => {
            run_local_multi_agent_node(
                Path::new(&runtime_home),
                &role,
                &agent_id,
                config_path.as_deref().map(Path::new),
            )?;
        }
    }
    Ok(())
}
