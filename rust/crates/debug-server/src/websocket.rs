use crate::{
    ChatSendRequest, DebugActionHandler, DebugDataError, HttpRequest, header_value, session_view,
};
use base64::{Engine as _, engine::general_purpose};
use serde_json::{Value, json};
use sha1::{Digest, Sha1};
use std::{fs, io::Write, net::TcpStream, path::Path, sync::mpsc, thread, time::Duration};

#[path = "websocket_frame.rs"]
mod frame;
use frame::{read_frame, write_close_frame, write_text_frame};
#[path = "websocket_mobile_items.rs"]
mod mobile_items;
use mobile_items::{mobile_tool_item_frame, mobile_tool_records};

const WS_GUID: &str = "258EAFA5-E914-47DA-95CA-C5AB0DC85B11";

pub(crate) fn handle_ws_connection(
    stream: &mut TcpStream,
    request: &HttpRequest,
    runtime_home: &Path,
    handler: &(impl DebugActionHandler + Sync),
) -> Result<(), DebugDataError> {
    let Some(accept_key) = websocket_accept_key(request) else {
        return write_bad_ws_request(stream);
    };
    stream
        .set_read_timeout(None)
        .map_err(|source| DebugDataError::Io {
            path: "websocket-clear-read-timeout".into(),
            source,
        })?;
    write_ws_handshake(stream, &accept_key)?;
    while let Some(frame) = read_frame(stream)? {
        if frame.opcode == 0x8 {
            write_close_frame(stream)?;
            return Ok(());
        }
        if frame.opcode != 0x1 {
            continue;
        }
        let text = String::from_utf8_lossy(&frame.payload);
        if let Some(user_input) = parse_user_input_message(&text) {
            handle_user_input_streaming(stream, user_input, runtime_home, handler)?;
        } else {
            let responses = handle_text_message(&text, runtime_home, handler);
            for response in responses {
                write_text_frame(stream, &response)?;
            }
        }
    }
    Ok(())
}

pub(crate) fn websocket_accept_value(key: &str) -> String {
    let mut hasher = Sha1::new();
    hasher.update(key.trim().as_bytes());
    hasher.update(WS_GUID.as_bytes());
    general_purpose::STANDARD.encode(hasher.finalize())
}

fn websocket_accept_key(request: &HttpRequest) -> Option<String> {
    let upgrade = header_value(request, "Upgrade")?;
    let connection = header_value(request, "Connection")?;
    let key = header_value(request, "Sec-WebSocket-Key")?;
    (upgrade.eq_ignore_ascii_case("websocket")
        && connection
            .split(',')
            .any(|part| part.trim().eq_ignore_ascii_case("upgrade"))
        && !key.trim().is_empty())
    .then(|| websocket_accept_value(key))
}

fn write_ws_handshake(stream: &mut TcpStream, accept_key: &str) -> Result<(), DebugDataError> {
    let header = format!(
        "HTTP/1.1 101 Switching Protocols\r\n\
         Upgrade: websocket\r\n\
         Connection: Upgrade\r\n\
         Sec-WebSocket-Accept: {accept_key}\r\n\r\n"
    );
    stream
        .write_all(header.as_bytes())
        .and_then(|_| stream.flush())
        .map_err(|source| DebugDataError::Io {
            path: "websocket-handshake-write".into(),
            source,
        })
}

fn write_bad_ws_request(stream: &mut TcpStream) -> Result<(), DebugDataError> {
    stream
        .write_all(
            b"HTTP/1.1 400 Bad Request\r\nContent-Length: 22\r\nConnection: close\r\n\r\nbad websocket request\n",
        )
        .and_then(|_| stream.flush())
        .map_err(|source| DebugDataError::Io {
            path: "websocket-bad-request-write".into(),
            source,
        })
}

fn handle_text_message(
    text: &str,
    runtime_home: &Path,
    handler: &(impl DebugActionHandler + Sync),
) -> Vec<String> {
    let message = match serde_json::from_str::<Value>(text) {
        Ok(value) => value,
        Err(error) => {
            return vec![json!({"type":"protocol.error","reason":error.to_string()}).to_string()];
        }
    };
    match message.get("type").and_then(Value::as_str).unwrap_or("") {
        "mobile.handshake" => vec![json!({"type":"handshake.ok"}).to_string()],
        "mobile.subscribe" => session_snapshot(runtime_home, handler),
        "session.bind" => vec![
            json!({
                "type":"session.bound",
                "session_id": message.get("session_id").and_then(Value::as_str).unwrap_or("")
            })
            .to_string(),
        ],
        "session.user_input" => handle_user_input_blocking(message, runtime_home, handler),
        other => {
            vec![json!({"type":"protocol.error","message_type":other,"reason":"unsupported_message"})
                .to_string()]
        }
    }
}

fn session_snapshot(
    runtime_home: &Path,
    handler: &(impl DebugActionHandler + Sync),
) -> Vec<String> {
    let binding = handler.read_binding(runtime_home).ok();
    let session_id = binding.as_ref().and_then(|value| value.session_id.clone());
    let sessions = session_id
        .as_ref()
        .map(|id| vec![json!({"session_id":id,"title":id,"project":"fin","archived":false})])
        .unwrap_or_default();
    let history = read_session_history(runtime_home).unwrap_or_default();
    let config_snapshot = handler
        .read_config_snapshot(runtime_home)
        .unwrap_or_else(|reason| {
            json!({
                "type": "config.snapshot",
                "status": "error",
                "reason": reason,
                "default_profile": "",
                "profiles": []
            })
        });
    let provider_health = provider_health_from_config_snapshot(&config_snapshot);
    vec![
        json!({"type":"session.list","sessions":sessions}).to_string(),
        json!({"type":"runtime.health","status":"ok"}).to_string(),
        config_snapshot.to_string(),
        provider_health.to_string(),
        json!({"type":"session.history","session_id":session_id,"turns":history}).to_string(),
    ]
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ParsedUserInput {
    payload: String,
    client_message_id: String,
}

fn parse_user_input_message(text: &str) -> Option<Result<ParsedUserInput, String>> {
    let message = match serde_json::from_str::<Value>(text) {
        Ok(value) => value,
        Err(error) => return Some(Err(error.to_string())),
    };
    if message.get("type").and_then(Value::as_str) != Some("session.user_input") {
        return None;
    }
    let payload = user_input_payload(&message);
    if payload.is_empty() {
        return Some(Err("empty_payload".into()));
    }
    let client_message_id = message
        .get("client_message_id")
        .and_then(Value::as_str)
        .unwrap_or("mobile-message")
        .to_string();
    Some(Ok(ParsedUserInput {
        payload,
        client_message_id,
    }))
}

fn handle_user_input_streaming(
    stream: &mut TcpStream,
    parsed: Result<ParsedUserInput, String>,
    runtime_home: &Path,
    handler: &(impl DebugActionHandler + Sync),
) -> Result<(), DebugDataError> {
    let parsed = match parsed {
        Ok(value) => value,
        Err(reason) => {
            write_text_frame(
                stream,
                &json!({"type":"protocol.error","reason":reason}).to_string(),
            )?;
            return Ok(());
        }
    };
    let turn_id = format!("turn-{}", parsed.client_message_id);
    let accepted = json!({
        "type": "input.accepted",
        "client_message_id": parsed.client_message_id
    });
    let started = json!({
        "type": "turn.started",
        "client_message_id": parsed.client_message_id,
        "turn_id": turn_id
    });
    let progress = json!({
        "type": "turn.progress",
        "client_message_id": parsed.client_message_id,
        "turn_id": turn_id,
        "phase": "inference_waiting"
    });
    write_text_frame(stream, &accepted.to_string())?;
    write_text_frame(stream, &started.to_string())?;
    write_text_frame(stream, &progress.to_string())?;

    let runtime_home = runtime_home.to_path_buf();
    let payload = parsed.payload.clone();
    let client_message_id = parsed.client_message_id.clone();
    let (tx, rx) = mpsc::channel();
    thread::scope(|scope| {
        scope.spawn(|| {
            let result = handler.send_chat_message(
                &runtime_home,
                ChatSendRequest {
                    message: payload.clone(),
                    input_kind: None,
                    attachments: Vec::new(),
                },
            );
            let _ = tx.send(render_user_input_result(
                &runtime_home,
                &client_message_id,
                &payload,
                result,
                true,
            ));
        });
        loop {
            match rx.recv_timeout(Duration::from_millis(200)) {
                Ok(frames) => {
                    for frame in frames {
                        write_text_frame(stream, &frame)?;
                    }
                    break;
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    let heartbeat = json!({
                        "type": "turn.progress",
                        "client_message_id": parsed.client_message_id,
                        "turn_id": turn_id,
                        "phase": "provider_wait"
                    });
                    write_text_frame(stream, &heartbeat.to_string())?;
                }
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    let frames = render_user_input_result(
                        &runtime_home,
                        &parsed.client_message_id,
                        &parsed.payload,
                        Err("chat handler disconnected".into()),
                        true,
                    );
                    for frame in frames {
                        write_text_frame(stream, &frame)?;
                    }
                    break;
                }
            }
        }
        Ok(())
    })
}

fn handle_user_input_blocking(
    message: Value,
    runtime_home: &Path,
    handler: &(impl DebugActionHandler + Sync),
) -> Vec<String> {
    let payload = user_input_payload(&message);
    let client_message_id = message
        .get("client_message_id")
        .and_then(Value::as_str)
        .unwrap_or("mobile-message");
    if payload.is_empty() {
        return vec![json!({"type":"protocol.error","reason":"empty_payload"}).to_string()];
    }
    let result = handler.send_chat_message(
        runtime_home,
        ChatSendRequest {
            message: payload.clone(),
            input_kind: None,
            attachments: Vec::new(),
        },
    );
    let mut frames = vec![
        json!({
            "type":"input.accepted",
            "client_message_id":client_message_id
        })
        .to_string(),
    ];
    frames.extend(render_user_input_result(
        runtime_home,
        client_message_id,
        &payload,
        result,
        false,
    ));
    frames
}

fn user_input_payload(message: &Value) -> String {
    message
        .get("payload")
        .or_else(|| message.get("content"))
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim()
        .to_string()
}

fn render_user_input_result(
    runtime_home: &Path,
    client_message_id: &str,
    payload: &str,
    result: Result<crate::ChatSendResponse, String>,
    include_completed: bool,
) -> Vec<String> {
    let turn_id = format!("turn-{client_message_id}");
    let status = if result.is_ok() {
        "completed"
    } else {
        "failed"
    };
    let (item_frames, rendered) = match result {
        Ok(response) => {
            let tool_records = mobile_tool_records(runtime_home).unwrap_or_default();
            let item_frames = tool_records
                .iter()
                .map(|record| mobile_tool_item_frame(client_message_id, &turn_id, record))
                .collect::<Vec<_>>();
            let rendered = json!({
            "type":"turn.rendered",
            "client_message_id":client_message_id,
            "turn_id": turn_id,
            "user_input": payload,
            "assistant_response": response.answer,
            "control_feedback_summary": response.response_kind,
            "tool_execution_summary": "",
            "closure_stop_source": "websocket",
            "tool_execution_records": tool_records,
            "error_records": []
            })
            .to_string();
            (item_frames, rendered)
        }
        Err(error) => {
            let rendered = json!({
            "type":"turn.rendered",
            "client_message_id":client_message_id,
            "turn_id": turn_id,
            "user_input": payload,
            "assistant_response": format!("执行失败：{error}"),
            "control_feedback_summary": "error",
            "tool_execution_summary": "",
            "closure_stop_source": "websocket",
            "tool_execution_records": [],
            "error_records": [{
                "item_id": format!("error-{client_message_id}"),
                "item_kind": "runtime_error",
                "label": "runtime.error",
                "title": "Runtime Error",
                "purpose": "Expose failed user input execution",
                "status": "failed",
                "error_summary": error
            }]
            })
            .to_string();
            (Vec::new(), rendered)
        }
    };
    if include_completed {
        let mut frames = item_frames;
        frames.extend([
            json!({
                "type": "turn.completed",
                "client_message_id": client_message_id,
                "turn_id": turn_id,
                "status": status
            })
            .to_string(),
            rendered,
        ]);
        frames
    } else {
        let mut frames = item_frames;
        frames.push(rendered);
        frames
    }
}

fn provider_health_from_config_snapshot(snapshot: &Value) -> Value {
    let default_profile = snapshot
        .get("default_profile")
        .and_then(Value::as_str)
        .unwrap_or("");
    let profiles = snapshot
        .get("profiles")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let active = profiles
        .iter()
        .find(|profile| profile.get("active").and_then(Value::as_bool) == Some(true))
        .or_else(|| {
            profiles.iter().find(|profile| {
                profile
                    .get("profile_name")
                    .or_else(|| profile.get("provider"))
                    .and_then(Value::as_str)
                    == Some(default_profile)
            })
        });
    let provider = active
        .and_then(|profile| profile.get("provider").and_then(Value::as_str))
        .unwrap_or(default_profile);
    let model = active
        .and_then(|profile| profile.get("model").and_then(Value::as_str))
        .unwrap_or("");
    let status = if provider.is_empty() || model.is_empty() {
        "schema_error"
    } else {
        "ok"
    };
    json!({
        "type": "provider.health",
        "status": status,
        "provider": provider,
        "model": model
    })
}

fn read_session_history(runtime_home: &Path) -> Result<Vec<Value>, DebugDataError> {
    let Some(path) =
        session_view::last_run_artifact_path(runtime_home, "session_recent_turns_path")?
    else {
        return Ok(Vec::new());
    };
    let body = fs::read_to_string(&path).map_err(|source| DebugDataError::Io {
        path: path.display().to_string(),
        source,
    })?;
    serde_json::from_str(&body).map_err(DebugDataError::Serialize)
}

#[cfg(test)]
#[path = "websocket_mobile_items_tests.rs"]
mod mobile_items_tests;
#[cfg(test)]
#[path = "websocket_unit_tests.rs"]
mod tests;
