use crate::{
    CliError, execution_state::clear_waiting_if_due, runtime_home::SessionMessageRecord,
    time::local_timestamp_now,
};
use chrono::{DateTime, Duration, Local};
use fin_debug_server::DebugBinding;
use serde::{Deserialize, Serialize};
use std::{fs, path::Path};

const SESSION_MESSAGE_LIMIT: usize = 128;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
struct PendingReminderRecord {
    reminder_id: String,
    #[serde(default)]
    session_id: Option<String>,
    #[serde(default)]
    task_id: Option<String>,
    #[serde(default)]
    operation_id: Option<String>,
    #[serde(default)]
    trace_id: Option<String>,
    wait_minutes: u64,
    reminder: String,
    wake_role: String,
    scheduled_at: String,
    #[serde(default)]
    status: String,
    #[serde(default)]
    fired_at: Option<String>,
}

pub(crate) fn inject_due_reminders(
    runtime_home: &Path,
    binding: &DebugBinding,
) -> Result<usize, CliError> {
    let pending_path = runtime_home.join("runtime/reminders/pending.json");
    let mut reminders = read_json_or_empty::<PendingReminderRecord>(&pending_path)?;
    if reminders.is_empty() {
        return Ok(0);
    }
    let now = Local::now();
    let mut fired_count = 0usize;

    for record in &mut reminders {
        if !record.status.is_empty() && record.status != "pending" {
            continue;
        }
        if !is_due(record, now) {
            continue;
        }
        let Some(session_id) = record
            .session_id
            .clone()
            .or_else(|| binding.session_id.clone())
        else {
            continue;
        };
        let Some(messages_path) = resolve_messages_path(runtime_home, binding, &session_id) else {
            continue;
        };
        let mut messages = read_json_or_empty::<SessionMessageRecord>(&messages_path)?;
        let message_id = format!("system-reminder-{}", record.reminder_id);
        if messages
            .iter()
            .any(|message| message.message_id == message_id)
        {
            record.status = "fired".into();
            record.fired_at = Some(local_timestamp_now());
            continue;
        }
        messages.push(SessionMessageRecord {
            message_id,
            role: "system".into(),
            content: format!(
                "⏰ Reminder ({}m): {}",
                record.wait_minutes, record.reminder
            ),
            created_at: local_timestamp_now(),
            session_id: session_id.clone(),
            task_id: record.task_id.clone().or_else(|| binding.task_id.clone()),
            operation_id: record.operation_id.clone(),
            trace_id: record.trace_id.clone(),
            closure_id: None,
        });
        trim_head(&mut messages, SESSION_MESSAGE_LIMIT);
        write_json(&messages_path, &messages)?;
        record.status = "fired".into();
        record.fired_at = Some(local_timestamp_now());
        fired_count += 1;
    }

    write_json(&pending_path, &reminders)?;
    write_json(
        &runtime_home.join("runtime/current/current_reminders.json"),
        &reminders,
    )?;
    if fired_count > 0 {
        clear_waiting_if_due(runtime_home, binding, &local_timestamp_now())?;
    }
    Ok(fired_count)
}

fn resolve_messages_path(
    runtime_home: &Path,
    binding: &DebugBinding,
    session_id: &str,
) -> Option<std::path::PathBuf> {
    if let (Some(path), Some(binding_session_id)) =
        (&binding.session_messages_path, &binding.session_id)
    {
        if binding_session_id == session_id {
            return Some(runtime_home.join(path));
        }
    }
    find_session_messages_path(runtime_home, session_id)
}

fn find_session_messages_path(runtime_home: &Path, session_id: &str) -> Option<std::path::PathBuf> {
    let sessions_root = runtime_home.join("sessions");
    let years = fs::read_dir(&sessions_root).ok()?;
    for year in years.flatten() {
        let months = fs::read_dir(year.path()).ok()?;
        for month in months.flatten() {
            let candidate = month
                .path()
                .join(session_id)
                .join("conversation/messages.json");
            if candidate.exists() {
                return Some(candidate);
            }
        }
    }
    None
}

fn is_due(record: &PendingReminderRecord, now: DateTime<Local>) -> bool {
    let Ok(scheduled) = DateTime::parse_from_rfc3339(&record.scheduled_at) else {
        return false;
    };
    let due = scheduled + Duration::minutes(record.wait_minutes as i64);
    due.with_timezone(&Local) <= now
}

fn trim_head<T>(items: &mut Vec<T>, limit: usize) {
    if items.len() > limit {
        let drain_count = items.len() - limit;
        items.drain(0..drain_count);
    }
}

fn read_json_or_empty<T: for<'de> Deserialize<'de>>(path: &Path) -> Result<Vec<T>, CliError> {
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_runtime_home() -> std::path::PathBuf {
        std::env::temp_dir().join(format!(
            "fin-reminder-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("time should work")
                .as_nanos()
        ))
    }

    #[test]
    fn inject_due_reminders_appends_system_message_once() {
        let home = temp_runtime_home();
        let session_dir = home.join("sessions/2026/04/session-reminder");
        fs::create_dir_all(session_dir.join("conversation")).expect("conversation dir");
        fs::create_dir_all(home.join("runtime/reminders")).expect("reminders dir");
        write_json(
            &session_dir.join("conversation/messages.json"),
            &Vec::<SessionMessageRecord>::new(),
        )
        .expect("write messages");
        write_json(
            &home.join("runtime/reminders/pending.json"),
            &vec![PendingReminderRecord {
                reminder_id: "reminder-1".into(),
                session_id: Some("session-reminder".into()),
                task_id: Some("task-reminder".into()),
                operation_id: Some("op-reminder".into()),
                trace_id: Some("trace-reminder".into()),
                wait_minutes: 1,
                reminder: "check build log".into(),
                wake_role: "system".into(),
                scheduled_at: (Local::now() - Duration::minutes(2))
                    .format("%Y-%m-%dT%H:%M:%S%:z")
                    .to_string(),
                status: "pending".into(),
                fired_at: None,
            }],
        )
        .expect("write pending");
        let binding = DebugBinding {
            project_id: "fin".into(),
            project_label: "fin".into(),
            runtime_home: home.display().to_string(),
            session_id: Some("session-reminder".into()),
            task_id: Some("task-reminder".into()),
            session_messages_path: Some(
                "sessions/2026/04/session-reminder/conversation/messages.json".into(),
            ),
            recent_contexts_path: None,
            recent_digests_path: None,
        };

        let fired = inject_due_reminders(&home, &binding).expect("inject reminders");
        assert_eq!(fired, 1);
        let messages = read_json_or_empty::<SessionMessageRecord>(
            &session_dir.join("conversation/messages.json"),
        )
        .expect("messages");
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].role, "system");
        assert!(messages[0].content.contains("check build log"));

        let fired_again = inject_due_reminders(&home, &binding).expect("inject reminders again");
        assert_eq!(fired_again, 0);
    }
}
