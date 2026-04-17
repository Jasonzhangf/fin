use crate::{
    CliError,
    fs_utils::{read_file, write_file, write_json_lines},
};
use fin_config::{SystemConfig, system_to_toml};
use fin_contracts::{ContextSnapshotRecord, DigestRecord};
use fin_debug_server::persist_snapshot;
use fin_runtime::ClosureRun;
use fin_shared::expand_home_path;
use serde::{Deserialize, Serialize};
use serde::de::DeserializeOwned;
use serde_json::json;
use std::{
    fs,
    path::{Path, PathBuf},
};

const RECENT_CONTEXT_LIMIT: usize = 8;
const RECENT_DIGEST_LIMIT: usize = 8;
const SESSION_MESSAGE_LIMIT: usize = 128;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RuntimeHomeArtifacts {
    pub(crate) runtime_home: PathBuf,
    pub(crate) projection_json: PathBuf,
    pub(crate) snapshot_json: PathBuf,
    pub(crate) session_dir: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct SessionMessageRecord {
    pub(crate) message_id: String,
    pub(crate) role: String,
    pub(crate) content: String,
    pub(crate) created_at: String,
    pub(crate) session_id: String,
    pub(crate) task_id: Option<String>,
    #[serde(default)]
    pub(crate) operation_id: Option<String>,
    #[serde(default)]
    pub(crate) trace_id: Option<String>,
    #[serde(default)]
    pub(crate) closure_id: Option<String>,
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

pub(crate) fn persist_runtime_demo(
    user_toml: &str,
    system: &SystemConfig,
    run: &ClosureRun,
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
        "conversation",
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
    persist_recent_digests(&session_dir, &run.digest)?;
    persist_context_snapshots(&runtime_home, &session_dir, &run.context_snapshot)?;
    persist_session_messages(&session_dir, run)?;

    let snapshot_paths = persist_snapshot(&runtime_home.join("runtime/projections"), &run.events)?;
    write_file(
        &runtime_home.join("runtime/current/last_run.json"),
        serde_json::to_vec_pretty(&json!({
            "session_id": session_id,
            "task_id": run.progress.refs.task_id,
            "digest_id": run.digest.digest_id,
            "provider": run.prepared_request.provider_name,
            "model": run.prepared_request.model,
            "provider_user_agent": run.prepared_request.user_agent,
            "provider_header_names": run.prepared_request.sanitized_headers.keys().collect::<Vec<_>>(),
            "answer": run.provider_response.output_text,
            "current_context_path": "runtime/current/current_context.json",
            "session_recent_contexts_path": format!(
                "sessions/{year}/{month}/{session_id}/context/recent_contexts.json"
            ),
            "session_recent_digests_path": format!(
                "sessions/{year}/{month}/{session_id}/digests/recent_digests.json"
            ),
            "session_messages_path": format!(
                "sessions/{year}/{month}/{session_id}/conversation/messages.json"
            ),
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

fn persist_context_snapshots(
    runtime_home: &Path,
    session_dir: &Path,
    snapshot: &ContextSnapshotRecord,
) -> Result<(), CliError> {
    write_file(
        &runtime_home.join("runtime/current/current_context.json"),
        serde_json::to_vec_pretty(snapshot)?.as_slice(),
    )?;

    let recent_path = session_dir.join("context/recent_contexts.json");
    let mut recent = read_recent_contexts(&recent_path)?;
    recent.push(snapshot.clone());
    trim_head(&mut recent, RECENT_CONTEXT_LIMIT);
    write_file(&recent_path, serde_json::to_vec_pretty(&recent)?.as_slice())
}

fn persist_recent_digests(session_dir: &Path, digest: &DigestRecord) -> Result<(), CliError> {
    let recent_path = session_dir.join("digests/recent_digests.json");
    let mut recent = read_recent_digests(&recent_path)?;
    recent.push(digest.clone());
    trim_head(&mut recent, RECENT_DIGEST_LIMIT);
    write_file(&recent_path, serde_json::to_vec_pretty(&recent)?.as_slice())
}

fn persist_session_messages(session_dir: &Path, run: &ClosureRun) -> Result<(), CliError> {
    let path = session_dir.join("conversation/messages.json");
    let mut messages = read_session_messages(&path)?;
    let session_id = run
        .context_snapshot
        .refs
        .session_id
        .clone()
        .unwrap_or_else(|| "session-m1".into());
    let task_id = run.context_snapshot.refs.task_id.clone();
    messages.push(SessionMessageRecord {
        message_id: format!("user-{}", run.context_snapshot.operation_id),
        role: "user".into(),
        content: run.context_snapshot.input.clone(),
        created_at: run.context_snapshot.captured_at.clone(),
        session_id: session_id.clone(),
        task_id: task_id.clone(),
        operation_id: Some(run.context_snapshot.operation_id.clone()),
        trace_id: Some(run.context_snapshot.trace_id.clone()),
        closure_id: Some(run.digest.closure_id.clone()),
    });
    messages.push(SessionMessageRecord {
        message_id: format!("assistant-{}", run.digest.closure_id),
        role: "assistant".into(),
        content: run.provider_response.output_text.clone(),
        created_at: run.note.created_at.clone(),
        session_id,
        task_id,
        operation_id: Some(run.context_snapshot.operation_id.clone()),
        trace_id: Some(run.context_snapshot.trace_id.clone()),
        closure_id: Some(run.digest.closure_id.clone()),
    });
    trim_head(&mut messages, SESSION_MESSAGE_LIMIT);
    write_file(&path, serde_json::to_vec_pretty(&messages)?.as_slice())
}

fn trim_head<T>(items: &mut Vec<T>, limit: usize) {
    if items.len() > limit {
        let drain_count = items.len() - limit;
        items.drain(0..drain_count);
    }
}

pub(crate) fn read_recent_contexts(path: &Path) -> Result<Vec<ContextSnapshotRecord>, CliError> {
    read_json_or_empty(path)
}

pub(crate) fn read_recent_digests(path: &Path) -> Result<Vec<DigestRecord>, CliError> {
    read_json_or_empty(path)
}

pub(crate) fn read_session_messages(path: &Path) -> Result<Vec<SessionMessageRecord>, CliError> {
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
