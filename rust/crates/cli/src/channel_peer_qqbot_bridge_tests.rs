use super::*;
use crate::channel_peer_conversations::{
    ChannelConversationRecord, ChannelConversationRegistry, load_conversation_by_target,
};
use crate::runtime_home::SessionMessageRecord;
use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{Arc, Mutex},
    time::{SystemTime, UNIX_EPOCH},
};

#[test]
fn outbound_target_maps_group_and_direct_messages() {
    let group = QqbotBridgeInboundMessage {
        message_type: "group".into(),
        sender_id: "user-1".into(),
        sender_name: None,
        content: "hello".into(),
        message_id: "msg-1".into(),
        timestamp: "2026-04-19T12:00:00+08:00".into(),
        group_openid: Some("group-1".into()),
        channel_id: None,
        guild_id: None,
        attachments: None,
    };
    assert_eq!(outbound_target_for(&group).unwrap(), "qqbot:group:group-1");

    let direct = QqbotBridgeInboundMessage {
        message_type: "c2c".into(),
        sender_id: "user-2".into(),
        sender_name: Some("Jason".into()),
        content: "hello".into(),
        message_id: "msg-2".into(),
        timestamp: "2026-04-19T12:00:00+08:00".into(),
        group_openid: None,
        channel_id: None,
        guild_id: None,
        attachments: None,
    };
    assert_eq!(outbound_target_for(&direct).unwrap(), "qqbot:c2c:user-2");
}

#[test]
fn attachment_summaries_keep_whitelisted_fields() {
    let attachments = serde_json::json!([{
        "id": "att-1",
        "content_type": "image/png",
        "filename": "demo.png",
        "url": "https://example.com/demo.png",
        "size": "42",
        "width": 128,
        "height": 64,
        "secret": "ignored"
    }]);
    let items = support::sanitize_attachment_summaries(attachments.as_array().map(Vec::as_slice));
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].attachment_id.as_deref(), Some("att-1"));
    assert_eq!(items[0].content_type.as_deref(), Some("image/png"));
    assert_eq!(items[0].name.as_deref(), Some("demo.png"));
    assert_eq!(items[0].size_bytes, Some(42));
    assert_eq!(items[0].width, Some(128));
    assert_eq!(items[0].height, Some(64));
}

#[test]
fn attachment_only_message_gets_fallback_prompt() {
    let attachments = vec![fin_contracts::InputAttachmentSummary {
        name: Some("demo.png".into()),
        ..fin_contracts::InputAttachmentSummary::default()
    }];
    let normalized = support::normalize_inbound_content("", &attachments);
    assert!(normalized.contains("用户发送了附件"));
    assert!(normalized.contains("demo.png"));
}

#[test]
fn text_channel_output_strips_fin_tags_and_markdown_emphasis() {
    let rendered = support::sanitize_text_channel_output(
        "<fin_user_response>\n**标题**\n- **项目范围:** `fin`\n</fin_user_response>\n<fin_control_feedback>{\"a\":1}</fin_control_feedback>",
    );
    assert!(!rendered.contains("<fin_user_response>"));
    assert!(!rendered.contains("**"));
    assert!(!rendered.contains('`'));
    assert!(!rendered.contains("fin_control_feedback"));
    assert!(rendered.contains("标题"));
    assert!(rendered.contains("- 项目范围: fin"));
}

#[test]
fn notice_texts_are_plain_and_non_empty() {
    assert_eq!(support::inbound_ack_text(), "已收到，正在处理。");
    assert!(support::unbound_session_notice_text().contains("绑定会话"));
    assert!(support::empty_payload_notice_text().contains("没有可处理的文本"));
    assert!(support::no_new_messages_notice_text().contains("没有生成新的可发送回复"));
}

#[test]
fn processing_failed_notice_is_short_and_prefixed() {
    let rendered = support::processing_failed_notice_text(
        "provider timeout while waiting for upstream response from anthropic-compatible endpoint",
    );
    assert!(rendered.starts_with("处理失败："));
    assert!(rendered.len() < 220);
}

fn temp_runtime_home() -> PathBuf {
    std::env::temp_dir().join(format!(
        "fin-qqbot-bridge-tests-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos()
    ))
}

fn write_json(path: &Path, value: &impl serde::Serialize) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("create parent");
    }
    fs::write(path, serde_json::to_vec_pretty(value).expect("json")).expect("write");
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
fn empty_sanitized_session_message_is_not_marked_delivered() {
    let home = temp_runtime_home();
    fs::create_dir_all(&home).expect("home");
    seed_session(
        &home,
        "session-empty-send",
        &[
            SessionMessageRecord {
                message_id: "assistant-empty".into(),
                role: "assistant".into(),
                content: "<fin_control_feedback>{\"a\":1}</fin_control_feedback>".into(),
                created_at: "2026-04-20T10:00:01+08:00".into(),
                session_id: "session-empty-send".into(),
                task_id: None,
                operation_id: None,
                trace_id: None,
                closure_id: None,
            },
            SessionMessageRecord {
                message_id: "assistant-real".into(),
                role: "assistant".into(),
                content: "<fin_user_response>real reply</fin_user_response>".into(),
                created_at: "2026-04-20T10:00:02+08:00".into(),
                session_id: "session-empty-send".into(),
                task_id: None,
                operation_id: None,
                trace_id: None,
                closure_id: None,
            },
        ],
    );
    write_json(
        &home.join("runtime/channels/qqbot/conversations.json"),
        &ChannelConversationRegistry {
            conversations: vec![ChannelConversationRecord {
                conversation_id: "qqconv-user-1".into(),
                channel_id: "qqbot".into(),
                target: "qqbot:c2c:user-1".into(),
                session_id: Some("session-empty-send".into()),
                status: "bound".into(),
                created_at: "2026-04-20T10:00:00+08:00".into(),
                updated_at: "2026-04-20T10:00:00+08:00".into(),
                last_seen_at: None,
                last_inbound_message_id: Some("inbound-1".into()),
                last_inbound_at: Some("2026-04-20T10:00:00+08:00".into()),
                last_delivered_message_id: None,
                last_delivered_message_at: None,
                last_delivery_at: None,
            }],
        },
    );

    let mut child = Command::new("sh")
        .arg("-c")
        .arg("cat >/dev/null")
        .stdin(Stdio::piped())
        .spawn()
        .expect("spawn sink");
    let stdin = Arc::new(Mutex::new(child.stdin.take()));

    let delivered = support::deliver_pending_messages_for_target(
        &home,
        &stdin,
        "qqbot:c2c:user-1",
        Some("inbound-1"),
        "test",
    )
    .expect("deliver");
    assert_eq!(delivered, 1);

    let conversation = load_conversation_by_target(&home, "qqbot:c2c:user-1")
        .expect("conversation")
        .expect("present");
    assert_eq!(
        conversation.last_delivered_message_id.as_deref(),
        Some("assistant-real")
    );

    let _ = child.kill();
    let _ = child.wait();
}
