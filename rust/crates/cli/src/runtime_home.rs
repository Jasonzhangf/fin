use crate::{
    CliError,
    fs_utils::{read_file, write_file},
};
use fin_config::{SystemConfig, system_to_toml};
use fin_contracts::{DigestRecord, ReasoningViewRecord, ToolExecutionRecord};
use fin_debug_server::persist_snapshot;
use fin_runtime::{ClosureRun, SessionMaterializer};
use fin_shared::expand_home_path;
use serde::de::DeserializeOwned;
use std::{
    fs,
    path::{Path, PathBuf},
};

pub(crate) use fin_runtime::SessionMessageRecord;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RuntimeHomeArtifacts {
    pub(crate) runtime_home: PathBuf,
    pub(crate) projection_json: PathBuf,
    pub(crate) snapshot_json: PathBuf,
    pub(crate) session_dir: PathBuf,
}

pub(crate) fn resolved_runtime_home(
    system: &SystemConfig,
    override_path: Option<&Path>,
) -> PathBuf {
    override_path
        .map(Path::to_path_buf)
        .unwrap_or_else(|| expand_home_path(&system.runtime.runtime_home))
}

pub(crate) fn init_runtime_home(
    user_toml: &str,
    system: &SystemConfig,
    override_path: Option<&Path>,
) -> Result<PathBuf, CliError> {
    let runtime_home = resolved_runtime_home(system, override_path);
    ensure_runtime_home_layout(&runtime_home)?;

    let config_dir = runtime_home.join("config");
    let system_toml = system_to_toml(system)?;
    write_file(&config_dir.join("user.toml"), user_toml.as_bytes())?;
    write_file(&config_dir.join("system.toml"), system_toml.as_bytes())?;
    write_file(
        &config_dir.join("system.template.toml"),
        system_toml.as_bytes(),
    )?;

    Ok(runtime_home)
}

pub(crate) fn ensure_runtime_home_layout(runtime_home: &Path) -> Result<(), CliError> {
    for relative in [
        "config",
        "skills",
        "bin",
        "install/staged",
        "install/versions",
        "install/receipts",
        "runtime/locks",
        "runtime/pids",
        "runtime/sockets",
        "runtime/leases",
        "runtime/heartbeats",
        "runtime/reminders",
        "runtime/peers",
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

pub(crate) fn persist_runtime_demo(
    user_toml: &str,
    system: &SystemConfig,
    run: &ClosureRun,
    override_path: Option<&Path>,
) -> Result<RuntimeHomeArtifacts, CliError> {
    let runtime_home = init_runtime_home(user_toml, system, override_path)?;
    let receipt = SessionMaterializer::default().persist(&runtime_home, run)?;

    let snapshot_paths = persist_snapshot(&runtime_home.join("runtime/projections"), &run.events)?;

    Ok(RuntimeHomeArtifacts {
        runtime_home,
        projection_json: snapshot_paths.projection_json,
        snapshot_json: snapshot_paths.snapshot_json,
        session_dir: receipt.session_dir,
    })
}

pub(crate) fn read_recent_digests(path: &Path) -> Result<Vec<DigestRecord>, CliError> {
    read_json_or_empty(path)
}

pub(crate) fn read_session_messages(path: &Path) -> Result<Vec<SessionMessageRecord>, CliError> {
    read_json_or_empty(path)
}

pub(crate) fn read_recent_reasoning_views(
    path: &Path,
) -> Result<Vec<ReasoningViewRecord>, CliError> {
    read_json_or_empty(path)
}

pub(crate) fn read_recent_tool_records(path: &Path) -> Result<Vec<ToolExecutionRecord>, CliError> {
    read_json_or_empty(path)
}

fn read_json_or_empty<T: DeserializeOwned>(path: &Path) -> Result<Vec<T>, CliError> {
    match fs::read_to_string(path) {
        Ok(content) => Ok(serde_json::from_str(&content)?),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(source) => Err(CliError::ReadFile {
            path: path.display().to_string(),
            source,
        }),
    }
}

pub(crate) fn read_last_run_value(runtime_home: &Path) -> Result<serde_json::Value, CliError> {
    serde_json::from_str(&read_file(
        &runtime_home.join("runtime/current/last_run.json"),
    )?)
    .map_err(CliError::Serialize)
}
