use crate::{
    CliError,
    channel_peer::{
        active_builtin_qqbot_session, complete_builtin_qqbot_pairing,
        record_builtin_qqbot_runtime_event,
    },
    channel_peer_activity_delivery::{bind_target, clear_target},
    channel_peer_connectivity::resolve_qqbot_credentials,
    channel_peer_conversations::upsert_inbound_conversation,
    process_utils::append_log,
    web_debug::CliDebugActionHandler,
    web_debug_entry::build_channel_peer_request,
};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{
    path::Path,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread::JoinHandle,
};

#[path = "channel_peer_qqbot_bridge_runtime.rs"]
mod runtime_support;
#[path = "channel_peer_qqbot_bridge_support.rs"]
mod support;
use runtime_support::{ensure_runner_entry, spawn_bridge_supervisor};
use support::{
    QqbotBridgeInboundMessage, deliver_pending_messages_for_target, empty_payload_notice_text,
    inbound_ack_text, no_new_messages_notice_text, normalize_inbound_content, outbound_target_for,
    processing_failed_notice_text, sanitize_attachment_summaries, send_channel_notice, shorten,
    spawn_activity_delivery_loop, unbound_session_notice_text, write_bridge_request,
};

const QQBOT_CHANNEL_ID: &str = "qqbot";
const QQBOT_RUNNER_SOURCE: &str = include_str!("../assets/qqbot_peer_runner.mjs");

#[derive(Debug)]
pub(crate) struct BuiltinQqbotBridge {
    stdin: Arc<Mutex<Option<std::process::ChildStdin>>>,
    stop_signal: Arc<AtomicBool>,
    ready_signal: Arc<AtomicBool>,
    supervisor_thread: Option<JoinHandle<()>>,
    activity_thread: Option<JoinHandle<()>>,
}

#[derive(Debug, Deserialize)]
struct BridgeResponse {
    ok: bool,
    #[serde(default)]
    error: Option<String>,
    #[serde(rename = "requestId", default)]
    request_id: Option<String>,
}

#[derive(Debug, Deserialize)]
struct BridgeEventEnvelope {
    event: String,
    data: Value,
}

impl BuiltinQqbotBridge {
    pub(crate) fn start(
        runtime_home: &Path,
        handler: CliDebugActionHandler,
        user_toml_path: Option<&Path>,
    ) -> Result<Self, CliError> {
        let runner_entry = ensure_runner_entry(runtime_home, QQBOT_RUNNER_SOURCE)?;
        let credentials = resolve_qqbot_credentials(user_toml_path).ok_or_else(|| {
            CliError::ChannelConnectivity(
                "missing qqbot credentials for built-in peer start".into(),
            )
        })?;
        let stdin = Arc::new(Mutex::new(None));
        let stop_signal = Arc::new(AtomicBool::new(false));
        let ready_signal = Arc::new(AtomicBool::new(false));
        let supervisor_thread = Some(spawn_bridge_supervisor(
            runtime_home.to_path_buf(),
            handler,
            runner_entry,
            credentials,
            stdin.clone(),
            stop_signal.clone(),
            ready_signal.clone(),
        ));
        let activity_thread = Some(spawn_activity_delivery_loop(
            runtime_home.to_path_buf(),
            stdin.clone(),
            stop_signal.clone(),
            ready_signal.clone(),
        ));
        Ok(Self {
            stdin,
            stop_signal,
            ready_signal,
            supervisor_thread,
            activity_thread,
        })
    }
}

impl Drop for BuiltinQqbotBridge {
    fn drop(&mut self) {
        self.stop_signal.store(true, Ordering::SeqCst);
        self.ready_signal.store(false, Ordering::SeqCst);
        let _ = write_bridge_request(&self.stdin, "stop", None);
        if let Some(handle) = self.supervisor_thread.take() {
            let _ = handle.join();
        }
        if let Some(handle) = self.activity_thread.take() {
            let _ = handle.join();
        }
    }
}

fn handle_stdout_line(
    runtime_home: &Path,
    handler: &CliDebugActionHandler,
    stdin: &Arc<Mutex<Option<std::process::ChildStdin>>>,
    ready_signal: &Arc<AtomicBool>,
    line: &str,
) -> Result<(), CliError> {
    if let Ok(event) = serde_json::from_str::<BridgeEventEnvelope>(line) {
        return handle_bridge_event(runtime_home, handler, stdin, ready_signal, event);
    }
    if let Ok(response) = serde_json::from_str::<BridgeResponse>(line) {
        return handle_bridge_response(runtime_home, response);
    }
    append_log(
        &runtime_home.join("runtime/peers/qqbot/bridge.stdout.log"),
        &(line.to_string() + "\n"),
    )?;
    Ok(())
}

fn handle_bridge_response(runtime_home: &Path, response: BridgeResponse) -> Result<(), CliError> {
    if response.ok {
        if let Some(request_id) = response
            .request_id
            .as_deref()
            .filter(|id| id.starts_with("qqbot-send-"))
        {
            record_builtin_qqbot_runtime_event(
                runtime_home,
                "channel.peer.message_sent",
                Some("bridge_ready"),
                Some("connected"),
                None,
                json!({ "request_id": request_id }),
            )?;
        }
        return Ok(());
    }
    record_builtin_qqbot_runtime_event(
        runtime_home,
        "channel.peer.bridge_response_error",
        Some("bridge_error"),
        None,
        None,
        json!({
            "request_id": response.request_id,
            "error": response.error.unwrap_or_else(|| "unknown bridge error".into()),
        }),
    )
    .map(|_| ())
}

fn handle_bridge_event(
    runtime_home: &Path,
    handler: &CliDebugActionHandler,
    stdin: &Arc<Mutex<Option<std::process::ChildStdin>>>,
    ready_signal: &Arc<AtomicBool>,
    event: BridgeEventEnvelope,
) -> Result<(), CliError> {
    match event.event.as_str() {
        "ready" => {
            ready_signal.store(true, Ordering::SeqCst);
            record_builtin_qqbot_runtime_event(
                runtime_home,
                "channel.peer.bridge_ready",
                Some("bridge_ready"),
                Some("connected"),
                None,
                event.data,
            )?;
        }
        "disconnected" => {
            ready_signal.store(false, Ordering::SeqCst);
            record_builtin_qqbot_runtime_event(
                runtime_home,
                "channel.peer.bridge_disconnected",
                Some("bridge_disconnected"),
                Some("connecting"),
                None,
                event.data,
            )?;
        }
        "stopped" => {
            record_builtin_qqbot_runtime_event(
                runtime_home,
                "channel.peer.bridge_stopped",
                Some("bridge_stopped"),
                Some("local_only"),
                None,
                event.data,
            )?;
        }
        "error" => {
            record_builtin_qqbot_runtime_event(
                runtime_home,
                "channel.peer.bridge_error",
                Some("bridge_error"),
                Some("degraded"),
                None,
                event.data,
            )?;
        }
        "message" => {
            let inbound: QqbotBridgeInboundMessage =
                serde_json::from_value(event.data).map_err(CliError::Serialize)?;
            if let Err(err) = process_inbound_message(runtime_home, handler, stdin, inbound.clone())
            {
                let target = outbound_target_for(&inbound).ok();
                if let Some(target) = target.as_deref() {
                    let _ = send_channel_notice(
                        runtime_home,
                        stdin,
                        target,
                        Some(&inbound.message_id),
                        &processing_failed_notice_text(&err.to_string()),
                        "processing_failed",
                    );
                }
                record_builtin_qqbot_runtime_event(
                    runtime_home,
                    "channel.peer.message_processing_failed",
                    Some("bridge_degraded"),
                    None,
                    None,
                    json!({
                        "message_id": inbound.message_id,
                        "target": target,
                        "error": err.to_string(),
                    }),
                )?;
            }
        }
        other => {
            record_builtin_qqbot_runtime_event(
                runtime_home,
                "channel.peer.bridge_unknown_event",
                None,
                None,
                None,
                json!({ "event": other, "data": event.data }),
            )?;
        }
    }
    Ok(())
}

fn process_inbound_message(
    runtime_home: &Path,
    handler: &CliDebugActionHandler,
    stdin: &Arc<Mutex<Option<std::process::ChildStdin>>>,
    inbound: QqbotBridgeInboundMessage,
) -> Result<(), CliError> {
    let target = outbound_target_for(&inbound)?;
    record_builtin_qqbot_runtime_event(
        runtime_home,
        "channel.peer.message_ingested",
        None,
        None,
        None,
        json!({
            "target": target,
            "message_id": inbound.message_id,
            "message_type": inbound.message_type,
            "sender_id": inbound.sender_id,
            "sender_name": inbound.sender_name,
            "group_openid": inbound.group_openid,
            "channel_id": inbound.channel_id,
            "guild_id": inbound.guild_id,
            "content_preview": shorten(inbound.content.trim(), 160),
            "attachment_count": inbound.attachments.as_ref().map(|items| items.len()).unwrap_or(0),
            "timestamp": inbound.timestamp,
        }),
    )?;
    let default_session = active_builtin_qqbot_session(runtime_home)?;
    let resolution = upsert_inbound_conversation(
        runtime_home,
        &target,
        &inbound.message_id,
        Some(&inbound.timestamp),
        default_session.as_deref(),
    )?;
    if resolution.duplicate_inbound {
        record_builtin_qqbot_runtime_event(
            runtime_home,
            "channel.peer.message_duplicate_ignored",
            None,
            None,
            None,
            json!({ "target": target, "message_id": inbound.message_id }),
        )?;
        return Ok(());
    }
    let _ = send_channel_notice(
        runtime_home,
        stdin,
        &target,
        Some(&inbound.message_id),
        inbound_ack_text(),
        "received_ack",
    );
    if resolution.session_assigned || resolution.session_restored {
        record_builtin_qqbot_runtime_event(
            runtime_home,
            "channel.peer.session_restored",
            None,
            None,
            None,
            json!({
                "target": target,
                "session_id": resolution.record.session_id,
                "restored": resolution.session_restored,
                "assigned": resolution.session_assigned,
            }),
        )?;
    }
    let Some(session_id) = resolution.record.session_id.clone() else {
        let _ = clear_target(runtime_home);
        record_builtin_qqbot_runtime_event(
            runtime_home,
            "channel.peer.message_rejected",
            None,
            None,
            Some("pairing_required"),
            json!({
                "reason": "qqbot target is not bound to a session",
                "target": target,
                "message_id": inbound.message_id,
            }),
        )?;
        let _ = send_channel_notice(
            runtime_home,
            stdin,
            &target,
            Some(&inbound.message_id),
            unbound_session_notice_text(),
            "pairing_required",
        );
        return Ok(());
    };
    let _ = complete_builtin_qqbot_pairing(runtime_home, &session_id, None);
    let attachments = sanitize_attachment_summaries(inbound.attachments.as_deref());
    let content = normalize_inbound_content(&inbound.content, &attachments);
    if content.is_empty() {
        record_builtin_qqbot_runtime_event(
            runtime_home,
            "channel.peer.message_rejected",
            None,
            None,
            None,
            json!({
                "reason": "empty_text_payload",
                "target": target,
                "message_id": inbound.message_id,
                "attachment_count": inbound.attachments.as_ref().map(|items| items.len()).unwrap_or(0),
            }),
        )?;
        let _ = send_channel_notice(
            runtime_home,
            stdin,
            &target,
            Some(&inbound.message_id),
            empty_payload_notice_text(),
            "empty_payload",
        );
        return Ok(());
    }
    let _ = bind_target(runtime_home, &session_id, &target);
    let binding = handler.resolve_session_binding(runtime_home, &session_id)?;
    let _ = handler.send_chat_message_on_binding(
        runtime_home,
        binding,
        build_channel_peer_request(content, attachments),
    )?;
    let delivered = deliver_pending_messages_for_target(
        runtime_home,
        stdin,
        &target,
        Some(&inbound.message_id),
        "inbound_request",
    )?;
    if delivered == 0 {
        record_builtin_qqbot_runtime_event(
            runtime_home,
            "channel.peer.delivery_pending_no_new_messages",
            None,
            None,
            None,
            json!({ "target": target, "session_id": session_id, "message_id": inbound.message_id }),
        )?;
        let _ = send_channel_notice(
            runtime_home,
            stdin,
            &target,
            Some(&inbound.message_id),
            no_new_messages_notice_text(),
            "no_new_messages",
        );
    }
    Ok(())
}

#[cfg(test)]
#[path = "channel_peer_qqbot_bridge_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "channel_peer_qqbot_bridge_e2e_tests.rs"]
mod e2e_tests;
