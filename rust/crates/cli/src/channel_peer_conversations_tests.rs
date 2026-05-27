use super::channel_peer_conversations::*;
use crate::runtime_home::SessionMessageRecord;
use std::{
    fs,
    path::{Path, PathBuf},
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

fn seed_ledger_snapshot(home: &Path, session_id: &str, message_id: &str, created_at: &str) {
    let ledger = fin_runtime::LedgerStore::for_session(home, session_id).expect("ledger");
    let refs = fin_contracts::EntityRefs {
        session_id: Some(session_id.into()),
        task_id: Some("task-ledger".into()),
        worker_id: Some("worker-ledger".into()),
        ..fin_contracts::EntityRefs::default()
    };
    ledger
        .init(Some("task-ledger"), Some(session_id), created_at)
        .expect("init");
    let snapshot = fin_contracts::SessionSnapshotRecord {
        snapshot_id: message_id.into(),
        operation_id: format!("op-{message_id}"),
        trace_id: format!("trace-{message_id}"),
        turn_id: format!("turn-{message_id}"),
        refs: refs.clone(),
        user_input: Some("hi".into()),
        assistant_summary: "ledger assistant reply".into(),
        important_tool_refs: Vec::new(),
        artifact_refs: Vec::new(),
        summary: Some("ledger summary".into()),
        created_at: created_at.into(),
    };
    ledger
        .append(fin_runtime::AppendLedgerRecordInput {
            ts: created_at.into(),
            track: fin_contracts::LedgerTrackKind::SessionSnapshot,
            record_id: message_id.into(),
            record_kind: "session_snapshot".into(),
            refs: fin_contracts::LedgerRefs {
                agent_id: Some("worker-ledger".into()),
                entity: refs,
                ledger_id: Some(session_id.into()),
                record_refs: Vec::new(),
            },
            payload: serde_json::to_value(snapshot).expect("snapshot"),
            caused_by: Some(format!("detail-{message_id}")),
            supersedes: None,
        })
        .expect("append snapshot");
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

#[test]
fn latest_deliverable_message_uses_ledger_when_messages_projection_missing() {
    let home = temp_runtime_home();
    fs::create_dir_all(&home).expect("home");
    seed_ledger_snapshot(
        &home,
        "session-ledger",
        "snapshot-1",
        "2026-04-19T12:00:02+08:00",
    );
    let record = upsert_inbound_conversation(
        &home,
        "qqbot:c2c:user-ledger",
        "msg-1",
        Some("2026-04-19T12:00:03+08:00"),
        Some("session-ledger"),
    )
    .expect("conversation")
    .record;
    assert_eq!(
        record.last_delivered_message_id.as_deref(),
        Some("assistant-ledger-snapshot-1")
    );
    assert_eq!(
        record.last_delivered_message_at.as_deref(),
        Some("2026-04-19T12:00:02+08:00")
    );
}
