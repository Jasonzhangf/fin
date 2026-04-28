use crate::{CliError, runtime_home::SessionMessageRecord, time::local_timestamp_now};
use chrono::Local;
use serde::Serialize;
use std::{fs, path::Path};

const SESSION_MESSAGE_LIMIT: usize = 128;

pub(crate) fn append_notice_messages(
    message_path: &Path,
    session_id: &str,
    task_id: Option<&str>,
    command: &str,
    notice: &str,
) -> Result<(), CliError> {
    let mut messages = read_json_or_empty::<SessionMessageRecord>(message_path)?;
    let now = local_timestamp_now();
    let stamp = Local::now().format("%Y%m%d%H%M%S").to_string();
    messages.push(SessionMessageRecord {
        message_id: format!("local-command-{stamp}"),
        role: "local_command".into(),
        content: command.to_string(),
        created_at: now.clone(),
        session_id: session_id.to_string(),
        task_id: task_id.map(str::to_string),
        operation_id: None,
        trace_id: None,
        closure_id: None,
        sender_kind: None,
        role_id: None,
        agent_name: None,
        display_name: None,
        source_kind: None,
    });
    messages.push(SessionMessageRecord {
        message_id: format!("system-notice-{stamp}"),
        role: "system".into(),
        content: notice.to_string(),
        created_at: now,
        session_id: session_id.to_string(),
        task_id: task_id.map(str::to_string),
        operation_id: None,
        trace_id: None,
        closure_id: None,
        sender_kind: None,
        role_id: None,
        agent_name: None,
        display_name: None,
        source_kind: None,
    });
    trim_head(&mut messages, SESSION_MESSAGE_LIMIT);
    write_json(message_path, &messages)
}

fn trim_head<T>(items: &mut Vec<T>, limit: usize) {
    if items.len() > limit {
        let drain_count = items.len() - limit;
        items.drain(0..drain_count);
    }
}

fn read_json_or_empty<T: for<'de> serde::Deserialize<'de>>(
    path: &Path,
) -> Result<Vec<T>, CliError> {
    match fs::read_to_string(path) {
        Ok(content) => serde_json::from_str(&content).map_err(CliError::Serialize),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(source) => Err(CliError::ReadFile {
            path: path.display().to_string(),
            source,
        }),
    }
}

fn write_json<T: Serialize>(path: &Path, value: &T) -> Result<(), CliError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|source| CliError::WriteFile {
            path: parent.display().to_string(),
            source,
        })?;
    }
    fs::write(
        path,
        serde_json::to_vec_pretty(value).map_err(CliError::Serialize)?,
    )
    .map_err(|source| CliError::WriteFile {
        path: path.display().to_string(),
        source,
    })
}
