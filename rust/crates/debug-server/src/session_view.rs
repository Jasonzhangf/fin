use crate::DebugDataError;
use serde_json::Value;
use std::{
    fs, io,
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
    let explicit = last_run
        .get(field)
        .and_then(Value::as_str)
        .map(|relative| runtime_home.join(relative));
    if explicit.is_some() {
        return Ok(explicit);
    }
    Ok(derive_session_artifact_path(runtime_home, &last_run, field))
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

pub(crate) fn read_json_value(path: &Path) -> Result<Value, DebugDataError> {
    let body = fs::read_to_string(path).map_err(|source| DebugDataError::Io {
        path: path.display().to_string(),
        source,
    })?;
    serde_json::from_str(&body).map_err(DebugDataError::Serialize)
}

pub(crate) fn session_event_stream_path(
    runtime_home: &Path,
) -> Result<Option<PathBuf>, DebugDataError> {
    let last_run = read_last_run_json(runtime_home)?;
    Ok(current_session_dir(runtime_home, &last_run).map(|dir| dir.join("events/stream.jsonl")))
}

pub(crate) fn session_event_archive_index_path(
    runtime_home: &Path,
) -> Result<Option<PathBuf>, DebugDataError> {
    if let Some(path) = last_run_artifact_path(runtime_home, "session_event_archive_index_path")? {
        return Ok(Some(path));
    }
    last_run_artifact_path(runtime_home, "current_event_archive_index_path")
}

pub(crate) fn read_event_archive_index(
    runtime_home: &Path,
) -> Result<Option<Value>, DebugDataError> {
    let Some(index_path) = session_event_archive_index_path(runtime_home)? else {
        return Ok(None);
    };
    let mut index = match read_json_value(&index_path)? {
        Value::Object(map) => map,
        _ => serde_json::Map::new(),
    };
    let local_dir = index
        .get("local_archive_dir")
        .and_then(Value::as_str)
        .map(|path| runtime_home.join(path));
    let cold_dir = index
        .get("cold_archive_dir")
        .and_then(Value::as_str)
        .map(|path| runtime_home.join(path));
    index.insert(
        "local_segments".into(),
        Value::Array(list_event_archive_segments(
            local_dir.as_deref(),
            runtime_home,
        )?),
    );
    index.insert(
        "cold_segments".into(),
        Value::Array(list_event_archive_segments(
            cold_dir.as_deref(),
            runtime_home,
        )?),
    );
    Ok(Some(Value::Object(index)))
}

pub(crate) fn event_archive_segment_path(
    runtime_home: &Path,
    tier: &str,
    segment: &str,
) -> Result<Option<PathBuf>, DebugDataError> {
    let segment = sanitize_segment_name(segment)?;
    let Some(index) = read_event_archive_index(runtime_home)? else {
        return Ok(None);
    };
    let base_dir = match tier {
        "local" => index
            .get("local_archive_dir")
            .and_then(Value::as_str)
            .map(|path| runtime_home.join(path)),
        "cold" => index
            .get("cold_archive_dir")
            .and_then(Value::as_str)
            .map(|path| runtime_home.join(path)),
        _ => None,
    };
    Ok(base_dir.map(|dir| dir.join(segment)))
}

pub(crate) fn request_path(path: &str) -> &str {
    path.split_once('?').map(|(head, _)| head).unwrap_or(path)
}

pub(crate) fn query_value<'a>(path: &'a str, name: &str) -> Option<&'a str> {
    let (_, query) = path.split_once('?')?;
    query.split('&').find_map(|pair| {
        let (key, value) = pair.split_once('=')?;
        (key == name).then_some(value)
    })
}

fn sanitize_segment_name(segment: &str) -> Result<&str, DebugDataError> {
    let trimmed = segment.trim();
    let valid = !trimmed.is_empty()
        && trimmed.starts_with("segment-")
        && trimmed.ends_with(".jsonl")
        && !trimmed.contains('/')
        && !trimmed.contains('\\');
    if valid {
        Ok(trimmed)
    } else {
        Err(DebugDataError::Io {
            path: format!("invalid event archive segment: {segment}"),
            source: io::Error::new(io::ErrorKind::InvalidInput, "invalid segment"),
        })
    }
}

fn list_event_archive_segments(
    dir: Option<&Path>,
    runtime_home: &Path,
) -> Result<Vec<Value>, DebugDataError> {
    let Some(dir) = dir else {
        return Ok(Vec::new());
    };
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(source) if source.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(source) => {
            return Err(DebugDataError::Io {
                path: dir.display().to_string(),
                source,
            });
        }
    };
    let mut segments = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.extension().and_then(|value| value.to_str()) == Some("jsonl"))
        .collect::<Vec<_>>();
    segments.sort();
    segments
        .into_iter()
        .map(|path| {
            let segment = path
                .file_name()
                .and_then(|value| value.to_str())
                .unwrap_or("segment-unknown.jsonl")
                .to_string();
            Ok(serde_json::json!({
                "segment": segment,
                "relative_path": relative_path(&path, runtime_home),
                "event_count": count_non_empty_lines(&path)?,
            }))
        })
        .collect()
}

fn count_non_empty_lines(path: &Path) -> Result<usize, DebugDataError> {
    let body = fs::read_to_string(path).map_err(|source| DebugDataError::Io {
        path: path.display().to_string(),
        source,
    })?;
    Ok(body.lines().filter(|line| !line.trim().is_empty()).count())
}

fn derive_session_artifact_path(
    runtime_home: &Path,
    last_run: &Value,
    field: &str,
) -> Option<PathBuf> {
    let session_dir = current_session_dir(runtime_home, last_run)?;
    let relative = match field {
        "session_messages_path" => "conversation/messages.json",
        "session_recent_contexts_path" => "context/recent_contexts.json",
        "session_recent_digests_path" => "digests/recent_digests.json",
        "session_recent_reasoning_path" => "reasoning/recent_reasoning_views.json",
        "session_recent_tool_records_path" => "tools/recent_tool_records.json",
        "session_recent_closures_path" => "closures/recent_closures.json",
        "session_recent_turns_path" => "turns/recent_turns.json",
        "session_event_archive_index_path" => "events/archive_index.json",
        _ => return None,
    };
    Some(session_dir.join(relative))
}

fn current_session_dir(runtime_home: &Path, last_run: &Value) -> Option<PathBuf> {
    if let Some(session_id) = last_run.get("session_id").and_then(Value::as_str)
        && let Some(dir) = find_session_dir(runtime_home, session_id)
    {
        return Some(dir);
    }
    last_run
        .get("session_messages_path")
        .and_then(Value::as_str)
        .and_then(|relative| relative.strip_suffix("conversation/messages.json"))
        .map(|prefix| runtime_home.join(prefix.trim_end_matches('/')))
}

fn find_session_dir(runtime_home: &Path, session_id: &str) -> Option<PathBuf> {
    let root = runtime_home.join("sessions");
    let years = fs::read_dir(root).ok()?;
    for year in years.flatten() {
        let year_path = year.path();
        if !year_path.is_dir() {
            continue;
        }
        let year_name = year.file_name().to_string_lossy().to_string();
        if year_name.len() != 4 || !year_name.chars().all(|ch| ch.is_ascii_digit()) {
            continue;
        }
        let months = fs::read_dir(year_path).ok()?;
        for month in months.flatten() {
            let month_path = month.path();
            if !month_path.is_dir() {
                continue;
            }
            let month_name = month.file_name().to_string_lossy().to_string();
            if month_name.len() != 2 || !month_name.chars().all(|ch| ch.is_ascii_digit()) {
                continue;
            }
            let dir = month_path.join(session_id);
            if dir.is_dir() {
                return Some(dir);
            }
        }
    }
    None
}

fn relative_path(path: &Path, runtime_home: &Path) -> String {
    path.strip_prefix(runtime_home)
        .ok()
        .and_then(|value| value.to_str())
        .map(str::to_string)
        .unwrap_or_else(|| path.display().to_string())
}
