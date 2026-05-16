use crate::chat_api::ChatSendRequest;
use crate::session_view::{last_run_artifact_path, read_json_value, read_last_run_json};
use crate::{DebugActionHandler, DebugDataError};
use serde_json::{Value, json};
use std::{collections::HashSet, net::TcpStream, path::Path};
use tungstenite::{Message, WebSocket, accept};

pub(crate) fn handle_mobile_ws(
    stream: TcpStream,
    runtime_home: &Path,
    handler: &(impl DebugActionHandler + Sync),
) -> Result<(), DebugDataError> {
    let mut ws = accept(stream).map_err(|source| DebugDataError::Io {
        path: "ws-accept".into(),
        source: std::io::Error::other(source.to_string()),
    })?;
    run_mobile_loop(&mut ws, runtime_home, handler)
}

fn run_mobile_loop(
    ws: &mut WebSocket<TcpStream>,
    runtime_home: &Path,
    handler: &(impl DebugActionHandler + Sync),
) -> Result<(), DebugDataError> {
    let mut handshake_ok = false;
    loop {
        let msg = ws.read_message().map_err(|source| DebugDataError::Io {
            path: "ws-read".into(),
            source: std::io::Error::other(source.to_string()),
        })?;
        if !msg.is_text() {
            continue;
        }
        let payload = msg.to_text().unwrap_or_default();
        let Ok(v) = serde_json::from_str::<Value>(payload) else {
            send_json(ws, &json!({"type":"handshake.protocol_mismatch","reason":"invalid_json"}))?;
            continue;
        };
        let t = v.get("type").and_then(Value::as_str).unwrap_or_default();
        match t {
            "mobile.handshake" => {
                let project = v.get("project").and_then(Value::as_str).unwrap_or_default();
                if project != "fin" {
                    send_json(
                        ws,
                        &json!({"type":"handshake.protocol_mismatch","reason":"project_mismatch"}),
                    )?;
                    continue;
                }
                handshake_ok = true;
                send_json(ws, &json!({"type":"handshake.ok"}))?;
            }
            "mobile.subscribe" => {
                if !handshake_ok {
                    send_json(ws, &json!({"type":"handshake.auth_failed"}))?;
                    continue;
                }
                send_session_list(ws, runtime_home)?;
                send_runtime_views(ws, runtime_home)?;
                send_session_history(ws, runtime_home, None)?;
            }
            "session.bind" => {
                let session_id = v
                    .get("session_id")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string();
                send_json(ws, &json!({"type":"session.bound","session_id":session_id}))?;
                send_session_history(ws, runtime_home, Some(session_id.as_str()))?;
            }
            "session.user_input" => {
                if !handshake_ok {
                    send_json(ws, &json!({"type":"handshake.auth_failed"}))?;
                    continue;
                }
                let text = v
                    .get("payload")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .trim()
                    .to_string();
                if text.is_empty() {
                    continue;
                }
                let client_message_id = v
                    .get("client_message_id")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string();
                send_json(
                    ws,
                    &json!({
                        "type":"input.accepted",
                        "client_message_id": client_message_id,
                        "session_id": v.get("session_id").and_then(Value::as_str).unwrap_or_default(),
                    }),
                )?;
                send_json(
                    ws,
                    &json!({
                        "type":"turn.progress",
                        "client_message_id": client_message_id,
                        "phase":"inference_waiting",
                    }),
                )?;

                let result = handler
                    .send_chat_message(
                        runtime_home,
                        ChatSendRequest {
                            message: text.clone(),
                            input_kind: Some("mobile_user_input".into()),
                            attachments: Vec::new(),
                        },
                    )
                    .map_err(|err| DebugDataError::Io {
                        path: "chat-send".into(),
                        source: std::io::Error::other(err),
                    })?;
                let (tool_execution_records, error_records) =
                    collect_turn_tool_records(runtime_home).unwrap_or_default();
                send_json(
                    ws,
                    &json!({
                        "type":"turn.rendered",
                        "client_message_id": client_message_id,
                        "user_input":text,
                        "assistant_response":result.answer,
                        "control_feedback_summary": result.control_feedback.as_ref().map(|f| f.reason.clone()).unwrap_or_default(),
                        "tool_execution_summary": tool_execution_records.iter().map(tool_record_summary).collect::<Vec<String>>().join(" | "),
                        "tool_execution_records": tool_execution_records,
                        "error_records": error_records,
                        "closure_stop_source": result.response_kind,
                    }),
                )?;
            }
            _ => {
                send_json(
                    ws,
                    &json!({"type":"handshake.protocol_mismatch","reason":"unknown_message_type"}),
                )?;
            }
        }
    }
}

fn collect_turn_tool_records(runtime_home: &Path) -> Result<(Vec<Value>, Vec<Value>), DebugDataError> {
    let turn_path = runtime_home.join("runtime/current/current_turn.json");
    if !turn_path.exists() {
        return Ok((Vec::new(), Vec::new()));
    }
    let turn = read_json_value(&turn_path)?;
    let refs = turn
        .get("tool_record_refs")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let ref_ids: HashSet<String> = refs
        .into_iter()
        .filter_map(|value| value.as_str().map(extract_tool_call_id))
        .collect();
    if ref_ids.is_empty() {
        return Ok((Vec::new(), Vec::new()));
    }

    let last_run = read_last_run_json(runtime_home)?;
    let Some(records_rel) = last_run
        .get("session_recent_tool_records_path")
        .and_then(Value::as_str)
    else {
        return Ok((Vec::new(), Vec::new()));
    };
    let records_path = runtime_home.join(records_rel);
    if !records_path.exists() {
        return Ok((Vec::new(), Vec::new()));
    }
    let records = read_json_value(&records_path)?;
    let mut matched = Vec::new();
    if let Some(list) = records.as_array() {
        for item in list {
            let id = item
                .get("tool_call_id")
                .and_then(Value::as_str)
                .unwrap_or_default();
            if ref_ids.contains(id) {
                matched.push(item.clone());
            }
        }
    }
    let errors = matched
        .iter()
        .filter(|record| {
            record
                .get("status")
                .and_then(Value::as_str)
                .map(|status| status != "completed")
                .unwrap_or(false)
                || record
                    .get("error_summary")
                    .and_then(Value::as_str)
                    .map(|value| !value.trim().is_empty())
                    .unwrap_or(false)
        })
        .cloned()
        .collect::<Vec<_>>();
    Ok((matched, errors))
}

fn extract_tool_call_id(reference: &str) -> String {
    reference
        .split("#tool_call_id=")
        .nth(1)
        .unwrap_or(reference)
        .to_string()
}

fn tool_record_summary(record: &Value) -> String {
    let name = record
        .get("tool_name")
        .and_then(Value::as_str)
        .unwrap_or("tool");
    let status = record
        .get("status")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    let err = record
        .get("error_summary")
        .and_then(Value::as_str)
        .unwrap_or("");
    if err.is_empty() {
        format!("{name}:{status}")
    } else {
        format!("{name}:{status}:{err}")
    }
}

fn send_json(ws: &mut WebSocket<TcpStream>, value: &Value) -> Result<(), DebugDataError> {
    ws.write_message(Message::Text(value.to_string()))
        .map_err(|source| DebugDataError::Io {
            path: "ws-write".into(),
            source: std::io::Error::other(source.to_string()),
        })
}

fn send_session_list(ws: &mut WebSocket<TcpStream>, runtime_home: &Path) -> Result<(), DebugDataError> {
    let last_run = read_json_value(&runtime_home.join("runtime/current/last_run.json"))?;
    let session_id = last_run
        .get("session_id")
        .and_then(Value::as_str)
        .unwrap_or("session-unknown");
    let task_id = last_run
        .get("task_id")
        .and_then(Value::as_str)
        .unwrap_or("task-unknown");
    let updated_at = last_run
        .get("completed_at")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    send_json(
        ws,
        &json!({
            "type":"session.list",
            "sessions":[{
                "session_id":session_id,
                "task_id":task_id,
                "topic":"fin",
                "phase":"ready",
                "project":"fin",
                "updated_at":updated_at
            }]
        }),
    )
}

fn send_runtime_views(ws: &mut WebSocket<TcpStream>, runtime_home: &Path) -> Result<(), DebugDataError> {
    let execution = last_run_artifact_path(runtime_home, "current_execution_state_path")?
        .and_then(|p| read_json_value(&p).ok())
        .unwrap_or_else(|| json!({}));
    send_json(
        ws,
        &json!({"type":"runtime.workers","workers":[{"worker_id":"entry","phase":execution.get("phase").and_then(Value::as_str).unwrap_or("idle")}]}),
    )?;
    send_json(
        ws,
        &json!({"type":"runtime.projects","projects":[{"project_id":"fin","state":"attached"}]}),
    )?;
    send_json(
        ws,
        &json!({"type":"runtime.daemon","daemon":{"state":"running"}}),
    )
}

fn send_session_history(
    ws: &mut WebSocket<TcpStream>,
    runtime_home: &Path,
    session_id: Option<&str>,
) -> Result<(), DebugDataError> {
    let last_run = read_last_run_json(runtime_home)?;
    let last_session_id = last_run
        .get("session_id")
        .and_then(Value::as_str)
        .unwrap_or("session-unknown");
    if let Some(id) = session_id {
        if id != last_session_id {
            return send_json(
                ws,
                &json!({"type":"session.history","session_id":id,"turns":[]}),
            );
        }
    }
    let Some(msg_path_rel) = last_run
        .get("session_messages_path")
        .and_then(Value::as_str)
    else {
        return send_json(
            ws,
            &json!({"type":"session.history","session_id":last_session_id,"turns":[]}),
        );
    };
    let msg_path = runtime_home.join(msg_path_rel);
    if !msg_path.exists() {
        return send_json(
            ws,
            &json!({"type":"session.history","session_id":last_session_id,"turns":[]}),
        );
    }
    let raw = read_json_value(&msg_path)?;
    let mut turns: Vec<Value> = Vec::new();
    let mut pending_user: Option<String> = None;
    if let Some(items) = raw.as_array() {
        for item in items {
            let role = item.get("role").and_then(Value::as_str).unwrap_or_default();
            let content = item
                .get("content")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string();
            match role {
                "user" => {
                    if let Some(u) = pending_user.take() {
                        turns.push(json!({"user_input":u,"assistant_response":""}));
                    }
                    pending_user = Some(content);
                }
                "assistant" => {
                    let u = pending_user.take().unwrap_or_default();
                    turns.push(json!({"user_input":u,"assistant_response":content}));
                }
                _ => {}
            }
        }
    }
    if let Some(u) = pending_user.take() {
        turns.push(json!({"user_input":u,"assistant_response":""}));
    }
    send_json(
        ws,
        &json!({
            "type":"session.history",
            "session_id":last_session_id,
            "turns":turns
        }),
    )
}
