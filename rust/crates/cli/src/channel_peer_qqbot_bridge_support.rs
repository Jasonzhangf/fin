use crate::{
    CliError,
    channel_peer::record_builtin_qqbot_runtime_event,
    channel_peer_activity_delivery::{
        current_delivery_signature_if_deliverable, mark_delivered, prepare_periodic_delivery,
    },
    channel_peer_conversations::{
        list_conversations, mark_delivered_message, pending_outbound_messages,
    },
};
use fin_contracts::InputAttachmentSummary;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    io::Write,
    path::PathBuf,
    process::ChildStdin,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread::{self, JoinHandle},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

#[derive(Debug, Clone, Deserialize)]
pub(super) struct QqbotBridgeInboundMessage {
    #[serde(rename = "type")]
    pub(super) message_type: String,
    #[serde(rename = "senderId")]
    pub(super) sender_id: String,
    #[serde(rename = "senderName", default)]
    pub(super) sender_name: Option<String>,
    pub(super) content: String,
    #[serde(rename = "messageId")]
    pub(super) message_id: String,
    pub(super) timestamp: String,
    #[serde(rename = "groupOpenid", default)]
    pub(super) group_openid: Option<String>,
    #[serde(rename = "channelId", default)]
    pub(super) channel_id: Option<String>,
    #[serde(rename = "guildId", default)]
    pub(super) guild_id: Option<String>,
    #[serde(default)]
    pub(super) attachments: Option<Vec<Value>>,
}

pub(super) fn outbound_target_for(inbound: &QqbotBridgeInboundMessage) -> Result<String, CliError> {
    match inbound.message_type.as_str() {
        "group" => inbound
            .group_openid
            .as_deref()
            .map(|value| format!("qqbot:group:{value}"))
            .ok_or_else(|| {
                CliError::ChannelConnectivity("qqbot group message missing groupOpenid".into())
            }),
        "guild" => inbound
            .channel_id
            .as_deref()
            .map(|value| format!("qqbot:channel:{value}"))
            .ok_or_else(|| {
                CliError::ChannelConnectivity("qqbot guild message missing channelId".into())
            }),
        "c2c" | "dm" => Ok(format!("qqbot:c2c:{}", inbound.sender_id)),
        other => Err(CliError::ChannelConnectivity(format!(
            "unsupported qqbot message type: {other}"
        ))),
    }
}

pub(super) fn next_request_id(kind: &str) -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time")
        .as_nanos();
    format!("qqbot-{kind}-{nanos}")
}

pub(super) fn shorten(input: &str, max_chars: usize) -> String {
    let mut chars = input.chars();
    let shortened = chars.by_ref().take(max_chars).collect::<String>();
    if chars.next().is_some() {
        format!("{shortened}…")
    } else {
        shortened
    }
}

pub(super) fn sanitize_attachment_summaries(
    items: Option<&[Value]>,
) -> Vec<InputAttachmentSummary> {
    items
        .unwrap_or(&[])
        .iter()
        .map(|item| {
            let kind = string_field(item, &["content_type", "contentType", "type"])
                .or_else(|| string_field(item, &["file_type", "fileType"]))
                .unwrap_or_else(|| "attachment".into());
            InputAttachmentSummary {
                attachment_id: string_field(item, &["id", "attachment_id", "attachmentId"]),
                kind,
                content_type: string_field(item, &["content_type", "contentType"]),
                name: string_field(item, &["filename", "file_name", "name", "title"]),
                url: string_field(item, &["url", "file_url", "fileUrl"]),
                local_path: string_field(item, &["local_path", "localPath", "path"]),
                size_bytes: u64_field(item, &["size", "size_bytes", "sizeBytes"]),
                width: u32_field(item, &["width"]),
                height: u32_field(item, &["height"]),
                source: Some("qqbot".into()),
            }
        })
        .collect()
}

pub(super) fn normalize_inbound_content(
    content: &str,
    attachments: &[InputAttachmentSummary],
) -> String {
    let trimmed = content.trim();
    if !trimmed.is_empty() || attachments.is_empty() {
        return trimmed.to_string();
    }
    let named = attachments
        .iter()
        .filter_map(|item| item.name.as_deref())
        .filter(|name| !name.trim().is_empty())
        .take(3)
        .collect::<Vec<_>>();
    if named.is_empty() {
        format!(
            "用户发送了 {} 个附件，没有附加文字。请先确认已收到附件；如果当前只能看到附件元信息，请明确说明。",
            attachments.len()
        )
    } else {
        format!(
            "用户发送了附件，没有附加文字。已收到的附件包括：{}。请先确认已收到附件；如果当前只能看到附件元信息，请明确说明。",
            named.join("、")
        )
    }
}

pub(super) fn sanitize_text_channel_output(text: &str) -> String {
    let mut output = remove_tag_block(text, "fin_control_feedback");
    output = remove_tag_block(&output, "fin_tool_calls");
    output = output
        .replace("<fin_user_response>", "")
        .replace("</fin_user_response>", "")
        .replace("**", "")
        .replace("__", "")
        .replace('`', "");
    let mut lines = Vec::new();
    let mut previous_blank = false;
    for raw in output.lines() {
        let trimmed = raw.trim();
        let cleaned = trimmed
            .strip_prefix("### ")
            .or_else(|| trimmed.strip_prefix("## "))
            .or_else(|| trimmed.strip_prefix("# "))
            .unwrap_or(trimmed);
        if cleaned.is_empty() {
            if !previous_blank {
                lines.push(String::new());
            }
            previous_blank = true;
            continue;
        }
        previous_blank = false;
        lines.push(cleaned.to_string());
    }
    lines.join("\n").trim().to_string()
}

pub(super) fn inbound_ack_text() -> &'static str {
    "已收到，正在处理。"
}

pub(super) fn unbound_session_notice_text() -> &'static str {
    "已收到，但当前通道还没有绑定会话，请先完成配对或恢复会话。"
}

pub(super) fn empty_payload_notice_text() -> &'static str {
    "已收到，但这条消息没有可处理的文本或附件内容。"
}

pub(super) fn no_new_messages_notice_text() -> &'static str {
    "已收到，但本轮没有生成新的可发送回复。"
}

pub(super) fn processing_failed_notice_text(error: &str) -> String {
    let cleaned = shorten(error.trim(), 160);
    if cleaned.is_empty() {
        "处理失败，请稍后重试。".into()
    } else {
        format!("处理失败：{cleaned}")
    }
}

#[derive(Debug, Serialize)]
struct BridgeRequest<'a> {
    action: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    payload: Option<Value>,
    #[serde(rename = "requestId")]
    request_id: String,
}

pub(super) fn write_bridge_request(
    stdin: &Arc<Mutex<Option<ChildStdin>>>,
    action: &str,
    payload: Option<Value>,
) -> Result<(), CliError> {
    let request = BridgeRequest {
        action,
        payload,
        request_id: next_request_id(action),
    };
    let line = serde_json::to_string(&request).map_err(CliError::Serialize)?;
    let mut stdin = stdin
        .lock()
        .map_err(|_| CliError::ChannelConnectivity("qqbot bridge stdin lock poisoned".into()))?;
    let stdin = stdin
        .as_mut()
        .ok_or_else(|| CliError::ChannelConnectivity("qqbot bridge stdin unavailable".into()))?;
    stdin
        .write_all(line.as_bytes())
        .and_then(|_| stdin.write_all(b"\n"))
        .and_then(|_| stdin.flush())
        .map_err(|source| CliError::WriteFile {
            path: "qqbot bridge stdin".into(),
            source,
        })
}

pub(super) fn send_channel_notice(
    runtime_home: &std::path::Path,
    stdin: &Arc<Mutex<Option<ChildStdin>>>,
    target: &str,
    reply_to_id: Option<&str>,
    text: &str,
    notice_kind: &str,
) -> Result<(), CliError> {
    let rendered = sanitize_text_channel_output(text);
    if rendered.trim().is_empty() {
        return Ok(());
    }
    write_bridge_request(
        stdin,
        "send",
        Some(json!({
            "to": target,
            "text": rendered,
            "replyToId": reply_to_id,
        })),
    )?;
    let _ = record_builtin_qqbot_runtime_event(
        runtime_home,
        "channel.peer.notice_send_requested",
        None,
        None,
        None,
        json!({
            "notice_kind": notice_kind,
            "target": target,
            "reply_to_id": reply_to_id,
            "text_preview": shorten(&rendered, 180),
        }),
    );
    Ok(())
}

pub(super) fn deliver_pending_messages_for_target(
    runtime_home: &std::path::Path,
    stdin: &Arc<Mutex<Option<ChildStdin>>>,
    target: &str,
    reply_to_id: Option<&str>,
    source: &str,
) -> Result<usize, CliError> {
    let Some((conversation, pending)) = pending_outbound_messages(runtime_home, target)? else {
        return Ok(0);
    };
    if pending.is_empty() {
        return Ok(0);
    }
    let mut delivered = 0_usize;
    for message in &pending {
        let rendered = sanitize_text_channel_output(&message.content);
        if rendered.trim().is_empty() {
            continue;
        }
        write_bridge_request(
            stdin,
            "send",
            Some(json!({
                "to": target,
                "text": rendered,
                "replyToId": if delivered == 0 { reply_to_id } else { None },
            })),
        )?;
        let _ = mark_delivered_message(
            runtime_home,
            target,
            &message.message_id,
            &message.created_at,
        )?;
        delivered += 1;
        let _ = record_builtin_qqbot_runtime_event(
            runtime_home,
            "channel.peer.session_message_send_requested",
            None,
            None,
            None,
            json!({
                "source": source,
                "target": target,
                "session_id": conversation.session_id,
                "message_id": message.message_id,
                "role": message.role,
                "text_preview": shorten(&rendered, 180),
            }),
        );
    }
    Ok(delivered)
}

pub(super) fn spawn_activity_delivery_loop(
    runtime_home: PathBuf,
    stdin: Arc<Mutex<Option<ChildStdin>>>,
    stop_signal: Arc<AtomicBool>,
    ready_signal: Arc<AtomicBool>,
) -> JoinHandle<()> {
    thread::spawn(move || {
        let mut was_ready = false;
        let mut last_progress_notice: std::collections::HashMap<String, u64> =
            std::collections::HashMap::new();
        while !stop_signal.load(Ordering::SeqCst) {
            let is_ready = ready_signal.load(Ordering::SeqCst);
            let reconnected = is_ready && !was_ready;
            was_ready = is_ready;

            if !is_ready {
                thread::sleep(Duration::from_secs(5));
                continue;
            }
            if stop_signal.load(Ordering::SeqCst) {
                break;
            }

            // On reconnect: send reconnect notice + immediate delivery.
            if reconnected {
                if let Ok(conversations) = list_conversations(&runtime_home) {
                    for conv in &conversations {
                        if conv.session_id.is_some() {
                            let _ = write_bridge_request(
                                &stdin,
                                "send",
                                Some(json!({
                                    "to": conv.target,
                                    "text": "重新连接成功，正在恢复上下文。",
                                })),
                            );
                        }
                    }
                }
            }

            // Deliver pending messages for all conversations.
            if let Err(err) = deliver_pending_messages_for_all(&runtime_home, &stdin) {
                let _ = record_builtin_qqbot_runtime_event(
                    &runtime_home,
                    "channel.peer.pending_delivery_failed",
                    Some("bridge_degraded"),
                    None,
                    None,
                    json!({ "error": err.to_string() }),
                );
            }

            // Progress notices: for conversations with a pending inbound that has
            // not yet received a delivery response, send periodic updates.
            let now_secs = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0);
            if let Ok(conversations) = list_conversations(&runtime_home) {
                for conv in &conversations {
                    if conv.session_id.is_none() {
                        continue;
                    }
                    // Inbound arrived but no delivery has followed since.
                    let inbound_after_delivery =
                        match (&conv.last_inbound_at, &conv.last_delivery_at) {
                            (Some(inbound_at), Some(delivery_at)) => {
                                inbound_at.as_str() > delivery_at.as_str()
                            }
                            (Some(_), None) => true,
                            _ => false,
                        };
                    if !inbound_after_delivery {
                        // No pending response — clear progress tracking.
                        last_progress_notice.remove(&conv.target);
                        continue;
                    }
                    // Check if there are pending outbound messages (response ready).
                    let has_pending_outbound = pending_outbound_messages(&runtime_home, &conv.target)
                        .ok()
                        .flatten()
                        .is_some_and(|(_, pending)| !pending.is_empty());
                    if has_pending_outbound {
                        // Response is ready but not yet delivered; the delivery pass
                        // above will send it — skip progress notice this cycle.
                        last_progress_notice.remove(&conv.target);
                        continue;
                    }
                    let last_notice = last_progress_notice
                        .get(&conv.target)
                        .copied()
                        .unwrap_or(0);
                    if now_secs.saturating_sub(last_notice) >= 15 {
                        let _ = write_bridge_request(
                            &stdin,
                            "send",
                            Some(json!({
                                "to": conv.target,
                                "text": "仍在处理中，请稍候…",
                            })),
                        );
                        let _ = record_builtin_qqbot_runtime_event(
                            &runtime_home,
                            "channel.peer.progress_notice_sent",
                            None,
                            None,
                            None,
                            json!({
                                "target": conv.target,
                                "session_id": conv.session_id,
                            }),
                        );
                        last_progress_notice.insert(conv.target.clone(), now_secs);
                    }
                }
            }
            match prepare_periodic_delivery(&runtime_home) {
                Ok(Some(prepared)) => {
                    let still_current = current_delivery_signature_if_deliverable(&runtime_home)
                        .ok()
                        .flatten()
                        .is_some_and(|signature| signature == prepared.signature);
                    if !still_current {
                        let _ = record_builtin_qqbot_runtime_event(
                            &runtime_home,
                            "channel.peer.activity_card_send_skipped_stale",
                            None,
                            None,
                            None,
                            json!({
                                "target": prepared.target,
                                "reason": prepared.reason,
                            }),
                        );
                        continue;
                    }
                    let payload = json!({
                        "to": prepared.target,
                        "text": prepared.text,
                    });
                    if let Err(err) = write_bridge_request(&stdin, "send", Some(payload)) {
                        let _ = record_builtin_qqbot_runtime_event(
                            &runtime_home,
                            "channel.peer.activity_card_send_failed",
                            Some("bridge_degraded"),
                            None,
                            None,
                            json!({
                                "reason": prepared.reason,
                                "error": err.to_string(),
                            }),
                        );
                        continue;
                    }
                    let _ = mark_delivered(
                        &runtime_home,
                        &prepared.signature,
                        &prepared.text,
                        &prepared.reason,
                    );
                    let _ = record_builtin_qqbot_runtime_event(
                        &runtime_home,
                        "channel.peer.activity_card_send_requested",
                        None,
                        None,
                        None,
                        json!({
                            "target": prepared.target,
                            "reason": prepared.reason,
                            "text_preview": shorten(&prepared.text, 180),
                        }),
                    );
                }
                Ok(None) => {}
                Err(err) => {
                    let _ = record_builtin_qqbot_runtime_event(
                        &runtime_home,
                        "channel.peer.activity_card_prepare_failed",
                        Some("bridge_degraded"),
                        None,
                        None,
                        json!({ "error": err.to_string() }),
                    );
                }
            }
            thread::sleep(Duration::from_secs(5));
        }
    })
}

fn string_field(value: &Value, keys: &[&str]) -> Option<String> {
    keys.iter().find_map(|key| {
        value
            .get(*key)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
    })
}

fn u64_field(value: &Value, keys: &[&str]) -> Option<u64> {
    keys.iter().find_map(|key| {
        value.get(*key).and_then(|field| {
            field
                .as_u64()
                .or_else(|| field.as_str().and_then(|raw| raw.parse::<u64>().ok()))
        })
    })
}

fn u32_field(value: &Value, keys: &[&str]) -> Option<u32> {
    u64_field(value, keys).and_then(|value| u32::try_from(value).ok())
}

fn deliver_pending_messages_for_all(
    runtime_home: &std::path::Path,
    stdin: &Arc<Mutex<Option<ChildStdin>>>,
) -> Result<(), CliError> {
    for conversation in list_conversations(runtime_home)? {
        let _ = deliver_pending_messages_for_target(
            runtime_home,
            stdin,
            &conversation.target,
            None,
            "periodic_scan",
        )?;
    }
    Ok(())
}

fn remove_tag_block(input: &str, tag: &str) -> String {
    let open = format!("<{tag}>");
    let close = format!("</{tag}>");
    let mut output = input.to_string();
    while let Some(start) = output.find(&open) {
        let Some(rel_end) = output[start..].find(&close) else {
            output.replace_range(start.., "");
            break;
        };
        let end = start + rel_end + close.len();
        output.replace_range(start..end, "");
    }
    output
}
