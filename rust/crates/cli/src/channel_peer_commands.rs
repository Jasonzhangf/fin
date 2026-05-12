use crate::{
    CliError,
    channel_peer::{
        complete_builtin_qqbot_pairing, ensure_builtin_qqbot_peer,
        force_expire_builtin_qqbot_session, probe_builtin_qqbot_connectivity,
        record_builtin_qqbot_heartbeat,
    },
    local_command_notice::append_notice_messages,
};
use fin_debug_server::{ChatSendRequest, ChatSendResponse, DebugBinding};
use std::path::Path;

pub(crate) fn try_handle_channel_peer_command(
    runtime_home: &Path,
    user_toml_path: Option<&Path>,
    request: &ChatSendRequest,
    binding: &DebugBinding,
) -> Result<Option<ChatSendResponse>, CliError> {
    let message = request.message.trim();
    if !message.starts_with("/qqbot") {
        return Ok(None);
    }
    let parts = message.split_whitespace().collect::<Vec<_>>();
    let subcommand = parts.get(1).copied().unwrap_or("status");
    let response = match subcommand {
        "status" => handle_status(runtime_home, binding)?,
        "connect" => handle_connect(runtime_home, user_toml_path, binding)?,
        "pair" => handle_pair(runtime_home, binding, &parts[2..])?,
        "heartbeat" => handle_heartbeat(runtime_home, binding)?,
        "expire" => handle_expire(runtime_home, binding, &parts[2..])?,
        _ => usage(binding),
    };
    Ok(Some(response))
}

fn handle_status(
    runtime_home: &Path,
    binding: &DebugBinding,
) -> Result<ChatSendResponse, CliError> {
    let state = ensure_builtin_qqbot_peer(runtime_home)?;
    Ok(build_response(
        binding,
        format!(
            "qqbot status: runtime={} connectivity={} binding={} lifecycle={} pairing_required={} session_valid={} session_id={} reconnects={} credential_source={} last_error={}",
            state.runtime_state,
            state.connectivity_state,
            state.binding_state,
            state.lifecycle_state,
            state.pairing_required,
            state.session_valid,
            state.session_id.unwrap_or_else(|| "-".into()),
            state.reconnect_count,
            state.credential_source.unwrap_or_else(|| "-".into()),
            state.last_connectivity_error.unwrap_or_else(|| "-".into()),
        ),
        "status",
    ))
}

fn handle_connect(
    runtime_home: &Path,
    user_toml_path: Option<&Path>,
    binding: &DebugBinding,
) -> Result<ChatSendResponse, CliError> {
    let state = probe_builtin_qqbot_connectivity(runtime_home, user_toml_path)?;
    let answer = if state.connectivity_state == "connected" {
        format!(
            "qqbot upstream connected: credential_source={} expires_at={}",
            state.credential_source.unwrap_or_else(|| "-".into()),
            state.upstream_expires_at.unwrap_or_else(|| "-".into())
        )
    } else {
        format!(
            "qqbot upstream not connected: connectivity={} credential_source={} error={}",
            state.connectivity_state,
            state.credential_source.unwrap_or_else(|| "-".into()),
            state.last_connectivity_error.unwrap_or_else(|| "-".into())
        )
    };
    Ok(build_response(binding, answer, "connect"))
}

fn handle_pair(
    runtime_home: &Path,
    binding: &DebugBinding,
    args: &[&str],
) -> Result<ChatSendResponse, CliError> {
    let session_id = args
        .first()
        .map(|value| value.to_string())
        .or_else(|| binding.session_id.clone());
    let Some(session_id) = session_id else {
        return Ok(build_response(
            binding,
            "usage: /qqbot pair [session_id] [ttl_minutes(optional)]".into(),
            "usage",
        ));
    };
    let ttl_minutes = args.get(1).and_then(|value| value.parse::<u64>().ok());
    let state = complete_builtin_qqbot_pairing(runtime_home, &session_id, ttl_minutes)?;
    maybe_append_binding_notice(
        binding,
        "/qqbot pair",
        &match state.session_ttl_minutes {
            Some(ttl) => format!("qqbot paired with session {} (ttl={}m)", session_id, ttl),
            None => format!("qqbot paired with session {} (persistent)", session_id),
        },
    )?;
    Ok(build_response(
        binding,
        match state.session_expires_at {
            Some(expires_at) => {
                format!(
                    "qqbot paired: session_id={} expires_at={}",
                    session_id, expires_at
                )
            }
            None => format!(
                "qqbot paired: session_id={} expires_at=persistent",
                session_id
            ),
        },
        "pair",
    ))
}

fn handle_heartbeat(
    runtime_home: &Path,
    binding: &DebugBinding,
) -> Result<ChatSendResponse, CliError> {
    let state = record_builtin_qqbot_heartbeat(runtime_home)?;
    Ok(build_response(
        binding,
        format!(
            "qqbot heartbeat recorded: last_heartbeat_at={}",
            state.last_heartbeat_at.unwrap_or_else(|| "-".into())
        ),
        "heartbeat",
    ))
}

fn handle_expire(
    runtime_home: &Path,
    binding: &DebugBinding,
    args: &[&str],
) -> Result<ChatSendResponse, CliError> {
    let reason = if args.is_empty() {
        "manual-expire".to_string()
    } else {
        args.join(" ")
    };
    let state = force_expire_builtin_qqbot_session(runtime_home, reason.as_str())?;
    maybe_append_binding_notice(
        binding,
        "/qqbot expire",
        &format!("qqbot session expired, repair required: {}", reason),
    )?;
    Ok(build_response(
        binding,
        format!(
            "qqbot expired: runtime={} connectivity={} binding={} lifecycle={} pairing_required={} reconnects={}",
            state.runtime_state,
            state.connectivity_state,
            state.binding_state,
            state.lifecycle_state,
            state.pairing_required,
            state.reconnect_count
        ),
        "expire",
    ))
}

fn maybe_append_binding_notice(
    binding: &DebugBinding,
    command: &str,
    notice: &str,
) -> Result<(), CliError> {
    let Some(path) = binding.session_messages_path.as_deref() else {
        return Ok(());
    };
    let Some(session_id) = binding.session_id.as_deref() else {
        return Ok(());
    };
    let message_path = Path::new(&binding.runtime_home).join(path);
    append_notice_messages(
        &message_path,
        session_id,
        binding.task_id.as_deref(),
        command,
        notice,
    )
}

fn usage(binding: &DebugBinding) -> ChatSendResponse {
    build_response(
        binding,
        "usage: /qqbot <status|connect|pair|heartbeat|expire>".into(),
        "usage",
    )
}

fn build_response(binding: &DebugBinding, answer: String, suffix: &str) -> ChatSendResponse {
    ChatSendResponse {
        binding: binding.clone(),
        answer,
        digest_id: format!("digest-local-command-qqbot-{suffix}"),
        events_count: 0,
        response_kind: "system_notice".into(),
        freshness: Some("instant".into()),
        control_feedback: None,
        progress: None,
        note: None,
        routing_action: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        fs,
        sync::atomic::{AtomicU64, Ordering},
        time::{SystemTime, UNIX_EPOCH},
    };

    static TEMP_HOME_SEQ: AtomicU64 = AtomicU64::new(1);

    fn temp_runtime_home() -> std::path::PathBuf {
        let seq = TEMP_HOME_SEQ.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!(
            "fin-qqbot-command-{}-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("time")
                .as_nanos(),
            seq,
        ))
    }

    fn binding(home: &Path) -> DebugBinding {
        let session_rel = "sessions/2026/05/session-qq/conversation/messages.json".to_string();
        let full = home.join(&session_rel);
        fs::create_dir_all(full.parent().expect("parent")).expect("mkdir");
        fs::write(&full, b"[]").expect("messages");
        DebugBinding {
            project_id: "fin".into(),
            project_label: "fin".into(),
            runtime_home: home.display().to_string(),
            session_id: Some("session-qq".into()),
            task_id: Some("task-qq".into()),
            session_messages_path: Some(session_rel),
            recent_contexts_path: None,
            recent_digests_path: None,
        }
    }

    #[test]
    fn qqbot_pair_command_updates_state_and_writes_session_notice() {
        let home = temp_runtime_home();
        fs::create_dir_all(&home).expect("home");
        let result = try_handle_channel_peer_command(
            &home,
            None,
            &ChatSendRequest {
                message: "/qqbot pair".into(),
                input_kind: None,
                attachments: Vec::new(),
            },
            &binding(&home),
        )
        .expect("command ok")
        .expect("handled");
        assert!(result.answer.contains("qqbot paired"));
        assert!(result.answer.contains("persistent"));
        let messages =
            fs::read_to_string(home.join("sessions/2026/05/session-qq/conversation/messages.json"))
                .expect("messages");
        assert!(messages.contains("/qqbot pair"));
        assert!(messages.contains("qqbot paired with session session-qq"));
        assert!(messages.contains("persistent"));
    }

    #[test]
    fn qqbot_expire_command_releases_binding_without_pairing_required() {
        let home = temp_runtime_home();
        fs::create_dir_all(&home).expect("home");
        let bind = binding(&home);
        let _ = try_handle_channel_peer_command(
            &home,
            None,
            &ChatSendRequest {
                message: "/qqbot pair".into(),
                input_kind: None,
                attachments: Vec::new(),
            },
            &bind,
        )
        .expect("pair ok");
        let result = try_handle_channel_peer_command(
            &home,
            None,
            &ChatSendRequest {
                message: "/qqbot expire test-reason".into(),
                input_kind: None,
                attachments: Vec::new(),
            },
            &bind,
        )
        .expect("expire ok")
        .expect("handled");
        assert!(result.answer.contains("binding=unbound"));
        assert!(result.answer.contains("pairing_required=false"));
    }
}
