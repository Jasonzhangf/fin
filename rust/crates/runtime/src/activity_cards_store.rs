use crate::RuntimeError;
use serde::Deserialize;
use serde_json::Value;
use std::{
    fs, io,
    path::{Path, PathBuf},
};

pub(super) fn read_last_run_json(runtime_home: &Path) -> Result<Value, RuntimeError> {
    read_json_required(&runtime_home.join("runtime/current/last_run.json"))
}

fn last_run_artifact_path(
    runtime_home: &Path,
    field: &str,
) -> Result<Option<PathBuf>, RuntimeError> {
    let last_run = read_last_run_json(runtime_home)?;
    Ok(last_run
        .get(field)
        .and_then(Value::as_str)
        .map(|relative| runtime_home.join(relative)))
}

pub(super) fn read_last_run_vec<T: for<'de> Deserialize<'de>>(
    runtime_home: &Path,
    field: &str,
) -> Result<Vec<T>, RuntimeError> {
    let Some(path) = last_run_artifact_path(runtime_home, field)? else {
        return Ok(Vec::new());
    };
    read_json_if_exists(&path).map(|value| value.unwrap_or_default())
}

pub(super) fn read_last_run_value<T: for<'de> Deserialize<'de>>(
    runtime_home: &Path,
    field: &str,
) -> Result<Option<T>, RuntimeError> {
    let Some(path) = last_run_artifact_path(runtime_home, field)? else {
        return Ok(None);
    };
    read_json_if_exists(&path)
}

fn read_json_required<T: for<'de> Deserialize<'de>>(path: &Path) -> Result<T, RuntimeError> {
    let body = fs::read_to_string(path).map_err(|source| RuntimeError::Io {
        path: path.display().to_string(),
        source,
    })?;
    serde_json::from_str(&body).map_err(RuntimeError::Serialize)
}

pub(super) fn read_json_if_exists<T: for<'de> Deserialize<'de>>(
    path: &Path,
) -> Result<Option<T>, RuntimeError> {
    match fs::read_to_string(path) {
        Ok(body) => serde_json::from_str(&body)
            .map(Some)
            .map_err(RuntimeError::Serialize),
        Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(source) => Err(RuntimeError::Io {
            path: path.display().to_string(),
            source,
        }),
    }
}

pub(super) fn pending_inbound_notice(
    runtime_home: &Path,
    session_id: Option<&str>,
) -> Result<Option<String>, RuntimeError> {
    let Some(session_id) = session_id else {
        return Ok(None);
    };
    let registry = read_json_if_exists::<super::ChannelConversationRegistry>(
        &runtime_home.join("runtime/channels/qqbot/conversations.json"),
    )?
    .unwrap_or_default();
    let pending = registry.conversations.iter().any(|record| {
        record.session_id.as_deref() == Some(session_id)
            && record.status == "bound"
            && match (
                record.last_inbound_at.as_deref(),
                record.last_delivery_at.as_deref(),
            ) {
                (Some(inbound_at), Some(delivery_at)) => inbound_at > delivery_at,
                (Some(_), None) => true,
                _ => false,
            }
    });
    Ok(pending.then(|| super::PENDING_INBOUND_NOTICE.into()))
}

pub(super) fn string_field(value: &Value, key: &str) -> Option<String> {
    value.get(key).and_then(Value::as_str).map(str::to_string)
}
