use super::{deliver_pending_messages_for_target, shorten, write_bridge_request};
use crate::{
    CliError,
    channel_peer::record_builtin_qqbot_runtime_event,
    channel_peer_activity_delivery::{
        current_delivery_signature_if_deliverable, mark_delivered, prepare_periodic_delivery,
    },
    channel_peer_conversations::{list_conversations, pending_outbound_messages},
};
use serde_json::json;
use std::{
    path::PathBuf,
    process::ChildStdin,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread::{self, JoinHandle},
    time::Duration,
};

pub(crate) fn spawn_activity_delivery_loop(
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

            if reconnected {
                send_reconnect_notices(&runtime_home, &stdin);
            }

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

            maybe_send_progress_notices(&runtime_home, &stdin, &mut last_progress_notice);
            maybe_send_activity_card(&runtime_home, &stdin);
            thread::sleep(Duration::from_secs(5));
        }
    })
}

fn send_reconnect_notices(runtime_home: &std::path::Path, stdin: &Arc<Mutex<Option<ChildStdin>>>) {
    if let Ok(conversations) = list_conversations(runtime_home) {
        for conv in &conversations {
            if conv.session_id.is_some() {
                let _ = write_bridge_request(
                    stdin,
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

fn maybe_send_progress_notices(
    runtime_home: &std::path::Path,
    stdin: &Arc<Mutex<Option<ChildStdin>>>,
    last_progress_notice: &mut std::collections::HashMap<String, u64>,
) {
    let now_secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    if let Ok(conversations) = list_conversations(runtime_home) {
        for conv in &conversations {
            if conv.session_id.is_none() {
                continue;
            }
            let inbound_after_delivery = match (&conv.last_inbound_at, &conv.last_delivery_at) {
                (Some(inbound_at), Some(delivery_at)) => inbound_at.as_str() > delivery_at.as_str(),
                (Some(_), None) => true,
                _ => false,
            };
            if !inbound_after_delivery {
                last_progress_notice.remove(&conv.target);
                continue;
            }
            let has_pending_outbound = pending_outbound_messages(runtime_home, &conv.target)
                .ok()
                .flatten()
                .is_some_and(|(_, pending)| !pending.is_empty());
            if has_pending_outbound {
                last_progress_notice.remove(&conv.target);
                continue;
            }
            let last_notice = last_progress_notice.get(&conv.target).copied().unwrap_or(0);
            if now_secs.saturating_sub(last_notice) >= 15 {
                let _ = write_bridge_request(
                    stdin,
                    "send",
                    Some(json!({"to": conv.target, "text": "仍在处理中，请稍候…"})),
                );
                let _ = record_builtin_qqbot_runtime_event(
                    runtime_home,
                    "channel.peer.progress_notice_sent",
                    None,
                    None,
                    None,
                    json!({"target": conv.target, "session_id": conv.session_id}),
                );
                last_progress_notice.insert(conv.target.clone(), now_secs);
            }
        }
    }
}

fn maybe_send_activity_card(
    runtime_home: &std::path::Path,
    stdin: &Arc<Mutex<Option<ChildStdin>>>,
) {
    match prepare_periodic_delivery(runtime_home) {
        Ok(Some(prepared)) => {
            let still_current = current_delivery_signature_if_deliverable(runtime_home)
                .ok()
                .flatten()
                .is_some_and(|signature| signature == prepared.signature);
            if !still_current {
                let _ = record_builtin_qqbot_runtime_event(
                    runtime_home,
                    "channel.peer.activity_card_send_skipped_stale",
                    None,
                    None,
                    None,
                    json!({
                        "target": prepared.target,
                        "reason": prepared.reason,
                    }),
                );
                return;
            }
            let payload = json!({
                "to": prepared.target,
                "text": prepared.text,
            });
            if let Err(err) = write_bridge_request(stdin, "send", Some(payload)) {
                let _ = record_builtin_qqbot_runtime_event(
                    runtime_home,
                    "channel.peer.activity_card_send_failed",
                    Some("bridge_degraded"),
                    None,
                    None,
                    json!({
                        "reason": prepared.reason,
                        "error": err.to_string(),
                    }),
                );
                return;
            }
            let _ = mark_delivered(
                runtime_home,
                &prepared.signature,
                &prepared.text,
                &prepared.reason,
            );
            let _ = record_builtin_qqbot_runtime_event(
                runtime_home,
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
                runtime_home,
                "channel.peer.activity_card_prepare_failed",
                Some("bridge_degraded"),
                None,
                None,
                json!({ "error": err.to_string() }),
            );
        }
    }
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
