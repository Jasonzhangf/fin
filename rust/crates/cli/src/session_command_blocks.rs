use crate::{
    CliError, session_ledger_read::list_sessions_from_ledger, shared_io::shared_write_json,
};
use serde_json::json;
use std::{fs, path::Path};

#[derive(Debug, Clone)]
pub(crate) struct SessionListItem {
    pub(crate) session_id: String,
    pub(crate) updated_at: String,
    pub(crate) title: Option<String>,
    pub(crate) archived: bool,
    pub(crate) preview_100: Option<String>,
    pub(crate) task_id: Option<String>,
}

pub(crate) fn list_sessions(runtime_home: &Path) -> Result<Vec<SessionListItem>, CliError> {
    let mut items = list_sessions_from_ledger(runtime_home)?
        .into_iter()
        .map(|item| SessionListItem {
            session_id: item.session_id,
            updated_at: item.updated_at,
            title: item.title,
            archived: item.archived,
            preview_100: item.preview.map(|value| trim_preview(&value, 100)),
            task_id: item.task_id,
        })
        .collect::<Vec<_>>();
    items.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
    Ok(items)
}

pub(crate) fn session_meta_path(runtime_home: &Path, session_id: &str) -> std::path::PathBuf {
    runtime_home
        .join("sessions")
        .join("meta")
        .join(format!("{session_id}.json"))
}

pub(crate) fn read_session_meta(
    runtime_home: &Path,
    session_id: &str,
) -> Option<serde_json::Value> {
    let path = session_meta_path(runtime_home, session_id);
    fs::read_to_string(path)
        .ok()
        .and_then(|content| serde_json::from_str::<serde_json::Value>(&content).ok())
}

pub(crate) fn set_session_meta(
    runtime_home: &Path,
    session_id: &str,
    title: Option<&str>,
    archived: Option<bool>,
) -> Result<(), CliError> {
    let path = session_meta_path(runtime_home, session_id);
    let mut value = read_session_meta(runtime_home, session_id)
        .unwrap_or_else(|| json!({"session_id":session_id}));
    if let Some(title) = title {
        value["title"] = json!(title);
    }
    if let Some(archived) = archived {
        value["archived"] = json!(archived);
    }
    shared_write_json(&path, &value)
}

pub(crate) fn delete_session_dir(
    runtime_home: &Path,
    session_id: &str,
    find_session_dir: impl Fn(&Path, &str) -> Option<(i32, u32, std::path::PathBuf)>,
) -> Result<(), CliError> {
    let Some((_year, _month, session_dir)) = find_session_dir(runtime_home, session_id) else {
        return Ok(());
    };
    fs::remove_dir_all(&session_dir).map_err(|source| CliError::WriteFile {
        path: session_dir.display().to_string(),
        source,
    })?;
    let meta = session_meta_path(runtime_home, session_id);
    let _ = fs::remove_file(meta);
    Ok(())
}

fn trim_preview(value: &str, max_chars: usize) -> String {
    let text = value.trim();
    if text.chars().count() <= max_chars {
        return text.to_string();
    }
    text.chars().take(max_chars).collect::<String>() + "…"
}
