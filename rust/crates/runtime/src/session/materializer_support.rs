use crate::RuntimeError;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(super) struct PendingReminderRecord {
    pub(super) reminder_id: String,
    #[serde(default)]
    pub(super) session_id: Option<String>,
    #[serde(default)]
    pub(super) task_id: Option<String>,
    #[serde(default)]
    pub(super) operation_id: Option<String>,
    #[serde(default)]
    pub(super) trace_id: Option<String>,
    pub(super) wait_minutes: u64,
    pub(super) reminder: String,
    pub(super) wake_role: String,
    pub(super) scheduled_at: String,
    #[serde(default)]
    pub(super) status: String,
    #[serde(default)]
    pub(super) fired_at: Option<String>,
}

pub(super) fn parse_scheduled_reminder_payload(payload: &Value) -> Option<PendingReminderRecord> {
    let object = payload.as_object()?;
    let reminder_id = object.get("reminder_id")?.as_str()?.trim().to_string();
    if reminder_id.is_empty() {
        return None;
    }
    let reminder = object.get("reminder")?.as_str()?.trim().to_string();
    if reminder.is_empty() {
        return None;
    }
    let wait_minutes = object.get("wait_minutes")?.as_u64()?;
    if wait_minutes == 0 {
        return None;
    }
    let scheduled_at = object.get("scheduled_at")?.as_str()?.trim().to_string();
    if scheduled_at.is_empty() {
        return None;
    }
    Some(PendingReminderRecord {
        reminder_id,
        session_id: object
            .get("session_id")
            .and_then(Value::as_str)
            .map(str::to_string),
        task_id: object
            .get("task_id")
            .and_then(Value::as_str)
            .map(str::to_string),
        operation_id: object
            .get("operation_id")
            .and_then(Value::as_str)
            .map(str::to_string),
        trace_id: object
            .get("trace_id")
            .and_then(Value::as_str)
            .map(str::to_string),
        wait_minutes,
        reminder,
        wake_role: object
            .get("wake_role")
            .and_then(Value::as_str)
            .filter(|value| !value.trim().is_empty())
            .unwrap_or("system")
            .to_string(),
        scheduled_at,
        status: "pending".into(),
        fired_at: None,
    })
}

pub(crate) fn trim_head<T>(items: &mut Vec<T>, limit: usize) {
    if items.len() > limit {
        let drain_count = items.len() - limit;
        items.drain(0..drain_count);
    }
}

pub(crate) fn read_json_or_empty<T: for<'de> Deserialize<'de>>(
    path: &Path,
) -> Result<Vec<T>, RuntimeError> {
    match fs::read_to_string(path) {
        Ok(content) => Ok(serde_json::from_str(&content)?),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(source) => Err(RuntimeError::Io {
            path: path.display().to_string(),
            source,
        }),
    }
}

pub(crate) fn write_json_file<T: Serialize + ?Sized>(
    path: &Path,
    value: &T,
) -> Result<(), RuntimeError> {
    write_bytes(path, serde_json::to_vec_pretty(value)?.as_slice())
}

pub(crate) fn write_bytes(path: &Path, bytes: &[u8]) -> Result<(), RuntimeError> {
    fs::write(path, bytes).map_err(|source| RuntimeError::Io {
        path: path.display().to_string(),
        source,
    })
}

pub(crate) fn create_dir_all(path: &Path) -> Result<(), RuntimeError> {
    fs::create_dir_all(path).map_err(|source| RuntimeError::Io {
        path: path.display().to_string(),
        source,
    })
}

pub(super) fn parse_turn_index(operation_id: &str) -> Option<u64> {
    operation_id
        .rsplit_once('-')
        .and_then(|(_, tail)| tail.parse::<u64>().ok())
}
