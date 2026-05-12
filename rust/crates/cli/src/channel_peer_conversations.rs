use crate::{
    CliError,
    runtime_home::{SessionMessageRecord, read_session_messages},
    session_binding::find_session_dir,
    time::local_timestamp_now,
};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
};

const QQBOT_CHANNEL_ID: &str = "qqbot";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct ChannelConversationRecord {
    pub(crate) conversation_id: String,
    pub(crate) channel_id: String,
    pub(crate) target: String,
    #[serde(default)]
    pub(crate) session_id: Option<String>,
    pub(crate) status: String,
    pub(crate) created_at: String,
    pub(crate) updated_at: String,
    #[serde(default)]
    pub(crate) last_seen_at: Option<String>,
    #[serde(default)]
    pub(crate) last_inbound_message_id: Option<String>,
    #[serde(default)]
    pub(crate) last_inbound_at: Option<String>,
    #[serde(default)]
    pub(crate) last_delivered_message_id: Option<String>,
    #[serde(default)]
    pub(crate) last_delivered_message_at: Option<String>,
    #[serde(default)]
    pub(crate) last_delivery_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub(crate) struct ChannelConversationRegistry {
    #[serde(default)]
    pub(crate) conversations: Vec<ChannelConversationRecord>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ConversationResolution {
    pub(crate) record: ChannelConversationRecord,
    pub(crate) duplicate_inbound: bool,
    pub(crate) session_restored: bool,
    pub(crate) session_assigned: bool,
}

pub(crate) fn list_conversations(
    runtime_home: &Path,
) -> Result<Vec<ChannelConversationRecord>, CliError> {
    Ok(load_registry(runtime_home)?.conversations)
}

pub(crate) fn load_conversation_by_target(
    runtime_home: &Path,
    target: &str,
) -> Result<Option<ChannelConversationRecord>, CliError> {
    Ok(load_registry(runtime_home)?
        .conversations
        .into_iter()
        .find(|record| record.target == target))
}

pub(crate) fn upsert_inbound_conversation(
    runtime_home: &Path,
    target: &str,
    inbound_message_id: &str,
    inbound_at: Option<&str>,
    default_session_id: Option<&str>,
) -> Result<ConversationResolution, CliError> {
    let now = local_timestamp_now();
    let mut registry = load_registry(runtime_home)?;
    let idx = registry
        .conversations
        .iter()
        .position(|record| record.target == target);
    let mut record = idx
        .map(|position| registry.conversations[position].clone())
        .unwrap_or_else(|| ChannelConversationRecord {
            conversation_id: conversation_id_for(target),
            channel_id: QQBOT_CHANNEL_ID.into(),
            target: target.to_string(),
            session_id: None,
            status: "unbound".into(),
            created_at: now.clone(),
            updated_at: now.clone(),
            last_seen_at: None,
            last_inbound_message_id: None,
            last_inbound_at: None,
            last_delivered_message_id: None,
            last_delivered_message_at: None,
            last_delivery_at: None,
        });
    let duplicate_inbound = record.last_inbound_message_id.as_deref() == Some(inbound_message_id);
    record.updated_at = now.clone();
    record.last_seen_at = Some(now.clone());
    record.last_inbound_message_id = Some(inbound_message_id.to_string());
    record.last_inbound_at = inbound_at.map(str::to_string).or_else(|| Some(now.clone()));

    let stored_session = record
        .session_id
        .as_deref()
        .filter(|session_id| session_exists(runtime_home, session_id));
    let default_session =
        default_session_id.filter(|session_id| session_exists(runtime_home, session_id));
    let session_restored = stored_session.is_some()
        && (default_session.is_none() || stored_session != default_session);
    let session_assigned = stored_session.is_none() && default_session.is_some();
    let session_just_bound =
        record.session_id.is_none() && stored_session.or(default_session).is_some();
    record.session_id = stored_session
        .map(str::to_string)
        .or_else(|| default_session.map(str::to_string));
    record.status = if record.session_id.is_some() {
        "bound".into()
    } else {
        "unbound".into()
    };
    if session_just_bound
        && record.last_delivered_message_id.is_none()
        && record.last_delivered_message_at.is_none()
    {
        if let Some((message_id, created_at)) =
            latest_deliverable_message(runtime_home, record.session_id.as_deref())?
        {
            record.last_delivered_message_id = Some(message_id);
            record.last_delivered_message_at = Some(created_at);
        }
    }

    if let Some(position) = idx {
        registry.conversations[position] = record.clone();
    } else {
        registry.conversations.push(record.clone());
    }
    persist_registry(runtime_home, &registry)?;

    Ok(ConversationResolution {
        record,
        duplicate_inbound,
        session_restored,
        session_assigned,
    })
}

pub(crate) fn mark_delivered_message(
    runtime_home: &Path,
    target: &str,
    message_id: &str,
    created_at: &str,
) -> Result<Option<ChannelConversationRecord>, CliError> {
    update_conversation(runtime_home, target, |record| {
        record.last_delivered_message_id = Some(message_id.to_string());
        record.last_delivered_message_at = Some(created_at.to_string());
        record.last_delivery_at = Some(local_timestamp_now());
    })
}

pub(crate) fn pending_outbound_messages(
    runtime_home: &Path,
    target: &str,
) -> Result<Option<(ChannelConversationRecord, Vec<SessionMessageRecord>)>, CliError> {
    let Some(record) = load_conversation_by_target(runtime_home, target)? else {
        return Ok(None);
    };
    let Some(session_id) = record.session_id.as_deref() else {
        return Ok(Some((record, Vec::new())));
    };
    let Some(messages_path) = session_messages_path(runtime_home, session_id) else {
        return Ok(Some((record, Vec::new())));
    };
    let messages = read_session_messages(&messages_path)?;
    let boundary_id = record.last_delivered_message_id.as_deref();
    let boundary_at = record.last_delivered_message_at.as_deref();
    let mut reached_boundary = boundary_id.is_none() && boundary_at.is_none();
    let mut pending = Vec::new();
    for message in messages {
        if !matches!(message.role.as_str(), "assistant" | "system") {
            continue;
        }
        if !reached_boundary {
            if boundary_id.is_some_and(|value| value == message.message_id) {
                reached_boundary = true;
                continue;
            }
            if let Some(timestamp) = boundary_at {
                if message.created_at.as_str() <= timestamp {
                    continue;
                }
                reached_boundary = true;
            }
        }
        pending.push(message);
    }
    Ok(Some((record, pending)))
}

fn update_conversation<F>(
    runtime_home: &Path,
    target: &str,
    mut mutate: F,
) -> Result<Option<ChannelConversationRecord>, CliError>
where
    F: FnMut(&mut ChannelConversationRecord),
{
    let mut registry = load_registry(runtime_home)?;
    let Some(position) = registry
        .conversations
        .iter()
        .position(|record| record.target == target)
    else {
        return Ok(None);
    };
    let mut record = registry.conversations[position].clone();
    record.updated_at = local_timestamp_now();
    mutate(&mut record);
    registry.conversations[position] = record.clone();
    persist_registry(runtime_home, &registry)?;
    Ok(Some(record))
}

fn load_registry(runtime_home: &Path) -> Result<ChannelConversationRegistry, CliError> {
    let path = registry_path(runtime_home);
    match fs::read_to_string(&path) {
        Ok(body) => serde_json::from_str(&body).map_err(CliError::Serialize),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            Ok(ChannelConversationRegistry::default())
        }
        Err(source) => Err(CliError::ReadFile {
            path: path.display().to_string(),
            source,
        }),
    }
}

fn persist_registry(
    runtime_home: &Path,
    registry: &ChannelConversationRegistry,
) -> Result<(), CliError> {
    let path = registry_path(runtime_home);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|source| CliError::WriteFile {
            path: parent.display().to_string(),
            source,
        })?;
    }
    fs::write(
        &path,
        serde_json::to_vec_pretty(registry).map_err(CliError::Serialize)?,
    )
    .map_err(|source| CliError::WriteFile {
        path: path.display().to_string(),
        source,
    })
}

fn registry_path(runtime_home: &Path) -> PathBuf {
    runtime_home.join("runtime/channels/qqbot/conversations.json")
}

fn conversation_id_for(target: &str) -> String {
    let mut id = String::with_capacity(target.len() + 8);
    id.push_str("qqconv-");
    for ch in target.chars() {
        if ch.is_ascii_alphanumeric() {
            id.push(ch);
        } else {
            id.push('-');
        }
    }
    id
}

fn session_exists(runtime_home: &Path, session_id: &str) -> bool {
    find_session_dir(runtime_home, session_id).is_some()
}

fn session_messages_path(runtime_home: &Path, session_id: &str) -> Option<PathBuf> {
    let (_, _, dir) = find_session_dir(runtime_home, session_id)?;
    Some(dir.join("conversation/messages.json"))
}

fn latest_deliverable_message(
    runtime_home: &Path,
    session_id: Option<&str>,
) -> Result<Option<(String, String)>, CliError> {
    let Some(messages_path) =
        session_id.and_then(|value| session_messages_path(runtime_home, value))
    else {
        return Ok(None);
    };
    let messages = read_session_messages(&messages_path)?;
    Ok(messages
        .into_iter()
        .rev()
        .find(|message| matches!(message.role.as_str(), "assistant" | "system"))
        .map(|message| (message.message_id, message.created_at)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        sync::atomic::{AtomicU64, Ordering},
        time::{SystemTime, UNIX_EPOCH},
    };

    static TEMP_HOME_SEQ: AtomicU64 = AtomicU64::new(1);

    fn temp_runtime_home() -> PathBuf {
        std::env::temp_dir().join(format!(
            "fin-qqconv-{}-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("time")
                .as_nanos(),
            TEMP_HOME_SEQ.fetch_add(1, Ordering::Relaxed),
        ))
    }

    fn seed_session(home: &Path, session_id: &str, messages: &[SessionMessageRecord]) {
        let session_dir = home
            .join("sessions/2026/05")
            .join(session_id)
            .join("conversation");
        fs::create_dir_all(&session_dir).expect("conversation dir");
        fs::write(
            session_dir.join("messages.json"),
            serde_json::to_vec_pretty(messages).expect("json"),
        )
        .expect("messages");
    }

    #[test]
    fn inbound_conversation_restores_existing_session_without_active_pair() {
        let home = temp_runtime_home();
        fs::create_dir_all(&home).expect("home");
        seed_session(&home, "session-a", &[]);
        let first = upsert_inbound_conversation(
            &home,
            "qqbot:c2c:user-1",
            "msg-1",
            Some("2026-04-19T12:00:00+08:00"),
            Some("session-a"),
        )
        .expect("first");
        assert_eq!(first.record.session_id.as_deref(), Some("session-a"));
        let second = upsert_inbound_conversation(
            &home,
            "qqbot:c2c:user-1",
            "msg-2",
            Some("2026-04-19T12:01:00+08:00"),
            None,
        )
        .expect("second");
        assert!(second.session_restored);
        assert_eq!(second.record.session_id.as_deref(), Some("session-a"));
    }

    #[test]
    fn pending_outbound_messages_skip_user_and_cursor() {
        let home = temp_runtime_home();
        fs::create_dir_all(&home).expect("home");
        seed_session(
            &home,
            "session-b",
            &[
                SessionMessageRecord {
                    message_id: "user-1".into(),
                    role: "user".into(),
                    content: "hi".into(),
                    created_at: "2026-04-19T12:00:00+08:00".into(),
                    session_id: "session-b".into(),
                    task_id: None,
                    operation_id: None,
                    trace_id: None,
                    closure_id: None,
                },
                SessionMessageRecord {
                    message_id: "assistant-1".into(),
                    role: "assistant".into(),
                    content: "hello".into(),
                    created_at: "2026-04-19T12:00:01+08:00".into(),
                    session_id: "session-b".into(),
                    task_id: None,
                    operation_id: None,
                    trace_id: None,
                    closure_id: None,
                },
                SessionMessageRecord {
                    message_id: "system-1".into(),
                    role: "system".into(),
                    content: "notice".into(),
                    created_at: "2026-04-19T12:00:02+08:00".into(),
                    session_id: "session-b".into(),
                    task_id: None,
                    operation_id: None,
                    trace_id: None,
                    closure_id: None,
                },
            ],
        );
        let record = ChannelConversationRecord {
            conversation_id: conversation_id_for("qqbot:c2c:user-2"),
            channel_id: QQBOT_CHANNEL_ID.into(),
            target: "qqbot:c2c:user-2".into(),
            session_id: Some("session-b".into()),
            status: "bound".into(),
            created_at: "2026-04-19T12:00:00+08:00".into(),
            updated_at: "2026-04-19T12:00:00+08:00".into(),
            last_seen_at: None,
            last_inbound_message_id: Some("msg-a".into()),
            last_inbound_at: Some("2026-04-19T12:00:00+08:00".into()),
            last_delivered_message_id: None,
            last_delivered_message_at: None,
            last_delivery_at: None,
        };
        persist_registry(
            &home,
            &ChannelConversationRegistry {
                conversations: vec![record.clone()],
            },
        )
        .expect("registry");
        let (_, first_pending) = pending_outbound_messages(&home, &record.target)
            .expect("pending")
            .expect("present");
        assert_eq!(first_pending.len(), 2);
        mark_delivered_message(
            &home,
            &record.target,
            "assistant-1",
            "2026-04-19T12:00:01+08:00",
        )
        .expect("mark");
        let (_, second_pending) = pending_outbound_messages(&home, &record.target)
            .expect("pending")
            .expect("present");
        assert_eq!(second_pending.len(), 1);
        assert_eq!(second_pending[0].message_id, "system-1");
    }

    #[test]
    fn first_bind_to_existing_session_initializes_delivery_cursor() {
        let home = temp_runtime_home();
        fs::create_dir_all(&home).expect("home");
        seed_session(
            &home,
            "session-c",
            &[
                SessionMessageRecord {
                    message_id: "assistant-old".into(),
                    role: "assistant".into(),
                    content: "old reply".into(),
                    created_at: "2026-04-19T12:00:01+08:00".into(),
                    session_id: "session-c".into(),
                    task_id: None,
                    operation_id: None,
                    trace_id: None,
                    closure_id: None,
                },
                SessionMessageRecord {
                    message_id: "assistant-new".into(),
                    role: "assistant".into(),
                    content: "new reply".into(),
                    created_at: "2026-04-19T12:00:02+08:00".into(),
                    session_id: "session-c".into(),
                    task_id: None,
                    operation_id: None,
                    trace_id: None,
                    closure_id: None,
                },
            ],
        );
        let record = upsert_inbound_conversation(
            &home,
            "qqbot:c2c:user-3",
            "msg-first",
            Some("2026-04-19T12:00:03+08:00"),
            Some("session-c"),
        )
        .expect("conversation")
        .record;
        assert_eq!(
            record.last_delivered_message_id.as_deref(),
            Some("assistant-new")
        );
        let (_, pending) = pending_outbound_messages(&home, &record.target)
            .expect("pending")
            .expect("present");
        assert!(pending.is_empty());
    }
}
