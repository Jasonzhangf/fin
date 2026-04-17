use fin_config::{ConfigMapper, SystemConfig, parse_user_toml, system_to_toml};
use fin_contracts::OperationEnvelope;
use fin_debug_server::{build_projection, persist_snapshot, serve_debug_mvp};
use fin_provider::{ProviderDescriptor, ProviderRegistry};
use fin_runtime::M1Runtime;
use fin_shared::expand_home_path;
use serde_json::json;
use std::{
    env, fs,
    path::{Path, PathBuf},
    process::Command as ProcessCommand,
    time::{SystemTime, UNIX_EPOCH},
};
use thiserror::Error;

#[cfg(unix)]
use std::os::unix::fs as unix_fs;

#[derive(Debug, Error)]
enum CliError {
    #[error(
        "usage: fin <config-check|home-init|runtime-demo|debug-projection|web-debug|install-dev|rollback> <user.toml> [input|port|build-id]"
    )]
    Usage,
    #[error("invalid port: {0}")]
    InvalidPort(String),
    #[error("command failed: {command} (exit={exit_code:?})")]
    ProcessFailed {
        command: String,
        exit_code: Option<i32>,
    },
    #[error("missing install target: {0}")]
    MissingInstallTarget(String),
    #[error("invalid install state: {0}")]
    InvalidInstallState(String),
    #[error("failed to read file '{path}': {source}")]
    ReadFile {
        path: String,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to write file '{path}': {source}")]
    WriteFile {
        path: String,
        #[source]
        source: std::io::Error,
    },
    #[error(transparent)]
    Config(#[from] fin_config::ConfigError),
    #[error(transparent)]
    Provider(#[from] fin_provider::ProviderError),
    #[error(transparent)]
    Runtime(#[from] fin_runtime::RuntimeError),
    #[error(transparent)]
    DebugData(#[from] fin_debug_server::DebugDataError),
    #[error(transparent)]
    Serialize(#[from] serde_json::Error),
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Command {
    ConfigCheck {
        path: String,
    },
    HomeInit {
        path: String,
    },
    RuntimeDemo {
        path: String,
        input: String,
    },
    DebugProjection {
        path: String,
        input: String,
    },
    WebDebug {
        path: String,
        port: u16,
    },
    InstallDev {
        path: String,
        build_id: Option<String>,
    },
    Rollback {
        path: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RuntimeHomeArtifacts {
    runtime_home: PathBuf,
    projection_json: PathBuf,
    snapshot_json: PathBuf,
    session_dir: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct InstallArtifacts {
    build_id: String,
    runtime_home: PathBuf,
    staged_dir: PathBuf,
    version_dir: PathBuf,
    bin_link: PathBuf,
}

fn main() {
    if let Err(err) = run(env::args().skip(1).collect()) {
        eprintln!("{err}");
        std::process::exit(1);
    }
}

fn run(args: Vec<String>) -> Result<(), CliError> {
    run_with_runtime_home(args, runtime_home_override_from_env())
}

fn run_with_runtime_home(
    args: Vec<String>,
    runtime_home_override: Option<PathBuf>,
) -> Result<(), CliError> {
    let command = parse_command(&args)?;
    match command {
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
        Command::RuntimeDemo { path, input } => {
            let user_toml = read_file(Path::new(&path))?;
            let system = map_system_config(&user_toml)?;
            let provider = default_provider_descriptor(&system)?;
            let run = run_demo(&provider, &input)?;
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
            let provider = default_provider_descriptor(&system)?;
            let run = run_demo(&provider, &input)?;
            persist_runtime_demo(&user_toml, &system, &run, runtime_home_override.as_deref())?;
            let projection = build_projection(&run.events);
            println!("{}", serde_json::to_string_pretty(&projection)?);
        }
        Command::WebDebug { path, port } => {
            let user_toml = read_file(Path::new(&path))?;
            let system = map_system_config(&user_toml)?;
            let runtime_home = resolved_runtime_home(&system, runtime_home_override.as_deref());
            ensure_runtime_home_layout(&runtime_home)?;
            let bind_addr = format!("127.0.0.1:{port}");
            println!(
                "web debug serving: http://{bind_addr} (runtime_home={})",
                runtime_home.display()
            );
            serve_debug_mvp(&runtime_home, &bind_addr)?;
        }
        Command::InstallDev { path, build_id } => {
            let user_toml = read_file(Path::new(&path))?;
            let system = map_system_config(&user_toml)?;
            let artifacts = install_dev(
                &user_toml,
                &system,
                build_id,
                runtime_home_override.as_deref(),
                None,
                None,
            )?;
            println!(
                "install dev ok: build_id={} current={} bin={}",
                artifacts.build_id,
                artifacts.version_dir.display(),
                artifacts.bin_link.display()
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

fn parse_command(args: &[String]) -> Result<Command, CliError> {
    match args {
        [cmd, path] if cmd == "config-check" => Ok(Command::ConfigCheck { path: path.clone() }),
        [cmd, path] if cmd == "home-init" => Ok(Command::HomeInit { path: path.clone() }),
        [cmd, path, input] if cmd == "runtime-demo" => Ok(Command::RuntimeDemo {
            path: path.clone(),
            input: input.clone(),
        }),
        [cmd, path, input] if cmd == "debug-projection" => Ok(Command::DebugProjection {
            path: path.clone(),
            input: input.clone(),
        }),
        [cmd, path] if cmd == "web-debug" => Ok(Command::WebDebug {
            path: path.clone(),
            port: 4040,
        }),
        [cmd, path, port] if cmd == "web-debug" => Ok(Command::WebDebug {
            path: path.clone(),
            port: port
                .parse::<u16>()
                .map_err(|_| CliError::InvalidPort(port.clone()))?,
        }),
        [cmd, path] if cmd == "install-dev" => Ok(Command::InstallDev {
            path: path.clone(),
            build_id: None,
        }),
        [cmd, path, build_id] if cmd == "install-dev" => Ok(Command::InstallDev {
            path: path.clone(),
            build_id: Some(build_id.clone()),
        }),
        [cmd, path] if cmd == "rollback" => Ok(Command::Rollback { path: path.clone() }),
        _ => Err(CliError::Usage),
    }
}

fn read_file(path: &Path) -> Result<String, CliError> {
    fs::read_to_string(path).map_err(|source| CliError::ReadFile {
        path: path.display().to_string(),
        source,
    })
}

fn map_system_config(user_toml: &str) -> Result<SystemConfig, CliError> {
    let user = parse_user_toml(user_toml)?;
    Ok(ConfigMapper::map_user_to_system(&user)?)
}

fn load_system_config(path: &Path) -> Result<SystemConfig, CliError> {
    let content = read_file(path)?;
    map_system_config(&content)
}

fn default_provider_descriptor(system: &SystemConfig) -> Result<ProviderDescriptor, CliError> {
    let mut registry = ProviderRegistry::default();
    for provider in system.providers.values() {
        registry.register_resolved(provider)?;
    }
    let provider = system.default_provider_config()?;
    Ok(registry
        .get(&provider.name)
        .expect("default provider should be registered")
        .clone())
}

fn run_demo(
    provider: &ProviderDescriptor,
    input: &str,
) -> Result<fin_runtime::ClosureRun, CliError> {
    let mut runtime = M1Runtime::default();
    let demo_ids = demo_identity(demo_namespace_from_env().as_deref());
    let mut operation = OperationEnvelope::new(
        demo_ids.operation_id,
        "start_inference",
        "2026-04-17T00:00:00Z",
        "cli",
        demo_ids.trace_id,
        json!({"input": input}),
    );
    operation.refs.session_id = Some(demo_ids.session_id);
    operation.refs.task_id = Some(demo_ids.task_id);
    Ok(runtime.run_closure(operation, provider)?)
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct DemoIdentity {
    operation_id: String,
    trace_id: String,
    session_id: String,
    task_id: String,
}

fn runtime_home_override_from_env() -> Option<PathBuf> {
    env::var("FIN_RUNTIME_HOME_OVERRIDE")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .map(PathBuf::from)
}

fn demo_namespace_from_env() -> Option<String> {
    env::var("FIN_SESSION_NAMESPACE")
        .ok()
        .map(|raw| sanitize_id_fragment(&raw))
        .filter(|value| !value.is_empty())
}

fn demo_identity(namespace: Option<&str>) -> DemoIdentity {
    let scope = namespace.unwrap_or("cli-demo");
    DemoIdentity {
        operation_id: format!("op-{scope}"),
        trace_id: format!("trace-{scope}"),
        session_id: format!("session-{scope}"),
        task_id: format!("task-{scope}"),
    }
}

fn sanitize_id_fragment(raw: &str) -> String {
    raw.trim()
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                ch
            } else {
                '-'
            }
        })
        .collect::<String>()
        .trim_matches('-')
        .to_string()
}

fn resolved_runtime_home(system: &SystemConfig, override_path: Option<&Path>) -> PathBuf {
    override_path
        .map(Path::to_path_buf)
        .unwrap_or_else(|| expand_home_path(&system.runtime.runtime_home))
}

fn init_runtime_home(
    user_toml: &str,
    system: &SystemConfig,
    override_path: Option<&Path>,
) -> Result<PathBuf, CliError> {
    let runtime_home = resolved_runtime_home(system, override_path);
    ensure_runtime_home_layout(&runtime_home)?;

    let config_dir = runtime_home.join("config");
    write_file(&config_dir.join("user.toml"), user_toml.as_bytes())?;
    write_file(
        &config_dir.join("system.toml"),
        system_to_toml(system)?.as_bytes(),
    )?;
    write_file(
        &config_dir.join("system.template.toml"),
        system_to_toml(system)?.as_bytes(),
    )?;

    Ok(runtime_home)
}

fn ensure_runtime_home_layout(runtime_home: &Path) -> Result<(), CliError> {
    for relative in [
        "config",
        "bin",
        "install/staged",
        "install/versions",
        "install/receipts",
        "runtime/locks",
        "runtime/pids",
        "runtime/sockets",
        "runtime/leases",
        "runtime/heartbeats",
        "runtime/projections",
        "runtime/current",
        "logs/cli",
        "logs/runtime",
        "logs/provider",
        "logs/orchestrator",
        "logs/debug-server",
        "logs/install",
        "logs/regression",
        "diagnostics/crashes",
        "diagnostics/error-samples",
        "diagnostics/traces",
        "diagnostics/snapshots",
        "diagnostics/repro",
        "harness/recordings",
        "harness/replays",
        "harness/fault-injection",
        "harness/baselines",
        "harness/reports",
        "workdirs",
        "archive/sessions",
        "archive/logs",
        "archive/diagnostics",
        "archive/harness",
        "tmp",
    ] {
        fs::create_dir_all(runtime_home.join(relative)).map_err(|source| CliError::WriteFile {
            path: runtime_home.join(relative).display().to_string(),
            source,
        })?;
    }
    Ok(())
}

fn install_dev(
    user_toml: &str,
    system: &SystemConfig,
    build_id: Option<String>,
    runtime_home_override: Option<&Path>,
    source_executable_override: Option<&Path>,
    smoke_home_override: Option<&Path>,
) -> Result<InstallArtifacts, CliError> {
    let runtime_home = init_runtime_home(user_toml, system, runtime_home_override)?;
    let repo_root = repo_root()?;
    let build_id = build_id.unwrap_or_else(generate_build_id);
    let install_log = runtime_home
        .join("logs/install")
        .join(format!("{build_id}.log"));
    let regression_log = runtime_home
        .join("logs/regression")
        .join(format!("{build_id}.log"));
    let smoke_config_path = runtime_home.join("config/user.toml");
    let smoke_home = resolve_smoke_home(&runtime_home, smoke_home_override);

    run_source_validation(&repo_root, &install_log)?;
    let staged_dir = stage_build(
        &runtime_home,
        &build_id,
        source_executable_override
            .map(Path::to_path_buf)
            .unwrap_or(env::current_exe().map_err(|source| CliError::ReadFile {
                path: "current executable".into(),
                source,
            })?),
        &install_log,
    )?;
    let staged_binary = staged_dir.join("bin/fin");
    run_installed_smoke(
        &staged_binary,
        &smoke_config_path,
        &smoke_home,
        &runtime_home,
        &build_id,
        &regression_log,
    )?;
    let version_dir = promote_build(&runtime_home, &build_id, &install_log)?;
    run_post_install_smoke(
        &runtime_home,
        &smoke_config_path,
        &smoke_home,
        &build_id,
        &install_log,
    )?;

    Ok(InstallArtifacts {
        build_id,
        runtime_home: runtime_home.clone(),
        staged_dir,
        version_dir,
        bin_link: runtime_home.join("bin/fin"),
    })
}

fn rollback_install(
    system: &SystemConfig,
    runtime_home_override: Option<&Path>,
    smoke_home_override: Option<&Path>,
) -> Result<PathBuf, CliError> {
    let runtime_home = resolved_runtime_home(system, runtime_home_override);
    ensure_runtime_home_layout(&runtime_home)?;
    let current_link = runtime_home.join("install/current");
    let previous_link = runtime_home.join("install/previous");
    let current_target = fs::read_link(&current_link).map_err(|source| CliError::ReadFile {
        path: current_link.display().to_string(),
        source,
    })?;
    let previous_target = fs::read_link(&previous_link).map_err(|source| CliError::ReadFile {
        path: previous_link.display().to_string(),
        source,
    })?;
    let current_build = build_id_from_link(&current_target)?;
    let previous_build = build_id_from_link(&previous_target)?;
    let install_log = runtime_home
        .join("logs/install")
        .join(format!("rollback-{previous_build}.log"));
    replace_symlink(&current_link, &previous_target)?;
    replace_symlink(&previous_link, &current_target)?;
    replace_symlink(
        &runtime_home.join("bin/fin"),
        &runtime_home.join("install/current/bin/fin"),
    )?;
    write_receipt(
        &runtime_home,
        &format!("rollback-{previous_build}"),
        &json!({
            "action": "rollback",
            "rolled_back_from": current_build,
            "current_version": previous_build,
            "previous_version": current_build,
            "timestamp": now_unix_seconds(),
        }),
    )?;
    let smoke_home = resolve_smoke_home(&runtime_home, smoke_home_override);
    let config_path = runtime_home.join("config/user.toml");
    run_post_install_smoke(
        &runtime_home,
        &config_path,
        &smoke_home,
        &format!("rollback-{previous_build}"),
        &install_log,
    )?;
    Ok(runtime_home)
}

fn repo_root() -> Result<PathBuf, CliError> {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../..")
        .canonicalize()
        .map_err(|source| CliError::ReadFile {
            path: "repo root".into(),
            source,
        })
}

fn generate_build_id() -> String {
    let ts = now_unix_seconds();
    let git = short_git_sha().unwrap_or_else(|| "nogit".into());
    format!("{ts}-{git}-dev")
}

fn short_git_sha() -> Option<String> {
    let repo_root = repo_root().ok()?;
    let output = ProcessCommand::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .current_dir(repo_root)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn now_unix_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time should be after epoch")
        .as_secs()
}

fn resolve_smoke_home(runtime_home: &Path, override_path: Option<&Path>) -> PathBuf {
    override_path
        .map(Path::to_path_buf)
        .unwrap_or_else(|| runtime_home.parent().unwrap_or(runtime_home).to_path_buf())
}

fn run_source_validation(repo_root: &Path, log_path: &Path) -> Result<(), CliError> {
    run_process_and_log(
        ProcessCommand::new(repo_root.join("scripts/verify-governance.sh")),
        "verify-governance",
        log_path,
    )?;
    run_process_and_log(
        ProcessCommand::new(repo_root.join("scripts/check-code-line-limit.py")),
        "check-code-line-limit",
        log_path,
    )?;
    let mut fmt = ProcessCommand::new("cargo");
    fmt.args(["fmt", "--check"])
        .current_dir(repo_root.join("rust"));
    run_process_and_log(fmt, "cargo fmt --check", log_path)?;
    let mut test = ProcessCommand::new("cargo");
    test.args(["test"]).current_dir(repo_root.join("rust"));
    run_process_and_log(test, "cargo test", log_path)?;
    Ok(())
}

fn stage_build(
    runtime_home: &Path,
    build_id: &str,
    source_executable: PathBuf,
    log_path: &Path,
) -> Result<PathBuf, CliError> {
    let staged_dir = runtime_home.join("install/staged").join(build_id);
    if staged_dir.exists() {
        return Err(CliError::InvalidInstallState(format!(
            "staged build already exists: {}",
            staged_dir.display()
        )));
    }
    fs::create_dir_all(staged_dir.join("bin")).map_err(|source| CliError::WriteFile {
        path: staged_dir.join("bin").display().to_string(),
        source,
    })?;
    let staged_binary = staged_dir.join("bin/fin");
    fs::copy(&source_executable, &staged_binary).map_err(|source| CliError::WriteFile {
        path: staged_binary.display().to_string(),
        source,
    })?;

    let checksum = file_checksum_hex(&source_executable)?;
    let manifest = format!(
        "build_id = \"{build_id}\"\nsource_executable = \"{}\"\ncreated_at = {}\n",
        source_executable.display(),
        now_unix_seconds()
    );
    write_file(&staged_dir.join("manifest.toml"), manifest.as_bytes())?;
    write_file(
        &staged_dir.join("checksums.txt"),
        format!("{checksum}  bin/fin\n").as_bytes(),
    )?;
    append_log(
        log_path,
        &format!(
            "[stage] build_id={build_id} source={} staged={}\n",
            source_executable.display(),
            staged_binary.display()
        ),
    )?;
    Ok(staged_dir)
}

fn run_installed_smoke(
    binary_path: &Path,
    config_path: &Path,
    smoke_home: &Path,
    runtime_home: &Path,
    build_id: &str,
    log_path: &Path,
) -> Result<(), CliError> {
    let commands: [(&str, Vec<String>); 3] = [
        (
            "config-check",
            vec![
                binary_path.display().to_string(),
                "config-check".into(),
                config_path.display().to_string(),
            ],
        ),
        (
            "runtime-demo",
            vec![
                binary_path.display().to_string(),
                "runtime-demo".into(),
                config_path.display().to_string(),
                format!("installed smoke {build_id}"),
            ],
        ),
        (
            "debug-projection",
            vec![
                binary_path.display().to_string(),
                "debug-projection".into(),
                config_path.display().to_string(),
                format!("observable smoke {build_id}"),
            ],
        ),
    ];

    for (label, argv) in commands {
        let mut command = ProcessCommand::new(&argv[0]);
        command.args(&argv[1..]).env("HOME", smoke_home);
        run_process_and_log(command, label, log_path)?;
    }

    for path in [
        runtime_home.join("runtime/current/last_run.json"),
        runtime_home.join("runtime/projections/current_snapshot.json"),
        runtime_home.join("runtime/projections/current_projection.json"),
    ] {
        if !path.exists() {
            return Err(CliError::MissingInstallTarget(path.display().to_string()));
        }
    }

    let report_dir = runtime_home.join("harness/reports").join(build_id);
    fs::create_dir_all(&report_dir).map_err(|source| CliError::WriteFile {
        path: report_dir.display().to_string(),
        source,
    })?;
    write_file(
        &report_dir.join("summary.json"),
        serde_json::to_vec_pretty(&json!({
            "build_id": build_id,
            "smoke_home": smoke_home.display().to_string(),
            "binary": binary_path.display().to_string(),
            "verified_paths": [
                "runtime/current/last_run.json",
                "runtime/projections/current_snapshot.json",
                "runtime/projections/current_projection.json"
            ]
        }))?
        .as_slice(),
    )?;
    Ok(())
}

fn promote_build(
    runtime_home: &Path,
    build_id: &str,
    log_path: &Path,
) -> Result<PathBuf, CliError> {
    let staged_dir = runtime_home.join("install/staged").join(build_id);
    if !staged_dir.exists() {
        return Err(CliError::MissingInstallTarget(
            staged_dir.display().to_string(),
        ));
    }
    let version_dir = runtime_home.join("install/versions").join(build_id);
    if version_dir.exists() {
        return Err(CliError::InvalidInstallState(format!(
            "version already exists: {}",
            version_dir.display()
        )));
    }
    fs::rename(&staged_dir, &version_dir).map_err(|source| CliError::WriteFile {
        path: format!("{} -> {}", staged_dir.display(), version_dir.display()),
        source,
    })?;

    let current_link = runtime_home.join("install/current");
    let previous_link = runtime_home.join("install/previous");
    let old_current = fs::read_link(&current_link).ok();
    if let Some(old) = &old_current {
        replace_symlink(&previous_link, old)?;
    }
    replace_symlink(&current_link, &version_dir)?;
    replace_symlink(
        &runtime_home.join("bin/fin"),
        &runtime_home.join("install/current/bin/fin"),
    )?;
    write_receipt(
        runtime_home,
        build_id,
        &json!({
            "action": "promote",
            "build_id": build_id,
            "current_version": build_id,
            "previous_version": old_current
                .as_ref()
                .and_then(|path| path.file_name())
                .and_then(|name| name.to_str()),
            "timestamp": now_unix_seconds(),
        }),
    )?;
    append_log(
        log_path,
        &format!(
            "[promote] build_id={build_id} current={} previous={}\n",
            version_dir.display(),
            old_current
                .as_ref()
                .map(|p| p.display().to_string())
                .unwrap_or_else(|| "-".into())
        ),
    )?;
    Ok(version_dir)
}

fn run_post_install_smoke(
    runtime_home: &Path,
    config_path: &Path,
    smoke_home: &Path,
    build_id: &str,
    log_path: &Path,
) -> Result<(), CliError> {
    let bin_link = runtime_home.join("bin/fin");
    if !bin_link.exists() {
        return Err(CliError::MissingInstallTarget(
            bin_link.display().to_string(),
        ));
    }
    let mut command = ProcessCommand::new(&bin_link);
    command
        .arg("config-check")
        .arg(config_path)
        .env("HOME", smoke_home);
    run_process_and_log(command, "post-install config-check", log_path)?;
    write_file(
        &runtime_home.join("runtime/current/install_state.json"),
        serde_json::to_vec_pretty(&json!({
            "build_id": build_id,
            "bin": bin_link.display().to_string(),
            "timestamp": now_unix_seconds(),
        }))?
        .as_slice(),
    )?;
    Ok(())
}

fn run_process_and_log(
    mut command: ProcessCommand,
    label: &str,
    log_path: &Path,
) -> Result<(), CliError> {
    append_log(log_path, &format!("$ [{}] {:?}\n", label, command))?;
    let output = command.output().map_err(|source| CliError::ReadFile {
        path: format!("process: {label}"),
        source,
    })?;
    append_log(log_path, &String::from_utf8_lossy(&output.stdout))?;
    append_log(log_path, &String::from_utf8_lossy(&output.stderr))?;
    if !output.status.success() {
        return Err(CliError::ProcessFailed {
            command: label.into(),
            exit_code: output.status.code(),
        });
    }
    Ok(())
}

fn append_log(path: &Path, line: &str) -> Result<(), CliError> {
    use std::io::Write;

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|source| CliError::WriteFile {
            path: parent.display().to_string(),
            source,
        })?;
    }
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|source| CliError::WriteFile {
            path: path.display().to_string(),
            source,
        })?;
    file.write_all(line.as_bytes())
        .map_err(|source| CliError::WriteFile {
            path: path.display().to_string(),
            source,
        })
}

fn file_checksum_hex(path: &Path) -> Result<String, CliError> {
    let bytes = fs::read(path).map_err(|source| CliError::ReadFile {
        path: path.display().to_string(),
        source,
    })?;
    let checksum = bytes.iter().fold(0_u64, |acc, byte| {
        acc.wrapping_mul(16777619) ^ u64::from(*byte)
    });
    Ok(format!("{checksum:016x}"))
}

fn write_receipt(
    runtime_home: &Path,
    name: &str,
    payload: &serde_json::Value,
) -> Result<(), CliError> {
    write_file(
        &runtime_home
            .join("install/receipts")
            .join(format!("{name}.json")),
        serde_json::to_vec_pretty(payload)?.as_slice(),
    )
}

fn build_id_from_link(link_target: &Path) -> Result<String, CliError> {
    link_target
        .file_name()
        .and_then(|name| name.to_str())
        .map(str::to_string)
        .ok_or_else(|| CliError::InvalidInstallState(link_target.display().to_string()))
}

#[cfg(unix)]
fn replace_symlink(link_path: &Path, target: &Path) -> Result<(), CliError> {
    if link_path.exists() || fs::symlink_metadata(link_path).is_ok() {
        let meta = fs::symlink_metadata(link_path).map_err(|source| CliError::ReadFile {
            path: link_path.display().to_string(),
            source,
        })?;
        if meta.is_dir() && !meta.file_type().is_symlink() {
            fs::remove_dir_all(link_path).map_err(|source| CliError::WriteFile {
                path: link_path.display().to_string(),
                source,
            })?;
        } else {
            fs::remove_file(link_path).map_err(|source| CliError::WriteFile {
                path: link_path.display().to_string(),
                source,
            })?;
        }
    }
    if let Some(parent) = link_path.parent() {
        fs::create_dir_all(parent).map_err(|source| CliError::WriteFile {
            path: parent.display().to_string(),
            source,
        })?;
    }
    unix_fs::symlink(target, link_path).map_err(|source| CliError::WriteFile {
        path: format!("{} -> {}", link_path.display(), target.display()),
        source,
    })
}

fn persist_runtime_demo(
    user_toml: &str,
    system: &SystemConfig,
    run: &fin_runtime::ClosureRun,
    override_path: Option<&Path>,
) -> Result<RuntimeHomeArtifacts, CliError> {
    let runtime_home = init_runtime_home(user_toml, system, override_path)?;
    let session_id = run
        .progress
        .refs
        .session_id
        .clone()
        .unwrap_or_else(|| "session-m1".into());
    let created_at = &run.note.created_at;
    let year = created_at.get(0..4).unwrap_or("unknown");
    let month = created_at.get(5..7).unwrap_or("00");
    let session_dir = runtime_home
        .join("sessions")
        .join(year)
        .join(month)
        .join(&session_id);
    for relative in [
        "events",
        "progress",
        "notes",
        "digests",
        "context",
        "collab",
        "tasks",
        "topics",
        "artifacts/candidates",
    ] {
        fs::create_dir_all(session_dir.join(relative)).map_err(|source| CliError::WriteFile {
            path: session_dir.join(relative).display().to_string(),
            source,
        })?;
    }

    write_json_lines(&session_dir.join("events/stream.jsonl"), &run.events)?;
    write_file(
        &session_dir.join("progress/latest.json"),
        serde_json::to_vec_pretty(&run.progress)?.as_slice(),
    )?;
    write_file(
        &session_dir.join("notes/latest.json"),
        serde_json::to_vec_pretty(&run.note)?.as_slice(),
    )?;
    write_file(
        &session_dir.join("digests/latest.json"),
        serde_json::to_vec_pretty(&run.digest)?.as_slice(),
    )?;

    let snapshot_paths = persist_snapshot(&runtime_home.join("runtime/projections"), &run.events)?;
    write_file(
        &runtime_home.join("runtime/current/last_run.json"),
        serde_json::to_vec_pretty(&serde_json::json!({
            "session_id": session_id,
            "task_id": run.progress.refs.task_id,
            "digest_id": run.digest.digest_id,
            "provider": run.prepared_request.provider_name,
            "model": run.prepared_request.model,
        }))?
        .as_slice(),
    )?;

    Ok(RuntimeHomeArtifacts {
        runtime_home,
        projection_json: snapshot_paths.projection_json,
        snapshot_json: snapshot_paths.snapshot_json,
        session_dir,
    })
}

fn write_file(path: &Path, bytes: &[u8]) -> Result<(), CliError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|source| CliError::WriteFile {
            path: parent.display().to_string(),
            source,
        })?;
    }
    fs::write(path, bytes).map_err(|source| CliError::WriteFile {
        path: path.display().to_string(),
        source,
    })
}

fn write_json_lines<T: serde::Serialize>(path: &Path, values: &[T]) -> Result<(), CliError> {
    let mut content = String::new();
    for value in values {
        content.push_str(&serde_json::to_string(value)?);
        content.push('\n');
    }
    write_file(path, content.as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn sample_user_toml() -> String {
        r#"
default_provider = "openai"

[providers.openai]
protocol = "open-ai-compatible"
base_url = "https://api.example.com/v1"
model = "gpt-5"
api_key_env = "OPENAI_API_KEY"
"#
        .into()
    }

    fn write_temp_user_config() -> String {
        let path = std::env::temp_dir().join(format!(
            "fin-cli-test-{}.toml",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("time should work")
                .as_nanos()
        ));
        fs::write(&path, sample_user_toml()).expect("temp config should write");
        path.display().to_string()
    }

    fn temp_runtime_home() -> PathBuf {
        std::env::temp_dir().join(format!(
            "fin-runtime-home-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("time should work")
                .as_nanos()
        ))
    }

    #[test]
    fn parse_command_accepts_home_init() {
        let args = vec!["home-init".into(), "/tmp/user.toml".into()];
        assert_eq!(
            parse_command(&args).expect("command should parse"),
            Command::HomeInit {
                path: "/tmp/user.toml".into(),
            }
        );
    }

    #[test]
    fn parse_command_accepts_web_debug_default_port() {
        let args = vec!["web-debug".into(), "/tmp/user.toml".into()];
        assert_eq!(
            parse_command(&args).expect("command should parse"),
            Command::WebDebug {
                path: "/tmp/user.toml".into(),
                port: 4040,
            }
        );
    }

    #[test]
    fn load_system_config_maps_user_config() {
        let path = write_temp_user_config();
        let system = load_system_config(Path::new(&path)).expect("system config should load");
        assert_eq!(system.default_provider, "openai");
    }

    #[test]
    fn runtime_demo_persists_home_artifacts() {
        let path = write_temp_user_config();
        let home = temp_runtime_home();
        run_with_runtime_home(
            vec!["runtime-demo".into(), path, "hello".into()],
            Some(home.clone()),
        )
        .expect("runtime demo should run");

        assert!(home.join("config/user.toml").exists());
        assert!(
            home.join("runtime/projections/current_projection.json")
                .exists()
        );
        assert!(
            home.join("sessions/2026/04/session-cli-demo/events/stream.jsonl")
                .exists()
        );
    }

    #[test]
    fn demo_identity_uses_test_namespace_when_provided() {
        let ids = demo_identity(Some("test-provider-smoke"));
        assert_eq!(ids.session_id, "session-test-provider-smoke");
        assert_eq!(ids.task_id, "task-test-provider-smoke");
        assert_eq!(ids.operation_id, "op-test-provider-smoke");
    }

    #[test]
    fn sanitize_id_fragment_normalizes_non_identifier_chars() {
        assert_eq!(
            sanitize_id_fragment(" test/provider smoke "),
            "test-provider-smoke"
        );
    }

    #[test]
    fn debug_projection_command_runs() {
        let path = write_temp_user_config();
        let home = temp_runtime_home();
        run_with_runtime_home(
            vec!["debug-projection".into(), path, "hello".into()],
            Some(home.clone()),
        )
        .expect("debug projection should run");
        assert!(
            home.join("runtime/projections/current_snapshot.json")
                .exists()
        );
    }
}
