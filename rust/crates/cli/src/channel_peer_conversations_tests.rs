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
        .join("sessions/2026/04")
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
                sender_kind: Some("user_message".into()),
                role_id: None,
                agent_name: None,
                display_name: Some("User".into()),
                source_kind: Some("conversation_message".into()),
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
                sender_kind: Some("agent_reply".into()),
                role_id: Some("system".into()),
                agent_name: Some("system".into()),
                display_name: Some("System Agent".into()),
                source_kind: Some("session_message".into()),
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
                sender_kind: Some("system_notice".into()),
                role_id: None,
                agent_name: None,
                display_name: Some("系统通知".into()),
                source_kind: Some("system_notice".into()),
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
                sender_kind: Some("agent_reply".into()),
                role_id: Some("system".into()),
                agent_name: Some("system".into()),
                display_name: Some("System Agent".into()),
                source_kind: Some("session_message".into()),
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
                sender_kind: Some("agent_reply".into()),
                role_id: Some("system".into()),
                agent_name: Some("system".into()),
                display_name: Some("System Agent".into()),
                source_kind: Some("session_message".into()),
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
