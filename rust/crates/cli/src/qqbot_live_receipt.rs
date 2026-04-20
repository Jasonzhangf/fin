use crate::{
    CliError,
    channel_peer_conversations::load_conversation_by_target,
    runtime_home::{read_session_messages, resolved_runtime_home},
    session_binding::{find_session_dir, infer_session_task_id},
    time::local_timestamp_now,
};
use fin_config::SystemConfig;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    fs,
    path::{Path, PathBuf},
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct QqbotLiveReceiptReport {
    pub(crate) run_id: String,
    pub(crate) target: String,
    pub(crate) conversation_id: String,
    pub(crate) conversation_status: String,
    pub(crate) session_id: String,
    #[serde(default)]
    pub(crate) task_id: Option<String>,
    pub(crate) status: String,
    pub(crate) runtime_home: String,
    pub(crate) run_root: String,
    pub(crate) receipt_path: String,
    pub(crate) conversation_registry_path: String,
    pub(crate) peer_events_path: String,
    pub(crate) session_messages_path: String,
    pub(crate) provider_requests_path: String,
    pub(crate) provider_responses_path: String,
    #[serde(default)]
    pub(crate) last_inbound_message_id: Option<String>,
    #[serde(default)]
    pub(crate) last_inbound_at: Option<String>,
    #[serde(default)]
    pub(crate) last_delivered_message_id: Option<String>,
    #[serde(default)]
    pub(crate) last_delivered_message_at: Option<String>,
    pub(crate) ack_notice_present: bool,
    pub(crate) session_reply_present: bool,
    pub(crate) provider_request_present: bool,
    pub(crate) provider_response_present: bool,
    #[serde(default)]
    pub(crate) latest_reply_preview: Option<String>,
    pub(crate) peer_event_count: usize,
    pub(crate) verified_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct QqbotLiveReceiptLayout {
    run_id: String,
    run_root: PathBuf,
    receipt_path: PathBuf,
    source_runtime_home: PathBuf,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
struct PeerEvent {
    event_type: String,
    #[serde(default)]
    payload: Value,
}

pub(crate) fn run_qqbot_live_receipt(
    system: &SystemConfig,
    runtime_home_override: Option<&Path>,
    target: &str,
    run_id_override: Option<&str>,
) -> Result<QqbotLiveReceiptReport, CliError> {
    let layout = qqbot_live_receipt_layout(system, runtime_home_override, run_id_override);
    let report = collect_qqbot_live_receipt(&layout, target)?;
    write_report(&report, &layout.receipt_path)?;
    Ok(report)
}

fn qqbot_live_receipt_layout(
    system: &SystemConfig,
    runtime_home_override: Option<&Path>,
    run_id_override: Option<&str>,
) -> QqbotLiveReceiptLayout {
    let source_runtime_home = resolved_runtime_home(system, runtime_home_override);
    let run_id = run_id_override
        .map(sanitize_run_id)
        .filter(|value| !value.is_empty())
        .unwrap_or_else(default_run_id);
    let run_root = source_runtime_home.join("harness/runs").join(&run_id);
    QqbotLiveReceiptLayout {
        receipt_path: run_root.join("qqbot-live-receipt.json"),
        run_id,
        run_root,
        source_runtime_home,
    }
}

fn default_run_id() -> String {
    format!(
        "qqbot-live-receipt-{}",
        chrono::Local::now().format("%Y%m%d-%H%M%S")
    )
}

fn sanitize_run_id(raw: &str) -> String {
    raw.chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.') {
                ch
            } else {
                '-'
            }
        })
        .collect::<String>()
        .trim_matches('-')
        .to_string()
}

fn collect_qqbot_live_receipt(
    layout: &QqbotLiveReceiptLayout,
    target: &str,
) -> Result<QqbotLiveReceiptReport, CliError> {
    let runtime_home = &layout.source_runtime_home;
    let conversation = load_conversation_by_target(runtime_home, target)?.ok_or_else(|| {
        CliError::InvalidInstallState(format!(
            "qqbot target not found in conversation registry: {target}"
        ))
    })?;
    let session_id = conversation.session_id.clone().ok_or_else(|| {
        CliError::InvalidInstallState(format!("qqbot target has no bound session: {target}"))
    })?;
    let (_, _, session_dir) = find_session_dir(runtime_home, &session_id).ok_or_else(|| {
        CliError::InvalidInstallState(format!("qqbot session directory missing: {session_id}"))
    })?;

    let messages_path = session_dir.join("conversation/messages.json");
    let provider_requests_path = session_dir.join("provider/recent_provider_requests.json");
    let provider_responses_path = session_dir.join("provider/recent_provider_responses.json");
    let conversation_registry_path = runtime_home.join("runtime/channels/qqbot/conversations.json");
    let peer_events_path = runtime_home.join("runtime/peers/qqbot/events.jsonl");

    let messages = read_session_messages(&messages_path)?;
    let peer_events = read_peer_events(&peer_events_path)?;
    let last_inbound_at = conversation.last_inbound_at.as_deref();
    let latest_reply = latest_reply_after(&messages, last_inbound_at);
    let latest_reply_preview = latest_reply.map(|message| shorten(message.content.as_str(), 160));

    let ack_notice_present = peer_events.iter().any(|event| {
        event.event_type == "channel.peer.notice_send_requested"
            && event.payload.get("target").and_then(Value::as_str) == Some(target)
            && event.payload.get("notice_kind").and_then(Value::as_str) == Some("received_ack")
            && event.payload.get("reply_to_id").and_then(Value::as_str)
                == conversation.last_inbound_message_id.as_deref()
    });
    let session_reply_present = latest_reply.is_some()
        && conversation.last_delivered_message_id.as_deref()
            == latest_reply.map(|message| message.message_id.as_str());
    let provider_request_present = file_contains_non_empty_json_array(&provider_requests_path)?;
    let provider_response_present = file_contains_non_empty_json_array(&provider_responses_path)?;
    let status = if ack_notice_present && session_reply_present && provider_request_present {
        "passed"
    } else {
        "failed"
    };

    Ok(QqbotLiveReceiptReport {
        run_id: layout.run_id.clone(),
        target: target.to_string(),
        conversation_id: conversation.conversation_id,
        conversation_status: conversation.status,
        session_id,
        task_id: infer_session_task_id(&session_dir),
        status: status.into(),
        runtime_home: runtime_home.display().to_string(),
        run_root: layout.run_root.display().to_string(),
        receipt_path: layout.receipt_path.display().to_string(),
        conversation_registry_path: conversation_registry_path.display().to_string(),
        peer_events_path: peer_events_path.display().to_string(),
        session_messages_path: messages_path.display().to_string(),
        provider_requests_path: provider_requests_path.display().to_string(),
        provider_responses_path: provider_responses_path.display().to_string(),
        last_inbound_message_id: conversation.last_inbound_message_id,
        last_inbound_at: conversation.last_inbound_at,
        last_delivered_message_id: conversation.last_delivered_message_id,
        last_delivered_message_at: conversation.last_delivered_message_at,
        ack_notice_present,
        session_reply_present,
        provider_request_present,
        provider_response_present,
        latest_reply_preview,
        peer_event_count: peer_events.len(),
        verified_at: local_timestamp_now(),
    })
}

fn read_peer_events(path: &Path) -> Result<Vec<PeerEvent>, CliError> {
    match fs::read_to_string(path) {
        Ok(content) => content
            .lines()
            .filter(|line| !line.trim().is_empty())
            .map(|line| serde_json::from_str::<PeerEvent>(line).map_err(CliError::Serialize))
            .collect(),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(source) => Err(CliError::ReadFile {
            path: path.display().to_string(),
            source,
        }),
    }
}

fn latest_reply_after<'a>(
    messages: &'a [crate::runtime_home::SessionMessageRecord],
    last_inbound_at: Option<&str>,
) -> Option<&'a crate::runtime_home::SessionMessageRecord> {
    messages.iter().rev().find(|message| {
        matches!(message.role.as_str(), "assistant" | "system")
            && last_inbound_at.is_none_or(|boundary| message.created_at.as_str() >= boundary)
    })
}

fn file_contains_non_empty_json_array(path: &Path) -> Result<bool, CliError> {
    match fs::read_to_string(path) {
        Ok(content) => {
            let value: Value = serde_json::from_str(&content)?;
            Ok(value.as_array().is_some_and(|items| !items.is_empty()))
        }
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(source) => Err(CliError::ReadFile {
            path: path.display().to_string(),
            source,
        }),
    }
}

fn write_report(report: &QqbotLiveReceiptReport, path: &Path) -> Result<(), CliError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|source| CliError::WriteFile {
            path: parent.display().to_string(),
            source,
        })?;
    }
    fs::write(
        path,
        serde_json::to_vec_pretty(report).map_err(CliError::Serialize)?,
    )
    .map_err(|source| CliError::WriteFile {
        path: path.display().to_string(),
        source,
    })
}

fn shorten(input: &str, max_chars: usize) -> String {
    let mut chars = input.chars();
    let shortened = chars.by_ref().take(max_chars).collect::<String>();
    if chars.next().is_some() {
        format!("{shortened}…")
    } else {
        shortened
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::channel_peer_conversations::{
        ChannelConversationRecord, ChannelConversationRegistry,
    };
    use crate::runtime_home::{SessionMessageRecord, ensure_runtime_home_layout};
    use fin_config::ConfigMapper;
    use std::collections::BTreeMap;

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
            "fin-qqbot-live-receipt-tests-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("time")
                .as_nanos()
        ))
    }

    fn seed_session(home: &Path, session_id: &str) -> PathBuf {
        let session_dir = home.join("sessions/2026/04").join(session_id);
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
                "{\"event_type\":\"channel.peer.notice_send_requested\",\"payload\":{\"target\":\"qqbot:c2c:user-1\",\"notice_kind\":\"received_ack\",\"reply_to_id\":\"msg-inbound-1\"}}\n",
                "{\"event_type\":\"channel.peer.session_message_send_requested\",\"payload\":{\"target\":\"qqbot:c2c:user-1\",\"message_id\":\"assistant-1\"}}\n"
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
}
