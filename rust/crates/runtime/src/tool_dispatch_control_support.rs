use serde_json::{Value, json};
use std::{
    fs,
    path::{Path, PathBuf},
};

pub(super) fn extract_plan_steps(arguments: &Value) -> Option<Vec<Value>> {
    let steps = arguments.as_object()?.get("steps")?.as_array()?;
    let normalized = steps
        .iter()
        .filter_map(|item| {
            let object = item.as_object()?;
            let step = object.get("step").and_then(Value::as_str)?.trim();
            let status = object.get("status").and_then(Value::as_str)?.trim();
            if step.is_empty() || status.is_empty() {
                return None;
            }
            Some(json!({ "step": step, "status": status }))
        })
        .collect::<Vec<_>>();
    (!normalized.is_empty()).then_some(normalized)
}

pub(super) fn write_json(path: &Path, value: &Value) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|err| err.to_string())?;
    }
    fs::write(
        path,
        serde_json::to_vec_pretty(value).map_err(|err| err.to_string())?,
    )
    .map_err(|err| err.to_string())
}

pub(super) fn relative_artifact(runtime_home: &Path, path: &Path) -> String {
    path.strip_prefix(runtime_home)
        .map(|relative| relative.display().to_string())
        .unwrap_or_else(|_| path.display().to_string())
}

pub(super) fn session_dir_for_refs(
    runtime_home: &Path,
    session_id: Option<&str>,
) -> Option<PathBuf> {
    let session_id = session_id?.trim();
    if session_id.is_empty() {
        return None;
    }
    let sessions_root = runtime_home.join("sessions");
    let years = fs::read_dir(&sessions_root).ok()?;
    for year in years.flatten() {
        let year_path = year.path();
        if !year_path.is_dir() {
            continue;
        }
        for month in fs::read_dir(&year_path).ok()?.flatten() {
            let month_path = month.path();
            if !month_path.is_dir() {
                continue;
            }
            let candidate = month_path.join(session_id);
            if candidate.is_dir() {
                return Some(candidate);
            }
        }
    }
    None
}

pub(super) fn list_sessions(runtime_home: &Path, limit: usize) -> Result<Vec<String>, String> {
    let sessions_root = runtime_home.join("sessions");
    let mut sessions = Vec::new();
    if !sessions_root.exists() {
        return Ok(sessions);
    }
    for year in fs::read_dir(&sessions_root).map_err(|err| err.to_string())? {
        let year = year.map_err(|err| err.to_string())?;
        let year_path = year.path();
        if !year_path.is_dir() {
            continue;
        }
        for month in fs::read_dir(&year_path).map_err(|err| err.to_string())? {
            let month = month.map_err(|err| err.to_string())?;
            let month_path = month.path();
            if !month_path.is_dir() {
                continue;
            }
            for session in fs::read_dir(&month_path).map_err(|err| err.to_string())? {
                let session = session.map_err(|err| err.to_string())?;
                if session.path().is_dir() {
                    sessions.push(session.file_name().to_string_lossy().to_string());
                }
            }
        }
    }
    sessions.sort();
    sessions.reverse();
    sessions.truncate(limit);
    Ok(sessions)
}
