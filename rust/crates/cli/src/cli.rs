use crate::{
    CliError,
    command::{Command, parse_command},
    config::{default_provider_facade, load_system_config, map_system_config},
    demo::{run_demo, runtime_home_override_from_env},
    fs_utils::read_file,
    install_flow::{build_dev, promote_existing_build, rollback_install},
    mainline_demo::run_mainline_demo,
    runtime_home::{
        ensure_runtime_home_layout, init_runtime_home, persist_runtime_demo, resolved_runtime_home,
    },
    transcript::{load_transcript_scenario, run_transcript_demo},
    web_debug::{serve_web_debug, web_debug_runtime_home},
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
        Command::ConfigCheck { path } => {
            let system = load_system_config(Path::new(&path))?;
            println!(
                "config ok: default_provider={} providers={} runtime_home={}",
                system.default_provider,
                system.providers.len(),
                resolved_runtime_home(&system, runtime_home_override.as_deref()).display()
            );
        }
        Command::HomeInit { path } => {
            let user_toml = read_file(Path::new(&path))?;
            let system = map_system_config(&user_toml)?;
            let runtime_home =
                init_runtime_home(&user_toml, &system, runtime_home_override.as_deref())?;
            println!("home init ok: {}", runtime_home.display());
        }
        Command::MainlineDemo { path } => {
            let user_toml = read_file(Path::new(&path))?;
            let system = map_system_config(&user_toml)?;
            let transcript = run_mainline_demo(&system)?;
            let mut artifacts = None;
            for run in &transcript.runs {
                artifacts = Some(persist_runtime_demo(
                    &user_toml,
                    &system,
                    run,
                    runtime_home_override.as_deref(),
                )?);
            }
            let artifacts = artifacts.ok_or(CliError::Usage)?;
            let last_run = transcript.runs.last().ok_or(CliError::Usage)?;
            println!(
                "mainline demo ok: turns={} session={} task={} last_rounds={} last_events={} home={}",
                transcript.runs.len(),
                transcript.session_id,
                transcript.task_id,
                last_run.round_records.len(),
                last_run.events.len(),
                artifacts.runtime_home.display()
            );
        }
        Command::RuntimeDemo { path, input } => {
            let user_toml = read_file(Path::new(&path))?;
            let system = map_system_config(&user_toml)?;
            let provider = default_provider_facade(&system)?;
            let run = run_demo(&system, &provider, &input)?;
            let artifacts =
                persist_runtime_demo(&user_toml, &system, &run, runtime_home_override.as_deref())?;
            println!(
                "runtime demo ok: provider={} model={} events={} digest={} home={}",
                run.prepared_request.provider_name,
                run.prepared_request.model,
                run.events.len(),
                run.digest.digest_id,
                artifacts.runtime_home.display()
            );
        }
        Command::DebugProjection { path, input } => {
            let user_toml = read_file(Path::new(&path))?;
            let system = map_system_config(&user_toml)?;
            let provider = default_provider_facade(&system)?;
            let run = run_demo(&system, &provider, &input)?;
            persist_runtime_demo(&user_toml, &system, &run, runtime_home_override.as_deref())?;
            println!(
                "{}",
                serde_json::to_string_pretty(&build_projection(&run.events))?
            );
        }
        Command::TranscriptDemo {
            path,
            transcript_path,
        } => {
            let user_toml = read_file(Path::new(&path))?;
            let system = map_system_config(&user_toml)?;
            let provider = default_provider_facade(&system)?;
            let scenario = load_transcript_scenario(Path::new(&transcript_path))?;
            let transcript = run_transcript_demo(&system, &provider, &scenario)?;
            let mut artifacts = None;
            for run in &transcript.runs {
                artifacts = Some(persist_runtime_demo(
                    &user_toml,
                    &system,
                    run,
                    runtime_home_override.as_deref(),
                )?);
            }
            let artifacts = artifacts.ok_or(CliError::Usage)?;
            println!(
                "transcript demo ok: turns={} session={} task={} recent_contexts={} home={}",
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
        Command::WebDebug { path, port } => {
            let user_toml = read_file(Path::new(&path))?;
            let system = map_system_config(&user_toml)?;
            let runtime_home = web_debug_runtime_home(&system, runtime_home_override.as_deref());
            ensure_runtime_home_layout(&runtime_home)?;
            let bind_addr = format!("127.0.0.1:{port}");
            println!(
                "web debug serving: http://{bind_addr} (runtime_home={})",
                runtime_home.display()
            );
            serve_web_debug(user_toml, system, runtime_home, &bind_addr)?;
        }
        Command::BuildDev {
            path,
            build_version,
        } => {
            let user_toml = read_file(Path::new(&path))?;
            let system = map_system_config(&user_toml)?;
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
            let system = map_system_config(&user_toml)?;
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
            let system = map_system_config(&user_toml)?;
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
            let system = map_system_config(&user_toml)?;
            let runtime_home = rollback_install(&system, runtime_home_override.as_deref(), None)?;
            println!("rollback ok: {}", runtime_home.display());
        }
    }
    Ok(())
}
