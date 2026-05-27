use crate::{CliError, runtime_home::SessionMessageRecord, session_binding::find_session_dir};
use fin_contracts::{LedgerTrackKind, SessionSnapshotRecord};
use fin_runtime::LedgerStore;
use std::{fs, path::Path};

#[derive(Debug, Clone)]
pub(crate) struct LedgerSessionSnapshotView {
    pub(crate) session_id: String,
    pub(crate) updated_at: String,
    pub(crate) title: Option<String>,
    pub(crate) preview: Option<String>,
    pub(crate) task_id: Option<String>,
    pub(crate) archived: bool,
}

pub(crate) fn read_latest_snapshot_message(
    runtime_home: &Path,
    session_id: &str,
) -> Result<Option<SessionMessageRecord>, CliError> {
    let snapshots = read_session_snapshots(runtime_home, session_id)?;
    let Some(snapshot) = snapshots.last() else {
        return Ok(None);
    };
    Ok(Some(SessionMessageRecord {
        message_id: format!("assistant-ledger-{}", snapshot.snapshot_id),
        role: "assistant".into(),
        content: snapshot.assistant_summary.clone(),
        created_at: snapshot.created_at.clone(),
        session_id: session_id.to_string(),
        task_id: snapshot.refs.task_id.clone(),
        operation_id: Some(snapshot.operation_id.clone()),
        trace_id: Some(snapshot.trace_id.clone()),
        closure_id: None,
    }))
}

pub(crate) fn read_session_snapshots(
    runtime_home: &Path,
    session_id: &str,
) -> Result<Vec<SessionSnapshotRecord>, CliError> {
    let ledger = LedgerStore::for_session(runtime_home, session_id).map_err(runtime_err)?;
    let records = ledger
        .read_track(LedgerTrackKind::SessionSnapshot)
        .map_err(runtime_err)?;
    let mut snapshots = records
        .into_iter()
        .filter_map(|record| serde_json::from_value::<SessionSnapshotRecord>(record.payload).ok())
        .filter(|snapshot| snapshot.refs.session_id.as_deref() == Some(session_id))
        .collect::<Vec<_>>();
    snapshots.sort_by(|a, b| a.created_at.cmp(&b.created_at));
    Ok(snapshots)
}

pub(crate) fn list_sessions_from_ledger(
    runtime_home: &Path,
) -> Result<Vec<LedgerSessionSnapshotView>, CliError> {
    let root = runtime_home.join("ledgers");
    if !root.exists() {
        return Ok(Vec::new());
    }
    let mut items = Vec::new();
    for entry in fs::read_dir(&root).map_err(|source| CliError::ReadFile {
        path: root.display().to_string(),
        source,
    })? {
        let entry = match entry {
            Ok(value) => value,
            Err(_) => continue,
        };
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with("project-") || !entry.path().is_dir() {
            continue;
        }
        let session_id = name;
        let snapshots = match read_session_snapshots(runtime_home, &session_id) {
            Ok(value) => value,
            Err(_) => continue,
        };
        let Some(snapshot) = snapshots.last() else {
            continue;
        };
        let meta = read_session_meta(runtime_home, &session_id);
        items.push(LedgerSessionSnapshotView {
            session_id: session_id.clone(),
            updated_at: snapshot.created_at.clone(),
            title: meta
                .as_ref()
                .and_then(|value| {
                    value
                        .get("title")
                        .and_then(|item| item.as_str())
                        .map(str::to_string)
                })
                .or_else(|| snapshot.summary.clone()),
            preview: Some(snapshot.assistant_summary.clone()),
            task_id: snapshot.refs.task_id.clone(),
            archived: meta
                .as_ref()
                .and_then(|value| value.get("archived").and_then(|item| item.as_bool()))
                .unwrap_or(false),
        });
    }
    items.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
    Ok(items)
}

pub(crate) fn session_exists_by_ledger(runtime_home: &Path, session_id: &str) -> bool {
    runtime_home
        .join("ledgers")
        .join(session_id)
        .join("ledger.json")
        .exists()
        || find_session_dir(runtime_home, session_id).is_some()
}

fn read_session_meta(runtime_home: &Path, session_id: &str) -> Option<serde_json::Value> {
    let path = runtime_home
        .join("sessions")
        .join("meta")
        .join(format!("{session_id}.json"));
    fs::read_to_string(path)
        .ok()
        .and_then(|content| serde_json::from_str::<serde_json::Value>(&content).ok())
}

fn runtime_err(error: fin_runtime::RuntimeError) -> CliError {
    CliError::InvalidInstallState(error.to_string())
}
