use crate::DebugDataError;
use crate::session_view::read_json_value;
use fin_contracts::{LedgerTrackKind, SessionSnapshotRecord};
use fin_runtime::LedgerStore;
use serde::Serialize;
use serde_json::Value;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize)]
pub(crate) struct MobileSessionItem {
    pub(crate) session_id: String,
    pub(crate) task_id: String,
    pub(crate) topic: String,
    pub(crate) phase: String,
    pub(crate) project: String,
    pub(crate) updated_at: String,
    pub(crate) title: String,
    pub(crate) preview_100: String,
    pub(crate) archived: bool,
}

pub(crate) fn list_sessions(runtime_home: &Path) -> Vec<MobileSessionItem> {
    let mut out = Vec::new();
    let mut seen = std::collections::HashSet::new();

    // First pass: scan ledgers (sessions with ledger snapshots)
    let ledger_root = runtime_home.join("ledgers");
    if let Ok(entries) = std::fs::read_dir(ledger_root) {
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let session_id = entry.file_name().to_string_lossy().to_string();
            if session_id.starts_with("project-") || seen.contains(&session_id) {
                continue;
            }
            let ledger = match LedgerStore::for_session(runtime_home, &session_id) {
                Ok(value) => value,
                Err(_) => continue,
            };
            let records = match ledger.read_track(LedgerTrackKind::SessionSnapshot) {
                Ok(value) => value,
                Err(_) => continue,
            };
            let mut snapshots = records
                .into_iter()
                .filter_map(|record| {
                    serde_json::from_value::<SessionSnapshotRecord>(record.payload).ok()
                })
                .filter(|snapshot| snapshot.refs.session_id.as_deref() == Some(session_id.as_str()))
                .collect::<Vec<_>>();
            snapshots.sort_by(|a, b| a.created_at.cmp(&b.created_at));
            let Some(last) = snapshots.last() else {
                continue;
            };
            let meta_path = runtime_home
                .join("sessions")
                .join("meta")
                .join(format!("{session_id}.json"));
            let mut title = last.summary.clone().unwrap_or_default();
            let mut archived = false;
            if meta_path.exists() {
                if let Ok(meta) = read_json_value(&meta_path) {
                    if meta
                        .get("deleted")
                        .and_then(Value::as_bool)
                        .unwrap_or(false)
                    {
                        continue;
                    }
                    if let Some(v) = meta.get("title").and_then(Value::as_str) {
                        if !v.trim().is_empty() {
                            title = v.to_string();
                        }
                    }
                    archived = meta
                        .get("archived")
                        .and_then(Value::as_bool)
                        .unwrap_or(false);
                }
            }
            seen.insert(session_id.clone());
            out.push(MobileSessionItem {
                session_id,
                task_id: last
                    .refs
                    .task_id
                    .clone()
                    .unwrap_or_else(|| "task-unknown".into()),
                topic: "fin".into(),
                phase: "ready".into(),
                project: "fin".into(),
                updated_at: last.created_at.clone(),
                title,
                preview_100: last.assistant_summary.chars().take(100).collect(),
                archived,
            });
        }
    }

    // Second pass: scan sessions dir for sessions without ledgers
    let sessions_root = runtime_home.join("sessions");
    let Ok(years) = std::fs::read_dir(&sessions_root) else {
        return out;
    };
    for year_entry in years.flatten() {
        let year_path = year_entry.path();
        if !year_path.is_dir() {
            continue;
        }
        let year_name = year_entry.file_name().to_string_lossy().to_string();
        if year_name.len() != 4 || !year_name.chars().all(|ch| ch.is_ascii_digit()) {
            continue;
        }
        let Ok(months) = std::fs::read_dir(&year_path) else {
            continue;
        };
        for month_entry in months.flatten() {
            let month_path = month_entry.path();
            if !month_path.is_dir() {
                continue;
            }
            let month_name = month_entry.file_name().to_string_lossy().to_string();
            if month_name.len() != 2 || !month_name.chars().all(|ch| ch.is_ascii_digit()) {
                continue;
            }
            let Ok(session_entries) = std::fs::read_dir(&month_path) else {
                continue;
            };
            for session_entry in session_entries.flatten() {
                let session_path = session_entry.path();
                if !session_path.is_dir() {
                    continue;
                }
                let session_id = session_entry.file_name().to_string_lossy().to_string();
                if seen.contains(&session_id) || session_id.starts_with("project-") {
                    continue;
                }
                // Check if deleted
                let meta_path = runtime_home
                    .join("sessions")
                    .join("meta")
                    .join(format!("{session_id}.json"));
                if meta_path.exists() {
                    if let Ok(meta) = read_json_value(&meta_path) {
                        if meta
                            .get("deleted")
                            .and_then(Value::as_bool)
                            .unwrap_or(false)
                        {
                            continue;
                        }
                    }
                }
                // Read last_run.json to get session info
                let last_run_path = runtime_home.join("runtime/current/last_run.json");
                let (task_id, title_from_last_run) =
                    if let Ok(last_run) = read_json_value(&last_run_path) {
                        let tid = last_run
                            .get("session_id")
                            .and_then(Value::as_str)
                            .filter(|id| *id == session_id.as_str())
                            .map(|_| {
                                last_run
                                    .get("task_id")
                                    .and_then(Value::as_str)
                                    .unwrap_or("task-unknown")
                                    .to_string()
                            })
                            .unwrap_or_else(|| "task-unknown".into());
                        let title = last_run
                            .get("session_id")
                            .and_then(Value::as_str)
                            .filter(|id| *id == session_id.as_str())
                            .and_then(|_| last_run.get("answer"))
                            .and_then(Value::as_str)
                            .unwrap_or("")
                            .chars()
                            .take(100)
                            .collect();
                        (tid, title)
                    } else {
                        ("task-unknown".into(), String::new())
                    };
                // Use file modification time as updated_at (manual RFC3339)
                let updated_at = std::fs::metadata(&session_path)
                    .and_then(|m| m.modified())
                    .ok()
                    .and_then(|t| {
                        let secs = t.duration_since(std::time::UNIX_EPOCH).ok()?.as_secs();
                        let days = secs / 86400;
                        let remaining = secs % 86400;
                        let hours = remaining / 3600;
                        let minutes = (remaining % 3600) / 60;
                        let seconds = remaining % 60;
                        let mut y = 1970i64;
                        let mut d = days as i64;
                        loop {
                            let days_in_year = if (y % 4 == 0 && y % 100 != 0) || y % 400 == 0 {
                                366
                            } else {
                                365
                            };
                            if d < days_in_year {
                                break;
                            }
                            d -= days_in_year;
                            y += 1;
                        }
                        let mut m = 1u32;
                        let month_days = [
                            31,
                            if (y % 4 == 0 && y % 100 != 0) || y % 400 == 0 {
                                29
                            } else {
                                28
                            },
                            31,
                            30,
                            31,
                            30,
                            31,
                            31,
                            30,
                            31,
                            30,
                            31,
                        ];
                        while m < 12 && d >= month_days[(m - 1) as usize] as i64 {
                            d -= month_days[(m - 1) as usize] as i64;
                            m += 1;
                        }
                        Some(format!(
                            "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
                            y,
                            m,
                            d + 1,
                            hours,
                            minutes,
                            seconds
                        ))
                    })
                    .unwrap_or_else(|| "2026-01-01T00:00:00Z".into());
                let mut title = title_from_last_run;
                let mut archived = false;
                if meta_path.exists() {
                    if let Ok(meta) = read_json_value(&meta_path) {
                        if let Some(v) = meta.get("title").and_then(Value::as_str) {
                            if !v.trim().is_empty() {
                                title = v.to_string();
                            }
                        }
                        archived = meta
                            .get("archived")
                            .and_then(Value::as_bool)
                            .unwrap_or(false);
                    }
                }
                if title.is_empty() {
                    title = session_id.clone();
                }
                seen.insert(session_id.clone());
                out.push(MobileSessionItem {
                    session_id,
                    task_id,
                    topic: "fin".into(),
                    phase: "ready".into(),
                    project: "fin".into(),
                    updated_at,
                    title,
                    preview_100: String::new(),
                    archived,
                });
            }
        }
    }
    out
}

pub(crate) fn find_session_dir(runtime_home: &Path, session_id: &str) -> Option<PathBuf> {
    let root = runtime_home.join("sessions");
    let years = std::fs::read_dir(root).ok()?;
    for year in years.flatten() {
        let year_path = year.path();
        if !year_path.is_dir() {
            continue;
        }
        let year_name = year.file_name().to_string_lossy().to_string();
        if year_name.len() != 4 || !year_name.chars().all(|ch| ch.is_ascii_digit()) {
            continue;
        }
        let months = std::fs::read_dir(year_path).ok()?;
        for month in months.flatten() {
            let month_path = month.path();
            if !month_path.is_dir() {
                continue;
            }
            let month_name = month.file_name().to_string_lossy().to_string();
            if month_name.len() != 2 || !month_name.chars().all(|ch| ch.is_ascii_digit()) {
                continue;
            }
            let session_path = month_path.join(session_id);
            if session_path.is_dir() {
                return Some(session_path);
            }
        }
    }
    None
}

pub(crate) fn session_ids_from_value(value: &Value) -> Vec<String> {
    if let Some(items) = value.get("session_ids").and_then(Value::as_array) {
        return items
            .iter()
            .filter_map(Value::as_str)
            .map(str::to_string)
            .filter(|item| !item.trim().is_empty())
            .collect();
    }
    value
        .get("session_id")
        .and_then(Value::as_str)
        .map(|item| vec![item.to_string()])
        .unwrap_or_default()
}

pub(crate) fn session_is_visible(runtime_home: &Path, session_id: &str) -> bool {
    list_sessions(runtime_home)
        .iter()
        .any(|session| session.session_id == session_id)
}

pub(crate) fn runtime_view_messages(runtime_home: &Path) -> Result<Vec<Value>, DebugDataError> {
    let execution =
        crate::session_view::last_run_artifact_path(runtime_home, "current_execution_state_path")?
            .and_then(|p| read_json_value(&p).ok())
            .unwrap_or_else(|| serde_json::json!({}));
    let phase = execution
        .get("phase")
        .and_then(Value::as_str)
        .unwrap_or("idle");
    let mut messages = vec![
        serde_json::json!({"type":"runtime.health","status":"available","phase": phase}),
        serde_json::json!({"type":"provider.health","status":"available"}),
        serde_json::json!({"type":"runtime.workers","workers":[{"worker_id":"entry","phase":phase}]}),
        serde_json::json!({"type":"runtime.projects","projects":[{"project_id":"fin","state":"attached"}]}),
    ];
    if let Ok(cards) = fin_runtime::build_activity_cards(runtime_home) {
        messages.push(serde_json::json!({"type":"activity.cards.snapshot","snapshot":cards}));
    }
    messages.push(serde_json::json!({"type":"runtime.daemon","daemon":{"state":"running"}}));
    Ok(messages)
}
