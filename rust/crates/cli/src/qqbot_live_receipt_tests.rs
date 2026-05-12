use super::*;
use crate::channel_peer_conversations::{ChannelConversationRecord, ChannelConversationRegistry};
use crate::runtime_home::{current_session_dir, SessionMessageRecord, ensure_runtime_home_layout};
use fin_config::ConfigMapper;
use std::{
    collections::BTreeMap,
    sync::atomic::{AtomicU64, Ordering},
};

static TEMP_SEQ: AtomicU64 = AtomicU64::new(1);

fn sample_system_config() -> SystemConfig {
    let user = fin_config::UserConfig {
        default_provider: "local-anthropic".into(),
        providers: BTreeMap::from([(
            "local-anthropic".into(),
            fin_config::UserProviderConfig {
                protocol: fin_config::ProviderProtocol::AnthropicWire,
                base_url: "https://example.invalid".into(),
                model: "qwen3.6-plus".into(),
                api_key: Some("test-key".into()),
                api_key_env: None,
                user_agent: None,
                headers: BTreeMap::new(),
            },
        )]),
        runtime: fin_config::UserRuntimeConfig::default(),
    };
    ConfigMapper::map_user_to_system(&user).expect("system config")
}

fn temp_runtime_home() -> PathBuf {
    std::env::temp_dir().join(format!(
        "fin-qqbot-live-receipt-tests-{}-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("time")
            .as_nanos(),
        TEMP_SEQ.fetch_add(1, Ordering::Relaxed)
    ))
}

fn seed_session(home: &Path, session_id: &str) -> PathBuf {
    let session_dir = current_session_dir(&home, &session_id);
    fs::create_dir_all(session_dir.join("conversation")).expect("conversation dir");
    fs::create_dir_all(session_dir.join("provider")).expect("provider dir");
    session_dir
}

fn write_json(path: &Path, value: &impl serde::Serialize) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("parent");
    }
    fs::write(path, serde_json::to_vec_pretty(value).expect("json")).expect("write");
}

#[test]
fn qqbot_live_receipt_collects_passed_closure() {
    let home = temp_runtime_home();
    ensure_runtime_home_layout(&home).expect("runtime home");
    let session_dir = seed_session(&home, "session-live-receipt");
    write_json(
        &session_dir.join("conversation/messages.json"),
        &vec![
            SessionMessageRecord {
                message_id: "user-1".into(),
                role: "user".into(),
                content: "hello qqbot".into(),
                created_at: "2026-04-20T21:00:00+08:00".into(),
                session_id: "session-live-receipt".into(),
                task_id: Some("task-live-receipt".into()),
                operation_id: Some("op-live-1".into()),
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
                content: "live qqbot reply".into(),
                created_at: "2026-04-20T21:00:05+08:00".into(),
                session_id: "session-live-receipt".into(),
                task_id: Some("task-live-receipt".into()),
                operation_id: Some("op-live-1".into()),
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
    write_json(
        &session_dir.join("provider/recent_provider_requests.json"),
        &vec![serde_json::json!({"input":"hello qqbot"})],
    );
    write_json(
        &session_dir.join("provider/recent_provider_responses.json"),
        &vec![serde_json::json!({"output_text":"live qqbot reply"})],
    );
    write_json(
        &home.join("runtime/channels/qqbot/conversations.json"),
        &ChannelConversationRegistry {
            conversations: vec![ChannelConversationRecord {
                conversation_id: "conv-1".into(),
                channel_id: "qqbot".into(),
                target: "qqbot:c2c:user-1".into(),
                session_id: Some("session-live-receipt".into()),
                status: "bound".into(),
                created_at: "2026-04-20T21:00:00+08:00".into(),
                updated_at: "2026-04-20T21:00:05+08:00".into(),
                last_seen_at: Some("2026-04-20T21:00:00+08:00".into()),
                last_inbound_message_id: Some("msg-inbound-1".into()),
                last_inbound_at: Some("2026-04-20T21:00:00+08:00".into()),
                last_delivered_message_id: Some("assistant-1".into()),
                last_delivered_message_at: Some("2026-04-20T21:00:05+08:00".into()),
                last_delivery_at: Some("2026-04-20T21:00:05+08:00".into()),
            }],
        },
    );
    fs::create_dir_all(home.join("runtime/peers/qqbot")).expect("peer dir");
    fs::write(
            home.join("runtime/peers/qqbot/events.jsonl"),
            concat!(
                "{\"event_type\":\"channel.peer.notice_send_requested\",\"occurred_at\":\"2026-04-20T21:00:00+08:00\",\"payload\":{\"target\":\"qqbot:c2c:user-1\",\"notice_kind\":\"received_ack\",\"reply_to_id\":\"msg-inbound-1\"}}\n",
                "{\"event_type\":\"channel.peer.session_message_send_requested\",\"occurred_at\":\"2026-04-20T21:00:05+08:00\",\"payload\":{\"target\":\"qqbot:c2c:user-1\",\"message_id\":\"assistant-1\"}}\n"
            ),
        )
        .expect("events");

    let report = run_qqbot_live_receipt(
        &sample_system_config(),
        Some(&home),
        "qqbot:c2c:user-1",
        Some("live-receipt-run"),
    )
    .expect("receipt");

    assert_eq!(report.status, "passed");
    assert!(report.ack_notice_present);
    assert!(report.session_reply_present);
    assert!(report.provider_request_present);
    assert!(report.provider_response_present);
    assert!(!report.no_new_messages_notice_present);
    assert!(!report.waiting_card_with_failure_present);
    assert!(!report.empty_assistant_message_present);
    assert_eq!(report.current_execution_status, None);
    assert!(!report.waiting_failure_card_stale_against_current_execution);
    assert_eq!(report.task_id.as_deref(), Some("task-live-receipt"));
    assert_eq!(
        report.latest_reply_preview.as_deref(),
        Some("live qqbot reply")
    );
    assert!(Path::new(&report.receipt_path).exists());
}

#[test]
fn qqbot_live_receipt_marks_failed_when_ack_missing() {
    let home = temp_runtime_home();
    ensure_runtime_home_layout(&home).expect("runtime home");
    let session_dir = seed_session(&home, "session-live-receipt-failed");
    write_json(
        &session_dir.join("conversation/messages.json"),
        &vec![SessionMessageRecord {
            message_id: "assistant-2".into(),
            role: "assistant".into(),
            content: "reply without ack".into(),
            created_at: "2026-04-20T21:10:05+08:00".into(),
            session_id: "session-live-receipt-failed".into(),
            task_id: Some("task-failed".into()),
            operation_id: Some("op-failed".into()),
            trace_id: None,
            closure_id: None,
            sender_kind: Some("agent_reply".into()),
            role_id: Some("system".into()),
            agent_name: Some("system".into()),
            display_name: Some("System Agent".into()),
            source_kind: Some("session_message".into()),
        }],
    );
    write_json(
        &home.join("runtime/channels/qqbot/conversations.json"),
        &ChannelConversationRegistry {
            conversations: vec![ChannelConversationRecord {
                conversation_id: "conv-2".into(),
                channel_id: "qqbot".into(),
                target: "qqbot:c2c:user-2".into(),
                session_id: Some("session-live-receipt-failed".into()),
                status: "bound".into(),
                created_at: "2026-04-20T21:10:00+08:00".into(),
                updated_at: "2026-04-20T21:10:05+08:00".into(),
                last_seen_at: Some("2026-04-20T21:10:00+08:00".into()),
                last_inbound_message_id: Some("msg-inbound-2".into()),
                last_inbound_at: Some("2026-04-20T21:10:00+08:00".into()),
                last_delivered_message_id: Some("assistant-2".into()),
                last_delivered_message_at: Some("2026-04-20T21:10:05+08:00".into()),
                last_delivery_at: Some("2026-04-20T21:10:05+08:00".into()),
            }],
        },
    );
    fs::create_dir_all(home.join("runtime/peers/qqbot")).expect("peer dir");
    fs::write(home.join("runtime/peers/qqbot/events.jsonl"), b"").expect("events");

    let report = run_qqbot_live_receipt(
        &sample_system_config(),
        Some(&home),
        "qqbot:c2c:user-2",
        Some("live-receipt-failed"),
    )
    .expect("receipt");

    assert_eq!(report.status, "failed");
    assert!(!report.ack_notice_present);
    assert!(report.session_reply_present);
    assert!(!report.provider_request_present);
}

#[test]
fn qqbot_live_receipt_fails_when_waiting_card_still_contains_failure() {
    let home = temp_runtime_home();
    ensure_runtime_home_layout(&home).expect("runtime home");
    let session_dir = seed_session(&home, "session-live-receipt-waiting-failure");
    write_json(
        &session_dir.join("conversation/messages.json"),
        &vec![SessionMessageRecord {
            message_id: "assistant-3".into(),
            role: "assistant".into(),
            content: "reply after stale failure".into(),
            created_at: "2026-04-20T21:20:05+08:00".into(),
            session_id: "session-live-receipt-waiting-failure".into(),
            task_id: Some("task-waiting-failure".into()),
            operation_id: Some("op-waiting-failure".into()),
            trace_id: None,
            closure_id: None,
            sender_kind: Some("agent_reply".into()),
            role_id: Some("system".into()),
            agent_name: Some("system".into()),
            display_name: Some("System Agent".into()),
            source_kind: Some("session_message".into()),
        }],
    );
    write_json(
        &session_dir.join("provider/recent_provider_requests.json"),
        &vec![serde_json::json!({"input":"hello"})],
    );
    write_json(
        &session_dir.join("provider/recent_provider_responses.json"),
        &vec![serde_json::json!({"output_text":"reply after stale failure"})],
    );
    write_json(
        &session_dir.join("control/execution_state.json"),
        &serde_json::json!({
            "state_id":"exec-state-waiting-failure",
            "session_id":"session-live-receipt-waiting-failure",
            "task_id":"task-waiting-failure",
            "status":"idle",
            "pending_input_count":0,
            "accepts_user_input":true,
            "updated_at":"2026-04-20T21:20:05+08:00"
        }),
    );
    write_json(
        &home.join("runtime/channels/qqbot/conversations.json"),
        &ChannelConversationRegistry {
            conversations: vec![ChannelConversationRecord {
                conversation_id: "conv-3".into(),
                channel_id: "qqbot".into(),
                target: "qqbot:c2c:user-3".into(),
                session_id: Some("session-live-receipt-waiting-failure".into()),
                status: "bound".into(),
                created_at: "2026-04-20T21:20:00+08:00".into(),
                updated_at: "2026-04-20T21:20:05+08:00".into(),
                last_seen_at: Some("2026-04-20T21:20:00+08:00".into()),
                last_inbound_message_id: Some("msg-inbound-3".into()),
                last_inbound_at: Some("2026-04-20T21:20:00+08:00".into()),
                last_delivered_message_id: Some("assistant-3".into()),
                last_delivered_message_at: Some("2026-04-20T21:20:05+08:00".into()),
                last_delivery_at: Some("2026-04-20T21:20:05+08:00".into()),
            }],
        },
    );
    fs::create_dir_all(home.join("runtime/peers/qqbot")).expect("peer dir");
    fs::write(
            home.join("runtime/peers/qqbot/events.jsonl"),
            concat!(
                "{\"event_type\":\"channel.peer.notice_send_requested\",\"occurred_at\":\"2026-04-20T21:20:00+08:00\",\"payload\":{\"target\":\"qqbot:c2c:user-3\",\"notice_kind\":\"received_ack\",\"reply_to_id\":\"msg-inbound-3\"}}\n",
                "{\"event_type\":\"channel.peer.activity_card_send_requested\",\"occurred_at\":\"2026-04-20T21:20:01+08:00\",\"payload\":{\"target\":\"qqbot:c2c:user-3\",\"text_preview\":\"📡 状态卡 · System Agent · ⏳ 等待中 · 已收到，正在处理\\n📍 已收到，正在处理\\n❌ Edited file (失败)\"}}\n",
                "{\"event_type\":\"channel.peer.session_message_send_requested\",\"occurred_at\":\"2026-04-20T21:20:05+08:00\",\"payload\":{\"target\":\"qqbot:c2c:user-3\",\"message_id\":\"assistant-3\"}}\n"
            ),
        )
        .expect("events");

    let report = run_qqbot_live_receipt(
        &sample_system_config(),
        Some(&home),
        "qqbot:c2c:user-3",
        Some("live-receipt-waiting-failure"),
    )
    .expect("receipt");

    assert_eq!(report.status, "failed");
    assert!(report.waiting_card_with_failure_present);
    assert_eq!(report.current_execution_status.as_deref(), Some("idle"));
    assert!(report.waiting_failure_card_stale_against_current_execution);
    assert_eq!(
        report.latest_activity_card_preview.as_deref(),
        Some(
            "📡 状态卡 · System Agent · ⏳ 等待中 · 已收到，正在处理\n📍 已收到，正在处理\n❌ Edited file (失败)"
        )
    );
}
