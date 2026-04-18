use crate::DebugDataError;
use serde_json::Value;
use std::{
    fs,
    path::{Path, PathBuf},
};

pub(crate) fn read_last_run_json(runtime_home: &Path) -> Result<Value, DebugDataError> {
    let path = runtime_home.join("runtime/current/last_run.json");
    let body = fs::read_to_string(&path).map_err(|source| DebugDataError::Io {
        path: path.display().to_string(),
        source,
    })?;
    serde_json::from_str(&body).map_err(DebugDataError::Serialize)
}

pub(crate) fn last_run_artifact_path(
    runtime_home: &Path,
    field: &str,
) -> Result<Option<PathBuf>, DebugDataError> {
    let last_run = read_last_run_json(runtime_home)?;
    Ok(last_run
        .get(field)
        .and_then(Value::as_str)
        .map(|relative| runtime_home.join(relative)))
}

pub(crate) fn sibling_artifact_path(
    runtime_home: &Path,
    source_field: &str,
    source_suffix: &str,
    target_suffix: &str,
) -> Result<Option<PathBuf>, DebugDataError> {
    let Some(source_path) = last_run_artifact_path(runtime_home, source_field)? else {
        return Ok(None);
    };
    let source_text = source_path
        .strip_prefix(runtime_home)
        .ok()
        .and_then(|path| path.to_str());
    Ok(source_text.and_then(|value| {
        value
            .strip_suffix(source_suffix)
            .map(|prefix| runtime_home.join(format!("{prefix}{target_suffix}")))
    }))
}

pub(crate) fn read_json_lines(path: &Path) -> Result<Vec<Value>, DebugDataError> {
    let body = fs::read_to_string(path).map_err(|source| DebugDataError::Io {
        path: path.display().to_string(),
        source,
    })?;
    body.lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| serde_json::from_str::<Value>(line).map_err(DebugDataError::Serialize))
        .collect()
}
