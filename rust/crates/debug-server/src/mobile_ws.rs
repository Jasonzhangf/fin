use crate::chat_api::ChatSendRequest;
use crate::mobile_blocks::{
    find_session_dir, list_sessions, runtime_view_messages, session_ids_from_value,
    session_is_visible,
};
use crate::mobile_protocol::unknown_mobile_message;
use crate::session_view::{last_run_artifact_path, read_json_value, read_last_run_json};
use crate::{DebugActionHandler, DebugDataError};
use fin_config::{ConfigMapper, SystemConfig, parse_system_toml, parse_user_toml, system_to_toml};
use fin_contracts::{
    ActivityCardsSnapshot, LedgerTrackKind, ProviderTarget, SessionSnapshotRecord,
};
use fin_provider::{InferenceProvider, ProviderError, ProviderFacade, ProviderRequest};
use fin_runtime::{LedgerStore, build_activity_cards};
use serde_json::{Value, json};
use std::{
    collections::{HashMap, HashSet},
    fs,
    net::TcpStream,
    path::{Path, PathBuf},
    sync::mpsc,
    thread,
    time::{Duration, Instant, UNIX_EPOCH},
};
use tungstenite::{Message, WebSocket, accept};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
struct MobileHostConfig {
    #[serde(default)]
    thinking_effort: Option<String>,
}

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
            send_json(
                ws,
                &json!({"type":"handshake.protocol_mismatch","reason":"invalid_json"}),
            )?;
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
                send_config_snapshot(ws, runtime_home)?;
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
            "session.rename" => {
                if !handshake_ok {
                    send_json(ws, &json!({"type":"handshake.auth_failed"}))?;
                    continue;
                }
                let session_id = v
                    .get("session_id")
                    .and_then(Value::as_str)
                    .unwrap_or_default();
                let title = v.get("title").and_then(Value::as_str).unwrap_or_default();
                if let Err(err) =
                    update_session_meta(runtime_home, session_id, Some(title), None, false)
                {
                    send_session_operation_failed(ws, "rename", &[session_id.to_string()], err)?;
                    continue;
                }
                send_json(
                    ws,
                    &json!({"type":"session.operation.ok","operation":"rename","session_id":session_id}),
                )?;
                send_session_list(ws, runtime_home)?;
            }
            "session.archive" => {
                if !handshake_ok {
                    send_json(ws, &json!({"type":"handshake.auth_failed"}))?;
                    continue;
                }
                let session_ids = session_ids_from_value(&v);
                if let Err(err) =
                    update_session_metas(runtime_home, &session_ids, None, Some(true), false)
                {
                    send_session_operation_failed(ws, "archive", &session_ids, err)?;
                    continue;
                }
                send_json(
                    ws,
                    &json!({"type":"session.operation.ok","operation":"archive","session_ids":session_ids}),
                )?;
                send_session_list(ws, runtime_home)?;
            }
            "session.unarchive" => {
                if !handshake_ok {
                    send_json(ws, &json!({"type":"handshake.auth_failed"}))?;
                    continue;
                }
                let session_ids = session_ids_from_value(&v);
                if let Err(err) =
                    update_session_metas(runtime_home, &session_ids, None, Some(false), false)
                {
                    send_session_operation_failed(ws, "unarchive", &session_ids, err)?;
                    continue;
                }
                send_json(
                    ws,
                    &json!({"type":"session.operation.ok","operation":"unarchive","session_ids":session_ids}),
                )?;
                send_session_list(ws, runtime_home)?;
            }
            "session.delete" => {
                if !handshake_ok {
                    send_json(ws, &json!({"type":"handshake.auth_failed"}))?;
                    continue;
                }
                let session_ids = session_ids_from_value(&v);
                if let Err(err) =
                    update_session_metas(runtime_home, &session_ids, None, Some(true), true)
                {
                    send_session_operation_failed(ws, "delete", &session_ids, err)?;
                    continue;
                }
                send_json(
                    ws,
                    &json!({"type":"session.operation.ok","operation":"delete","session_ids":session_ids}),
                )?;
                send_session_list(ws, runtime_home)?;
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
                let session_id = v
                    .get("session_id")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string();
                let provisional_turn_id = mobile_provisional_turn_id(&client_message_id);
                send_json(
                    ws,
                    &json!({
                        "type":"turn.started",
                        "session_id": session_id,
                        "client_message_id": client_message_id,
                        "turn_id": provisional_turn_id,
                        "status":"running",
                    }),
                )?;
                send_json(
                    ws,
                    &json!({
                        "type":"turn.progress",
                        "session_id": session_id,
                        "client_message_id": client_message_id,
                        "turn_id": provisional_turn_id,
                        "phase":"inference_waiting",
                    }),
                )?;

                let request = ChatSendRequest {
                    message: text.clone(),
                    input_kind: Some("mobile_user_input".into()),
                    attachments: Vec::new(),
                };
                let (tx, rx) = mpsc::channel();
                thread::scope(|scope| -> Result<(), DebugDataError> {
                    scope.spawn(|| {
                        let result = handler
                            .resolve_session_binding(runtime_home, &session_id)
                            .and_then(|binding| {
                                handler.send_chat_message_on_binding(runtime_home, binding, request)
                            })
                            .map_err(|err| DebugDataError::Io {
                                path: "chat-send".into(),
                                source: std::io::Error::other(err),
                            });
                        let _ = tx.send(result);
                    });
                    let mut seen_tool_ids: HashSet<String> = HashSet::new();
                    let mut seen_error_keys: HashSet<String> = HashSet::new();
                    let mut item_started: HashSet<String> = HashSet::new();
                    let mut item_delta_signatures: HashMap<String, String> = HashMap::new();
                    let mut last_phase = String::new();
                    let mut last_activity_cards_signature: Option<String> = None;
                    loop {
                        match rx.recv_timeout(Duration::from_millis(180)) {
                            Ok(done) => {
                                let result = match done {
                                    Ok(value) => value,
                                    Err(err) => {
                                        let error_summary = err.to_string();
                                        let runtime_record = json!({
                                            "tool_call_id": format!("runtime-error-{}", stable_id_fragment(&client_message_id)),
                                            "tool_name":"runtime.closure",
                                            "tool_kind":"runtime",
                                            "title":"Runtime closure failed",
                                            "purpose":"execute local runtime turn closure",
                                            "status":"failed",
                                            "started_at": now_epoch_ms_string(),
                                            "duration_ms": 0,
                                            "error_summary": error_summary,
                                        });
                                        let item_events = mobile_item_events_from_record(
                                            &runtime_record,
                                            &session_id,
                                            &client_message_id,
                                            None,
                                            &provisional_turn_id,
                                        );
                                        for event in &item_events {
                                            send_json(ws, event)?;
                                        }
                                        send_provider_health_if_provider_error(
                                            ws,
                                            &session_id,
                                            &client_message_id,
                                            &json!({"tool_name":"provider.call","tool_kind":"framework_tool","target_kind":"provider","error_summary":error_summary}),
                                        )?;
                                        // legacy turn.error_event removed — turn.item.* events emitted above
                                        send_json(
                                            ws,
                                            &json!({
                                                "type":"turn.trace_event",
                                                "session_id": session_id,
                                                "client_message_id": client_message_id,
                                                "turn_id": provisional_turn_id,
                                                "trace":"closure_error",
                                            }),
                                        )?;
                                        send_json(
                                            ws,
                                            &json!({
                                                "type":"turn.completed",
                                                "session_id": session_id,
                                                "client_message_id": client_message_id,
                                                "turn_id": provisional_turn_id,
                                                "status":"failed",
                                                "error_summary": error_summary,
                                            }),
                                        )?;
                                        send_json(
                                            ws,
                                            &json!({
                                                "type":"turn.rendered",
                                                "session_id": session_id,
                                                "client_message_id": client_message_id,
                                                "turn_id": provisional_turn_id,
                                                "user_input":text,
                                                "assistant_response":"",
                                                "control_feedback_summary":"",
                                                "tool_execution_summary":"runtime.closure:failed",
                                                "tool_execution_records":[],
                                                "error_records":mobile_project_records(&[runtime_record.clone()]),
                                                "closure_stop_source":"error",
                                            }),
                                        )?;
                                        send_activity_cards_snapshot_if_changed(
                                            ws,
                                            runtime_home,
                                            &mut last_activity_cards_signature,
                                        )?;
                                        break;
                                    }
                                };
                                let (tool_execution_records, error_records) =
                                    collect_turn_tool_records(runtime_home, &session_id)
                                        .unwrap_or_default();
                                let final_turn_id = current_turn_id(runtime_home)
                                    .unwrap_or_else(|| provisional_turn_id.clone());
                                for record in &tool_execution_records {
                                    let id = mobile_record_item_id(record);
                                    if !id.is_empty() {
                                        seen_tool_ids.insert(id.clone());
                                    }
                                    let events = mobile_item_events_from_record(
                                        record,
                                        &session_id,
                                        &client_message_id,
                                        None,
                                        &final_turn_id,
                                    );
                                    for event in events {
                                        if event["type"] == "turn.item.started" {
                                            if let Some(item_id) = event["item_id"].as_str() {
                                                item_started.insert(item_id.to_string());
                                            }
                                        }
                                        send_json(ws, &event)?;
                                    }
                                    if !id.is_empty() {
                                        let signature = mobile_record_delta_signature(record, None);
                                        item_delta_signatures.insert(id, signature);
                                    }
                                }
                                for err in &error_records {
                                    send_provider_health_if_provider_error(
                                        ws,
                                        &session_id,
                                        &client_message_id,
                                        err,
                                    )?;
                                    let err_id = mobile_record_item_id(err);
                                    if !err_id.is_empty() {
                                        let signature =
                                            mobile_record_delta_signature(err, Some("failed"));
                                        item_delta_signatures.insert(err_id, signature);
                                    }
                                }
                                send_json(
                                    ws,
                                    &json!({
                                        "type":"turn.trace_event",
                                        "session_id": session_id,
                                        "client_message_id": client_message_id,
                                        "turn_id": final_turn_id,
                                        "trace":"closure_ready",
                                        "tool_count":tool_execution_records.len(),
                                        "error_count":error_records.len(),
                                    }),
                                )?;
                                send_json(
                                    ws,
                                    &json!({
                                        "type":"turn.completed",
                                        "session_id": session_id,
                                        "client_message_id": client_message_id,
                                        "turn_id": final_turn_id,
                                        "status":"completed",
                                        "item_count": tool_execution_records.len(),
                                        "error_count": error_records.len(),
                                    }),
                                )?;
                                let mobile_tool_records =
                                    mobile_project_records(&tool_execution_records);
                                let mobile_error_records = mobile_project_records(&error_records);
                                send_json(
                                    ws,
                                    &json!({
                                        "type":"turn.rendered",
                                        "session_id": session_id,
                                        "client_message_id": client_message_id,
                                        "turn_id": final_turn_id,
                                        "user_input":text,
                                        "assistant_response":result.answer,
                                        "control_feedback_summary": result.control_feedback.as_ref().map(|f| f.reason.clone()).unwrap_or_default(),
                                        "tool_execution_summary": mobile_tool_records.iter().map(tool_record_summary).collect::<Vec<String>>().join(" | "),
                                        "tool_execution_records": mobile_tool_records,
                                        "error_records": mobile_error_records,
                                        "closure_stop_source": result.response_kind,
                                    }),
                                )?;
                                send_activity_cards_snapshot_if_changed(
                                    ws,
                                    runtime_home,
                                    &mut last_activity_cards_signature,
                                )?;
                                break;
                            }
                            Err(mpsc::RecvTimeoutError::Timeout) => {
                                emit_incremental_mobile_events(
                                    ws,
                                    runtime_home,
                                    &session_id,
                                    &client_message_id,
                                    &mut seen_tool_ids,
                                    &mut seen_error_keys,
                                    &mut item_started,
                                    &mut item_delta_signatures,
                                    &mut last_phase,
                                    &mut last_activity_cards_signature,
                                )?;
                            }
                            Err(mpsc::RecvTimeoutError::Disconnected) => {
                                return Err(DebugDataError::Io {
                                    path: "chat-send-join".into(),
                                    source: std::io::Error::other("chat worker disconnected"),
                                });
                            }
                        }
                    }
                    Ok(())
                })?;
            }
            "session.command" => {
                if !handshake_ok {
                    send_json(ws, &json!({"type":"handshake.auth_failed"}))?;
                    continue;
                }
                let command = v
                    .get("command")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .trim()
                    .to_string();
                if command.is_empty() {
                    continue;
                }
                let result = handler
                    .send_chat_message(
                        runtime_home,
                        ChatSendRequest {
                            message: command.clone(),
                            input_kind: Some("mobile_user_input".into()),
                            attachments: Vec::new(),
                        },
                    )
                    .map_err(|err| DebugDataError::Io {
                        path: "chat-send-command".into(),
                        source: std::io::Error::other(err),
                    })?;
                send_json(
                    ws,
                    &json!({
                        "type":"session.command.result",
                        "command": command,
                        "answer": result.answer,
                    }),
                )?;
                send_session_list(ws, runtime_home)?;
            }
            "config.test.request" => {
                if !handshake_ok {
                    send_json(ws, &json!({"type":"handshake.auth_failed"}))?;
                    continue;
                }
                let profile_name = v
                    .get("profile_name")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .trim()
                    .to_string();
                let provider = v
                    .get("provider")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .trim()
                    .to_string();
                let model = v
                    .get("model")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .trim()
                    .to_string();
                let effort = v
                    .get("thinking_effort")
                    .and_then(Value::as_str)
                    .unwrap_or("medium")
                    .trim()
                    .to_string();
                send_json(
                    ws,
                    &json!({"type":"config.test.started","profile_name":profile_name,"provider":provider,"model":model,"thinking_effort":effort}),
                )?;
                let start = Instant::now();
                let result =
                    test_model_config(runtime_home, &profile_name, &provider, &model, &effort);
                let latency_ms = start.elapsed().as_millis() as u64;
                send_json(
                    ws,
                    &json!({
                        "type":"config.test.finished",
                        "ok": result.0,
                        "status": result.1,
                        "error_code": result.2,
                        "error_message": result.3,
                        "latency_ms": latency_ms,
                        "profile_name": profile_name,
                        "provider": provider,
                        "model": model,
                        "thinking_effort": effort,
                    }),
                )?;
            }
            "config.save.request" => {
                if !handshake_ok {
                    send_json(ws, &json!({"type":"handshake.auth_failed"}))?;
                    continue;
                }
                let profile_name = v
                    .get("profile_name")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .trim()
                    .to_string();
                let provider = v
                    .get("provider")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .trim()
                    .to_string();
                let model = v
                    .get("model")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .trim()
                    .to_string();
                let effort = v
                    .get("thinking_effort")
                    .and_then(Value::as_str)
                    .unwrap_or("medium")
                    .trim()
                    .to_string();
                let test = v.get("test_result").cloned().unwrap_or_else(|| json!({}));
                let tested_ok = test.get("ok").and_then(Value::as_bool).unwrap_or(false);
                let same = test
                    .get("profile_name")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    == profile_name
                    && test
                        .get("provider")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        == provider
                    && test
                        .get("model")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        == model
                    && test
                        .get("thinking_effort")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        == effort;
                send_json(
                    ws,
                    &json!({"type":"config.save.started","profile_name":profile_name,"provider":provider,"model":model,"thinking_effort":effort}),
                )?;
                if !tested_ok {
                    send_json(
                        ws,
                        &json!({"type":"config.save.rejected","reason":"test_failed","error_code":"TEST_REQUIRED","error_message":"test did not pass"}),
                    )?;
                    continue;
                }
                if !same {
                    send_json(
                        ws,
                        &json!({"type":"config.save.rejected","reason":"stale_test","error_code":"STALE_TEST","error_message":"config changed after test"}),
                    )?;
                    continue;
                }
                match save_model_config_to_host(
                    runtime_home,
                    &profile_name,
                    &provider,
                    &model,
                    &effort,
                ) {
                    Ok(applied) => {
                        send_json(
                            ws,
                            &json!({
                                "type":"config.save.finished",
                                "ok":true,
                                "status":"applied",
                                "applied_config":applied
                            }),
                        )?;
                        send_config_snapshot(ws, runtime_home)?;
                    }
                    Err(err) => {
                        send_json(
                            ws,
                            &json!({"type":"config.save.rejected","reason":"daemon_error","error_code":"SAVE_FAILED","error_message":err.to_string()}),
                        )?;
                    }
                }
            }
            _ => {
                send_unknown_mobile_message(ws, t, handshake_ok)?;
            }
        }
    }
}

fn send_unknown_mobile_message(
    ws: &mut WebSocket<TcpStream>,
    kind: &str,
    handshake_ok: bool,
) -> Result<(), DebugDataError> {
    send_json(ws, &unknown_mobile_message(kind, handshake_ok))
}

fn send_session_operation_failed(
    ws: &mut WebSocket<TcpStream>,
    operation: &str,
    session_ids: &[String],
    err: DebugDataError,
) -> Result<(), DebugDataError> {
    send_json(
        ws,
        &json!({
            "type":"session.operation.failed",
            "operation":operation,
            "session_ids":session_ids,
            "error":err.to_string()
        }),
    )
}

fn test_model_config(
    runtime_home: &Path,
    profile_name: &str,
    provider: &str,
    model: &str,
    _effort: &str,
) -> (bool, String, Option<String>, Option<String>) {
    if profile_name.is_empty() && (provider.is_empty() || model.is_empty()) {
        return (
            false,
            "invalid".into(),
            Some("INVALID_INPUT".into()),
            Some("profile_name or provider/model cannot be empty".into()),
        );
    }
    let (resolved, test_model) =
        match resolve_selected_provider(runtime_home, profile_name, provider, model) {
            Ok(value) => value,
            Err((status, code, message)) => {
                return (false, status, code, Some(message));
            }
        };
    let facade = ProviderFacade::from_resolved(&resolved);
    let prompt = "Reply with exactly OK.";
    let prepared = facade.prepare_request(&ProviderRequest {
        input: prompt.into(),
        rendered_input: Some(prompt.into()),
        override_model: Some(test_model.clone()),
        prompt_cache_key: Some("mobile-provider-smoke".into()),
    });
    match facade.execute_prepared(&prepared) {
        Ok(response) => (
            true,
            "ok".into(),
            None,
            Some(format!(
                "provider={} model={} status={} response_id={}",
                response.provider_name,
                response.model,
                response.status,
                response.response_id.unwrap_or_else(|| "-".into())
            )),
        ),
        Err(err) => {
            let (status, code, message) = classify_provider_error(err);
            (false, status, Some(code), Some(message))
        }
    }
}

fn save_model_config_to_host(
    runtime_home: &Path,
    profile_name: &str,
    provider: &str,
    model: &str,
    effort: &str,
) -> Result<Value, DebugDataError> {
    let user_path = runtime_home.join("config/user.toml");
    let system_path = runtime_home.join("config/system.toml");
    let user_content = fs::read_to_string(&user_path).map_err(|source| DebugDataError::Io {
        path: user_path.display().to_string(),
        source,
    })?;
    let mut user = parse_user_toml(&user_content).map_err(|err| DebugDataError::Io {
        path: user_path.display().to_string(),
        source: std::io::Error::other(err.to_string()),
    })?;
    let content = fs::read_to_string(&system_path).map_err(|source| DebugDataError::Io {
        path: system_path.display().to_string(),
        source,
    })?;
    let mut sys = parse_system_toml(&content).map_err(|err| DebugDataError::Io {
        path: system_path.display().to_string(),
        source: std::io::Error::other(err.to_string()),
    })?;
    let target = if !profile_name.is_empty() {
        profile_name
    } else {
        provider
    };
    if !sys.providers.contains_key(target) || !user.providers.contains_key(target) {
        return Err(DebugDataError::Io {
            path: system_path.display().to_string(),
            source: std::io::Error::other("provider not found"),
        });
    }
    let selected_model = if model.trim().is_empty() {
        user.providers
            .get(target)
            .map(|cfg| cfg.model.clone())
            .unwrap_or_default()
    } else {
        model.trim().to_string()
    };
    if let Some(cfg) = user.providers.get_mut(target) {
        cfg.model = selected_model.clone();
    }
    user.default_provider = target.to_string();
    fs::write(
        &user_path,
        toml::to_string_pretty(&user).map_err(|err| DebugDataError::Io {
            path: user_path.display().to_string(),
            source: std::io::Error::other(err.to_string()),
        })?,
    )
    .map_err(|source| DebugDataError::Io {
        path: user_path.display().to_string(),
        source,
    })?;
    sys.default_provider = target.to_string();
    if let Some(cfg) = sys.providers.get_mut(target) {
        cfg.model = selected_model.clone();
    }
    for role in sys.policy.roles.values_mut() {
        role.provider_path.targets = vec![ProviderTarget {
            provider_name: target.to_string(),
            model: selected_model.clone(),
        }];
    }
    let remapped = ConfigMapper::map_user_to_system(&user).map_err(|err| DebugDataError::Io {
        path: user_path.display().to_string(),
        source: std::io::Error::other(err.to_string()),
    })?;
    sys.providers = remapped.providers;
    if let Some(cfg) = sys.providers.get_mut(target) {
        cfg.model = selected_model.clone();
    }
    let toml = system_to_toml(&sys).map_err(|err| DebugDataError::Io {
        path: system_path.display().to_string(),
        source: std::io::Error::other(err.to_string()),
    })?;
    fs::write(&system_path, toml).map_err(|source| DebugDataError::Io {
        path: system_path.display().to_string(),
        source,
    })?;
    write_mobile_host_config(
        runtime_home,
        &MobileHostConfig {
            thinking_effort: Some(normalize_thinking_effort(effort)),
        },
    )?;
    let applied = sys.providers.get(target).cloned();
    Ok(json!({
        "profile_name":target,
        "provider":applied.as_ref().map(|v| v.name.clone()).unwrap_or_else(|| target.to_string()),
        "model":applied.as_ref().map(|v| v.model.clone()).unwrap_or_else(|| selected_model.clone()),
        "thinking_effort": normalize_thinking_effort(effort)
    }))
}

fn send_config_snapshot(
    ws: &mut WebSocket<TcpStream>,
    runtime_home: &Path,
) -> Result<(), DebugDataError> {
    let sys = read_effective_system_config(runtime_home)?;
    let profiles = sys
        .providers
        .values()
        .map(|p| {
            let credential_source = match &p.credential {
                fin_config::ProviderCredential::DirectApiKey { .. } => "direct_api_key",
                fin_config::ProviderCredential::ApiKeyEnv { env_var } => env_var.as_str(),
            };
            json!({
                "profile_name": p.name,
                "provider": p.name,
                "protocol": format!("{:?}", p.protocol),
                "base_url": p.base_url,
                "model": p.model,
                "credential_source": credential_source,
                "active": sys.default_provider == p.name,
            })
        })
        .collect::<Vec<_>>();
    let host_cfg = read_mobile_host_config(runtime_home)?;
    send_json(
        ws,
        &json!({
            "type":"config.snapshot",
            "default_profile":sys.default_provider,
            "profiles":profiles,
            "active_thinking_effort": host_cfg.thinking_effort.unwrap_or_else(|| "medium".into())
        }),
    )
}

fn resolve_selected_provider(
    runtime_home: &Path,
    profile_name: &str,
    provider: &str,
    model: &str,
) -> Result<(fin_config::ResolvedProviderConfig, String), (String, Option<String>, String)> {
    let sys = read_effective_system_config(runtime_home).map_err(|err| {
        (
            "invalid".into(),
            Some("EFFECTIVE_CONFIG_INVALID".into()),
            err.to_string(),
        )
    })?;
    let target = if !profile_name.trim().is_empty() {
        profile_name.trim()
    } else {
        provider.trim()
    };
    let Some(base) = sys.providers.get(target).cloned() else {
        return Err((
            "not_found".into(),
            Some(if !profile_name.trim().is_empty() {
                "PROFILE_NOT_FOUND".into()
            } else {
                "PROVIDER_NOT_FOUND".into()
            }),
            format!("profile/provider '{}' not found", target),
        ));
    };
    let selected_model = if model.trim().is_empty() {
        base.model.clone()
    } else {
        model.trim().to_string()
    };
    let mut resolved = base;
    resolved.model = selected_model.clone();
    Ok((resolved, selected_model))
}

fn read_effective_system_config(runtime_home: &Path) -> Result<SystemConfig, DebugDataError> {
    let user_path = runtime_home.join("config/user.toml");
    let user_content = fs::read_to_string(&user_path).map_err(|source| DebugDataError::Io {
        path: user_path.display().to_string(),
        source,
    })?;
    let user = parse_user_toml(&user_content).map_err(|err| DebugDataError::Io {
        path: user_path.display().to_string(),
        source: std::io::Error::other(err.to_string()),
    })?;
    let mapped = ConfigMapper::map_user_to_system(&user).map_err(|err| DebugDataError::Io {
        path: user_path.display().to_string(),
        source: std::io::Error::other(err.to_string()),
    })?;
    let system_path = runtime_home.join("config/system.toml");
    match fs::read_to_string(&system_path) {
        Ok(content) => {
            let existing = parse_system_toml(&content).map_err(|err| DebugDataError::Io {
                path: system_path.display().to_string(),
                source: std::io::Error::other(err.to_string()),
            })?;
            ConfigMapper::merge_user_layer(mapped, existing).map_err(|err| DebugDataError::Io {
                path: system_path.display().to_string(),
                source: std::io::Error::other(err.to_string()),
            })
        }
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(mapped),
        Err(source) => Err(DebugDataError::Io {
            path: system_path.display().to_string(),
            source,
        }),
    }
}

fn classify_provider_error(err: ProviderError) -> (String, String, String) {
    match err {
        ProviderError::HttpStatus { status, body } => (
            format!("http_{status}"),
            "UPSTREAM_HTTP_STATUS".into(),
            format!("upstream http status {status}: {body}"),
        ),
        ProviderError::Request { message } => {
            ("request_failed".into(), "REQUEST_FAILED".into(), message)
        }
        ProviderError::ParseResponse { message } => {
            ("parse_failed".into(), "PARSE_FAILED".into(), message)
        }
        ProviderError::MissingCredentialEnv { env_var } => (
            "missing_credential".into(),
            "MISSING_CREDENTIAL".into(),
            format!("missing provider credential env '{env_var}'"),
        ),
        ProviderError::InvalidHeader { name, message } => (
            "invalid_header".into(),
            "INVALID_HEADER".into(),
            format!("invalid header '{name}': {message}"),
        ),
        ProviderError::DuplicateProvider { name } => (
            "duplicate_provider".into(),
            "DUPLICATE_PROVIDER".into(),
            format!("duplicate provider '{name}'"),
        ),
        ProviderError::UnsupportedProtocol { protocol } => (
            "unsupported_protocol".into(),
            "UNSUPPORTED_PROTOCOL".into(),
            format!("unsupported protocol for real execution: {protocol:?}"),
        ),
    }
}

fn normalize_thinking_effort(effort: &str) -> String {
    match effort.trim() {
        "low" | "medium" | "high" => effort.trim().to_string(),
        _ => "medium".into(),
    }
}

fn mobile_host_config_path(runtime_home: &Path) -> PathBuf {
    runtime_home.join("config/mobile-host-config.json")
}

fn read_mobile_host_config(runtime_home: &Path) -> Result<MobileHostConfig, DebugDataError> {
    let path = mobile_host_config_path(runtime_home);
    match fs::read_to_string(&path) {
        Ok(content) => serde_json::from_str(&content).map_err(DebugDataError::Serialize),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(MobileHostConfig::default()),
        Err(source) => Err(DebugDataError::Io {
            path: path.display().to_string(),
            source,
        }),
    }
}

fn write_mobile_host_config(
    runtime_home: &Path,
    value: &MobileHostConfig,
) -> Result<(), DebugDataError> {
    let path = mobile_host_config_path(runtime_home);
    fs::write(&path, serde_json::to_vec_pretty(value)?).map_err(|source| DebugDataError::Io {
        path: path.display().to_string(),
        source,
    })
}

fn collect_turn_tool_records(
    runtime_home: &Path,
    session_id: &str,
) -> Result<(Vec<Value>, Vec<Value>), DebugDataError> {
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
    let ref_specs: Vec<(String, String)> = refs
        .into_iter()
        .filter_map(|value| value.as_str().map(parse_tool_ref))
        .collect();
    if ref_specs.is_empty() {
        return Ok((Vec::new(), Vec::new()));
    }
    let Some(session_dir) = find_session_dir(runtime_home, session_id) else {
        return Ok((Vec::new(), Vec::new()));
    };
    let mut grouped_paths: std::collections::BTreeMap<String, HashSet<String>> =
        std::collections::BTreeMap::new();
    for (rel, id) in ref_specs {
        grouped_paths.entry(rel).or_default().insert(id);
    }

    let mut matched = Vec::new();
    for (rel, ids) in grouped_paths {
        let records_path = session_dir.join(rel);
        if !records_path.exists() {
            continue;
        }
        let records = read_json_value(&records_path)?;
        if let Some(list) = records.as_array() {
            for item in list {
                let id = item
                    .get("tool_call_id")
                    .and_then(Value::as_str)
                    .unwrap_or_default();
                if ids.contains(id) {
                    matched.push(item.clone());
                }
            }
        }
    }
    if matched.is_empty() {
        return Ok((Vec::new(), Vec::new()));
    }
    let mut seen = HashSet::new();
    matched.retain(|item| {
        let id = item
            .get("tool_call_id")
            .and_then(Value::as_str)
            .unwrap_or_default();
        if id.is_empty() {
            true
        } else {
            seen.insert(id.to_string())
        }
    });
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

fn parse_tool_ref(reference: &str) -> (String, String) {
    let mut parts = reference.split("#tool_call_id=");
    let rel = parts
        .next()
        .unwrap_or("tools/recent_tool_records.json")
        .to_string();
    let id = parts.next().unwrap_or(reference).to_string();
    (rel, id)
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

fn current_turn_id(runtime_home: &Path) -> Option<String> {
    let turn_path = runtime_home.join("runtime/current/current_turn.json");
    read_json_value(&turn_path).ok().and_then(|turn| {
        turn.get("turn_id")
            .and_then(Value::as_str)
            .map(str::to_string)
    })
}

fn mobile_provisional_turn_id(client_message_id: &str) -> String {
    format!("mobile-turn-{}", stable_id_fragment(client_message_id))
}

fn stable_id_fragment(value: &str) -> String {
    let mut out = value
        .chars()
        .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '-' })
        .collect::<String>();
    while out.contains("--") {
        out = out.replace("--", "-");
    }
    let out = out.trim_matches('-').to_string();
    if out.is_empty() {
        "missing-id".to_string()
    } else {
        out
    }
}

fn now_epoch_ms_string() -> String {
    UNIX_EPOCH
        .elapsed()
        .map(|duration| duration.as_millis().to_string())
        .unwrap_or_else(|_| "0".to_string())
}

fn mobile_record_item_id(record: &Value) -> String {
    record
        .get("tool_call_id")
        .or_else(|| record.get("reasoning_id"))
        .or_else(|| record.get("operation_id"))
        .and_then(Value::as_str)
        .map(stable_id_fragment)
        .unwrap_or_default()
}

fn required_record_str(record: &Value, field: &str) -> String {
    record
        .get(field)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| format!("schema_error:{field}"))
}

fn record_str(record: &Value, field: &str) -> String {
    record
        .get(field)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or_default()
        .to_string()
}

fn command_from_record(record: &Value) -> String {
    let mut raw = record_str(record, "input_summary");
    if let Some(stripped) = raw.strip_prefix("cmd=") {
        raw = stripped.trim().to_string();
    }
    raw.trim_matches(['\'', '"']).trim().to_string()
}

fn shorten_command(command: &str) -> String {
    let command = command.trim();
    if command.chars().count() > 72 {
        format!("{}…", command.chars().take(69).collect::<String>())
    } else {
        command.to_string()
    }
}

fn exec_action_label(command: &str) -> &'static str {
    let lc = command.to_lowercase();
    let has_read = lc.contains("cat ")
        || lc.starts_with("cat")
        || lc.contains("bat ")
        || lc.contains("sed -n")
        || lc.contains("tail ")
        || lc.contains("head ")
        || lc.contains("nl -ba");
    let has_list = lc.contains("ls ")
        || lc == "ls"
        || lc.contains("find ")
        || lc.contains("git ls-files")
        || lc.contains("rg --files");
    let has_search = (lc.contains("rg ")
        || lc.starts_with("rg")
        || lc.contains("grep ")
        || lc.contains("egrep ")
        || lc.contains("fgrep ")
        || lc.contains("git grep"))
        && !lc.contains("rg --files");
    let has_edit = lc.contains("apply_patch")
        || lc.contains("perl -pi")
        || lc.contains("sed -i")
        || lc.contains("tee ")
        || lc.contains("mv ")
        || lc.contains("cp ")
        || lc.contains("rm ")
        || lc.contains("mkdir ")
        || lc.contains("touch ")
        || lc.contains("chmod ")
        || lc.contains("> ");
    if has_edit {
        "Edited"
    } else {
        let explore_count = [has_read, has_list, has_search]
            .into_iter()
            .filter(|v| *v)
            .count();
        if explore_count > 1 || ((lc.contains("&&") || lc.contains('|')) && explore_count >= 1) {
            "Explored"
        } else if has_search {
            "Searched"
        } else if has_list {
            "Listed"
        } else if has_read {
            "Read"
        } else {
            "Ran"
        }
    }
}

fn mobile_display_fields(
    record: &Value,
    label: &str,
    title: &str,
    purpose: &str,
) -> (String, String, String) {
    if label == "provider.call" && record_str(record, "output_summary").contains("cache_hit_rate=")
    {
        return (
            label.to_string(),
            title.to_string(),
            record_str(record, "output_summary"),
        );
    }
    if label == "exec_command" || label.contains("shell.exec") || label.contains("exec") {
        let command = command_from_record(record);
        let display_title = exec_action_label(&command).to_string();
        let display_label = if command.is_empty() {
            "local shell command".to_string()
        } else {
            shorten_command(&command)
        };
        let detail = record_str(record, "output_summary");
        let display_purpose = if detail.is_empty() {
            purpose.to_string()
        } else {
            detail
        };
        (display_label, display_title, display_purpose)
    } else {
        (label.to_string(), title.to_string(), purpose.to_string())
    }
}

fn mobile_item_kind(record: &Value) -> &'static str {
    let tool_name = record
        .get("tool_name")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let tool_kind = record
        .get("tool_kind")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if tool_name.contains("provider") || tool_kind.contains("provider") {
        "provider"
    } else if tool_name.contains("shell") || tool_name.contains("exec") {
        "exec"
    } else if record.get("reasoning_id").is_some() {
        "reasoning"
    } else if record
        .get("status")
        .and_then(Value::as_str)
        .map(|status| status == "failed")
        .unwrap_or(false)
    {
        "error"
    } else {
        "tool"
    }
}

fn mobile_project_record(record: &Value) -> Value {
    let raw_label = required_record_str(record, "tool_name");
    let raw_title = required_record_str(record, "title");
    let raw_purpose = required_record_str(record, "purpose");
    let (label, title, purpose) =
        mobile_display_fields(record, &raw_label, &raw_title, &raw_purpose);
    let mut projected = record.clone();
    if let Some(obj) = projected.as_object_mut() {
        obj.insert("tool_name".to_string(), json!(label));
        obj.insert("title".to_string(), json!(title));
        obj.insert("purpose".to_string(), json!(purpose));
        obj.insert("item_kind".to_string(), json!(mobile_item_kind(record)));
        obj.insert(
            "input_summary".to_string(),
            json!(
                record
                    .get("input_summary")
                    .and_then(Value::as_str)
                    .unwrap_or("")
            ),
        );
        obj.insert(
            "target_kind".to_string(),
            json!(
                record
                    .get("target_kind")
                    .and_then(Value::as_str)
                    .unwrap_or("")
            ),
        );
    }
    projected
}

fn mobile_project_records(records: &[Value]) -> Vec<Value> {
    records.iter().map(mobile_project_record).collect()
}

fn mobile_item_events_from_record(
    record: &Value,
    session_id: &str,
    client_message_id: &str,
    force_terminal_status: Option<&str>,
    turn_id: &str,
) -> Vec<Value> {
    let item_id = mobile_record_item_id(record);
    let item_id = if item_id.is_empty() {
        format!("mobile-item-{}", stable_id_fragment(client_message_id))
    } else {
        item_id
    };
    let raw_label = required_record_str(record, "tool_name");
    let raw_title = required_record_str(record, "title");
    let raw_purpose = required_record_str(record, "purpose");
    let (label, title, purpose) =
        mobile_display_fields(record, &raw_label, &raw_title, &raw_purpose);
    let item_kind = mobile_item_kind(record);
    let started_at = required_record_str(record, "started_at");
    let status = force_terminal_status
        .or_else(|| record.get("status").and_then(Value::as_str))
        .unwrap_or("failed");
    let output_summary = record
        .get("output_summary")
        .and_then(Value::as_str)
        .unwrap_or("");
    let error_summary = record
        .get("error_summary")
        .and_then(Value::as_str)
        .unwrap_or("");
    let duration_ms = record
        .get("duration_ms")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let mut events = Vec::new();
    events.push(json!({
        "type":"turn.item.started",
        "session_id":session_id,
        "client_message_id":client_message_id,
        "turn_id":turn_id,
        "item_id":item_id,
        "item_kind":item_kind,
        "label":label,
        "title":title,
        "purpose":purpose,
        "status":"running",
        "started_at":started_at,
        "input_summary": record.get("input_summary").and_then(Value::as_str).unwrap_or(""),
        "target_kind": record.get("target_kind").and_then(Value::as_str).unwrap_or(""),
    }));
    let failed_status = matches!(status, "failed" | "error" | "cancelled" | "canceled");
    let terminal_type = if status == "completed" && error_summary.trim().is_empty() {
        Some("turn.item.completed")
    } else if failed_status || !error_summary.trim().is_empty() {
        Some("turn.item.failed")
    } else {
        None
    };
    if let Some(terminal_type) = terminal_type {
        events.push(json!({
            "type":terminal_type,
            "session_id":session_id,
            "client_message_id":client_message_id,
            "turn_id":turn_id,
            "item_id":item_id,
            "item_kind":item_kind,
            "label":label,
            "title":title,
            "purpose":purpose,
            "status": if terminal_type == "turn.item.completed" { "completed" } else { "failed" },
            "started_at":started_at,
            "duration_ms":duration_ms,
            "output_summary":output_summary,
            "input_summary": record.get("input_summary").and_then(Value::as_str).unwrap_or(""),
            "target_kind": record.get("target_kind").and_then(Value::as_str).unwrap_or(""),
            "error_summary": if error_summary.trim().is_empty() { Value::Null } else { json!(error_summary) },
        }));
    }
    events
}

fn mobile_record_is_terminal(record: &Value, force_terminal_status: Option<&str>) -> bool {
    let status = force_terminal_status
        .or_else(|| record.get("status").and_then(Value::as_str))
        .unwrap_or("running");
    let error_summary = record
        .get("error_summary")
        .and_then(Value::as_str)
        .unwrap_or("");
    let failed_status = matches!(status, "failed" | "error" | "cancelled" | "canceled");
    (status == "completed" && error_summary.trim().is_empty())
        || failed_status
        || !error_summary.trim().is_empty()
}

fn mobile_record_delta_signature(record: &Value, force_terminal_status: Option<&str>) -> String {
    let status = force_terminal_status
        .or_else(|| record.get("status").and_then(Value::as_str))
        .unwrap_or("running");
    let output_summary = record
        .get("output_summary")
        .and_then(Value::as_str)
        .unwrap_or("");
    let error_summary = record
        .get("error_summary")
        .and_then(Value::as_str)
        .unwrap_or("");
    let duration_ms = record
        .get("duration_ms")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let input_summary = record
        .get("input_summary")
        .and_then(Value::as_str)
        .unwrap_or("");
    format!(
        "status={status}|duration_ms={duration_ms}|input={input_summary}|output={output_summary}|error={error_summary}"
    )
}

fn mobile_item_delta_event_from_record(
    record: &Value,
    session_id: &str,
    client_message_id: &str,
    force_terminal_status: Option<&str>,
    turn_id: &str,
) -> Value {
    let item_id = mobile_record_item_id(record);
    let item_id = if item_id.is_empty() {
        format!("mobile-item-{}", stable_id_fragment(client_message_id))
    } else {
        item_id
    };
    let raw_label = required_record_str(record, "tool_name");
    let raw_title = required_record_str(record, "title");
    let raw_purpose = required_record_str(record, "purpose");
    let (label, title, purpose) =
        mobile_display_fields(record, &raw_label, &raw_title, &raw_purpose);
    let item_kind = mobile_item_kind(record);
    let started_at = required_record_str(record, "started_at");
    let status = force_terminal_status
        .or_else(|| record.get("status").and_then(Value::as_str))
        .unwrap_or("running");
    let duration_ms = record
        .get("duration_ms")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let output_summary = record
        .get("output_summary")
        .and_then(Value::as_str)
        .unwrap_or("");
    let error_summary = record
        .get("error_summary")
        .and_then(Value::as_str)
        .unwrap_or("");
    json!({
        "type":"turn.item.delta",
        "session_id":session_id,
        "client_message_id":client_message_id,
        "turn_id":turn_id,
        "item_id":item_id,
        "item_kind":item_kind,
        "label":label,
        "title":title,
        "purpose":purpose,
        "status":status,
        "started_at":started_at,
        "duration_ms":duration_ms,
        "input_summary": record.get("input_summary").and_then(Value::as_str).unwrap_or(""),
        "output_summary":output_summary,
        "target_kind": record.get("target_kind").and_then(Value::as_str).unwrap_or(""),
        "error_summary": if error_summary.trim().is_empty() { Value::Null } else { json!(error_summary) },
    })
}

fn provider_health_status_from_error(error_summary: &str) -> Option<&'static str> {
    let lower = error_summary.to_lowercase();
    if lower.contains("503")
        || lower.contains("gatewayerror")
        || error_summary.contains("没有可用的内网节点")
        || lower.contains("route")
    {
        Some("route_unavailable")
    } else if lower.contains("auth") || lower.contains("401") || lower.contains("403") {
        Some("auth_failed")
    } else if lower.trim().is_empty() {
        None
    } else {
        Some("unavailable")
    }
}

fn send_provider_health_if_provider_error(
    ws: &mut WebSocket<TcpStream>,
    session_id: &str,
    client_message_id: &str,
    record: &Value,
) -> Result<(), DebugDataError> {
    let tool_name = record
        .get("tool_name")
        .and_then(Value::as_str)
        .unwrap_or("");
    let tool_kind = record
        .get("tool_kind")
        .and_then(Value::as_str)
        .unwrap_or("");
    let target_kind = record
        .get("target_kind")
        .and_then(Value::as_str)
        .unwrap_or("");
    let is_provider_record = tool_name.contains("provider")
        || tool_kind.contains("provider")
        || target_kind.contains("provider");
    if !is_provider_record {
        return Ok(());
    }
    let error_summary = record
        .get("error_summary")
        .and_then(Value::as_str)
        .unwrap_or("");
    if let Some(status) = provider_health_status_from_error(error_summary) {
        send_json(
            ws,
            &json!({
                "type":"provider.health",
                "session_id":session_id,
                "client_message_id":client_message_id,
                "status":status,
                "reason":error_summary,
            }),
        )?;
    }
    Ok(())
}

fn emit_incremental_mobile_events(
    ws: &mut WebSocket<TcpStream>,
    runtime_home: &Path,
    session_id: &str,
    client_message_id: &str,
    seen_tool_ids: &mut HashSet<String>,
    seen_error_keys: &mut HashSet<String>,
    item_started: &mut HashSet<String>,
    item_delta_signatures: &mut HashMap<String, String>,
    last_phase: &mut String,
    last_activity_cards_signature: &mut Option<String>,
) -> Result<(), DebugDataError> {
    if let Ok(Some(path)) = last_run_artifact_path(runtime_home, "current_execution_state_path")
        && let Ok(exec) = read_json_value(&path)
    {
        let phase = exec
            .get("phase")
            .and_then(Value::as_str)
            .unwrap_or("running")
            .to_string();
        if *last_phase != phase {
            *last_phase = phase.clone();
            send_json(
                ws,
                &json!({
                    "type":"turn.progress",
                    "session_id":session_id,
                    "client_message_id":client_message_id,
                    "phase":phase,
                }),
            )?;
        }
    }
    let turn_id = current_turn_id(runtime_home)
        .unwrap_or_else(|| mobile_provisional_turn_id(client_message_id));
    let (records, errors) = collect_turn_tool_records(runtime_home, session_id).unwrap_or_default();
    for record in records {
        let id = mobile_record_item_id(&record);
        if id.is_empty() || seen_tool_ids.insert(id) {
            let item_events = mobile_item_events_from_record(
                &record,
                session_id,
                client_message_id,
                None,
                &turn_id,
            );
            for event in &item_events {
                if event["type"] == "turn.item.started" {
                    if let Some(item_id) = event["item_id"].as_str() {
                        item_started.insert(item_id.to_string());
                    }
                }
                send_json(ws, event)?;
            }
            // legacy turn.tool_event removed — turn.item.* events emitted above
            let stable_id = mobile_record_item_id(&record);
            if !stable_id.is_empty() {
                let signature = mobile_record_delta_signature(&record, None);
                item_delta_signatures.insert(stable_id, signature);
            }
            continue;
        }
        let stable_id = mobile_record_item_id(&record);
        if stable_id.is_empty() {
            continue;
        }
        if !item_started.contains(&stable_id) {
            continue;
        }
        let signature = mobile_record_delta_signature(&record, None);
        let unchanged = item_delta_signatures
            .get(&stable_id)
            .map(|prev| prev == &signature)
            .unwrap_or(false);
        if unchanged || mobile_record_is_terminal(&record, None) {
            continue;
        }
        let delta = mobile_item_delta_event_from_record(
            &record,
            session_id,
            client_message_id,
            None,
            &turn_id,
        );
        send_json(ws, &delta)?;
        item_delta_signatures.insert(stable_id, signature);
    }
    for err in errors {
        let key = format!(
            "{}|{}|{}",
            err.get("tool_call_id")
                .and_then(Value::as_str)
                .unwrap_or("-"),
            err.get("tool_name").and_then(Value::as_str).unwrap_or("-"),
            err.get("error_summary")
                .and_then(Value::as_str)
                .unwrap_or("-"),
        );
        if !seen_error_keys.insert(key) {
            continue;
        }
        send_provider_health_if_provider_error(ws, session_id, client_message_id, &err)?;
        let item_events = mobile_item_events_from_record(
            &err,
            session_id,
            client_message_id,
            Some("failed"),
            &turn_id,
        );
        for event in &item_events {
            if event["type"] == "turn.item.started" {
                if let Some(item_id) = event["item_id"].as_str() {
                    item_started.insert(item_id.to_string());
                }
            }
            send_json(ws, event)?;
        }
        // legacy turn.error_event removed — turn.item.* events emitted above
        let stable_id = mobile_record_item_id(&err);
        if !stable_id.is_empty() {
            let signature = mobile_record_delta_signature(&err, Some("failed"));
            item_delta_signatures.insert(stable_id, signature);
        }
    }
    send_activity_cards_snapshot_if_changed(ws, runtime_home, last_activity_cards_signature)?;
    Ok(())
}

fn send_activity_cards_snapshot_if_changed(
    ws: &mut WebSocket<TcpStream>,
    runtime_home: &Path,
    last_signature: &mut Option<String>,
) -> Result<(), DebugDataError> {
    if let Some(message) =
        build_activity_cards_snapshot_message_if_changed(runtime_home, last_signature)?
    {
        send_json(ws, &message)?;
    }
    Ok(())
}

fn build_activity_cards_snapshot_message_if_changed(
    runtime_home: &Path,
    last_signature: &mut Option<String>,
) -> Result<Option<Value>, DebugDataError> {
    let Ok(snapshot) = build_activity_cards(runtime_home) else {
        return Ok(None);
    };
    let signature = activity_cards_snapshot_signature(&snapshot)?;
    if last_signature.as_deref() == Some(signature.as_str()) {
        return Ok(None);
    }
    *last_signature = Some(signature);
    Ok(Some(json!({
        "type": "activity.cards.snapshot",
        "snapshot": snapshot,
    })))
}

fn activity_cards_snapshot_signature(
    snapshot: &ActivityCardsSnapshot,
) -> Result<String, DebugDataError> {
    serde_json::to_string(&json!({
        "session_id": snapshot.session_id,
        "task_id": snapshot.task_id,
        "user_card": snapshot.user_card.as_ref().map(|card| json!({
            "owner_source_id": card.owner_source_id,
            "header": card.header,
            "state": card.state,
            "focus_source_id": card.focus_source_id,
            "focus_summary": card.focus_summary,
            "stage": card.stage,
            "recent_items": card.recent_items,
            "active_sources": card.active_sources,
            "waiting_detail": card.waiting_detail,
            "failure_detail": card.failure_detail,
        })),
        "source_cards": snapshot.source_cards.iter().map(|card| json!({
            "source_id": card.source_id,
            "source_kind": card.source_kind,
            "title": card.title,
            "visibility": card.visibility,
            "state": card.state,
            "summary": card.summary,
            "focus_label": card.focus_label,
            "auto_promoted": card.auto_promoted,
            "current_activity": card.current_activity,
            "recent_actions": card.recent_actions,
            "waiting_detail": card.waiting_detail,
            "failure_detail": card.failure_detail,
            "session_id": card.session_id,
            "task_id": card.task_id,
        })).collect::<Vec<_>>(),
    }))
    .map_err(DebugDataError::Serialize)
}

fn send_json(ws: &mut WebSocket<TcpStream>, value: &Value) -> Result<(), DebugDataError> {
    ws.write_message(Message::Text(value.to_string()))
        .map_err(|source| DebugDataError::Io {
            path: "ws-write".into(),
            source: std::io::Error::other(source.to_string()),
        })
}

fn send_session_list(
    ws: &mut WebSocket<TcpStream>,
    runtime_home: &Path,
) -> Result<(), DebugDataError> {
    let mut sessions = list_sessions(runtime_home);
    sessions.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
    send_json(
        ws,
        &json!({
            "type":"session.list",
            "sessions":sessions
        }),
    )
}

fn send_runtime_views(
    ws: &mut WebSocket<TcpStream>,
    runtime_home: &Path,
) -> Result<(), DebugDataError> {
    let messages = runtime_view_messages(runtime_home)?;
    for message in messages {
        send_json(ws, &message)?;
    }
    Ok(())
}

fn update_session_meta(
    runtime_home: &Path,
    session_id: &str,
    title: Option<&str>,
    archived: Option<bool>,
    deleted: bool,
) -> Result<(), DebugDataError> {
    if session_id.trim().is_empty() {
        return Ok(());
    }
    let dir = runtime_home.join("sessions/meta");
    fs::create_dir_all(&dir).map_err(|source| DebugDataError::Io {
        path: dir.display().to_string(),
        source,
    })?;
    let path = dir.join(format!("{session_id}.json"));
    let mut meta = read_json_value(&path).unwrap_or_else(|_| json!({}));
    meta["session_id"] = json!(session_id);
    if let Some(title) = title.map(str::trim).filter(|item| !item.is_empty()) {
        meta["title"] = json!(title);
    }
    if let Some(archived) = archived {
        meta["archived"] = json!(archived);
    }
    if deleted {
        meta["deleted"] = json!(true);
        meta["archived"] = json!(true);
    }
    fs::write(&path, serde_json::to_vec_pretty(&meta)?).map_err(|source| DebugDataError::Io {
        path: path.display().to_string(),
        source,
    })
}

fn update_session_metas(
    runtime_home: &Path,
    session_ids: &[String],
    title: Option<&str>,
    archived: Option<bool>,
    deleted: bool,
) -> Result<(), DebugDataError> {
    for session_id in session_ids {
        update_session_meta(runtime_home, session_id, title, archived, deleted)?;
    }
    Ok(())
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
    let target_session_id = session_id.unwrap_or(last_session_id);
    if !session_is_visible(runtime_home, target_session_id) {
        return send_json(
            ws,
            &json!({"type":"session.history","session_id":target_session_id,"turns":[]}),
        );
    }
    let msg_path = if target_session_id == last_session_id {
        if let Some(session_dir) = find_session_dir(runtime_home, target_session_id) {
            session_dir.join("conversation/messages.json")
        } else {
            return send_json(
                ws,
                &json!({"type":"session.history","session_id":target_session_id,"turns":[]}),
            );
        }
    } else if let Some(session_dir) = find_session_dir(runtime_home, target_session_id) {
        session_dir.join("conversation/messages.json")
    } else {
        return send_json(
            ws,
            &json!({"type":"session.history","session_id":target_session_id,"turns":[]}),
        );
    };
    if !msg_path.exists() {
        return send_json(
            ws,
            &json!({"type":"session.history","session_id":target_session_id,"turns":[]}),
        );
    }
    let raw = read_json_value(&msg_path)?;
    // Load tool records for this session
    let session_dir = if target_session_id == last_session_id {
        last_run
            .get("session_id")
            .and_then(Value::as_str)
            .and_then(|sid| find_session_dir(runtime_home, sid))
    } else {
        find_session_dir(runtime_home, target_session_id)
    };
    let tool_records: Vec<Value> = if let Some(ref dir) = session_dir {
        let tool_path = dir.join("tools/recent_tool_records.json");
        if tool_path.exists() {
            read_json_value(&tool_path)
                .ok()
                .and_then(|v| v.as_array().cloned())
                .unwrap_or_default()
        } else {
            Vec::new()
        }
    } else {
        Vec::new()
    };

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
                        turns.push(json!({"user_input":u,"assistant_response":"","tool_execution_records":mobile_project_records(&tool_records),"error_records":Vec::<Value>::new()}));
                    }
                    pending_user = Some(content);
                }
                "assistant" => {
                    let u = pending_user.take().unwrap_or_default();
                    turns.push(json!({"user_input":u,"assistant_response":content,"tool_execution_records":mobile_project_records(&tool_records),"error_records":Vec::<Value>::new()}));
                }
                _ => {}
            }
        }
    }
    if let Some(u) = pending_user.take() {
        turns.push(json!({"user_input":u,"assistant_response":"","tool_execution_records":mobile_project_records(&tool_records),"error_records":Vec::<Value>::new()}));
    }
    send_json(
        ws,
        &json!({
            "type":"session.history",
            "session_id":target_session_id,
            "turns":turns
        }),
    )
}

#[cfg(test)]
mod mobile_item_contract_tests {
    use super::*;
    use std::fs;

    fn temp_runtime_home(prefix: &str) -> PathBuf {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        std::env::temp_dir().join(format!("fin-{prefix}-{unique}"))
    }

    fn write_json(path: &Path, value: &Value) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("parent dir");
        }
        fs::write(path, serde_json::to_vec(value).expect("json")).expect("write json");
    }

    #[test]
    fn maps_tool_record_to_started_and_completed_item() {
        let record = json!({
            "tool_call_id":"call-1",
            "tool_name":"provider.call",
            "tool_kind":"framework_tool",
            "title":"Provider Call",
            "purpose":"dispatch compiled prompt to provider",
            "status":"completed",
            "started_at":"2026-05-22T00:00:00Z",
            "duration_ms":42,
            "output_summary":"cache_hit_rate=81.3% · cached_tokens=100/123",
            "error_summary":null
        });
        let events = mobile_item_events_from_record(&record, "s1", "m1", None, "t1");
        assert_eq!(events.len(), 2);
        assert_eq!(events[0]["type"], "turn.item.started");
        assert_eq!(events[1]["type"], "turn.item.completed");
        assert_eq!(events[0]["label"], "provider.call");
        assert_eq!(events[0]["title"], "Provider Call");
        assert_eq!(
            events[0]["purpose"],
            "cache_hit_rate=81.3% · cached_tokens=100/123"
        );
        assert_eq!(
            events[1]["output_summary"],
            "cache_hit_rate=81.3% · cached_tokens=100/123"
        );
        assert_ne!(events[0]["label"], "tool");
        assert_ne!(events[0]["label"], "unknown");
    }

    #[test]
    fn maps_error_record_to_failed_item_with_summary() {
        let record = json!({
            "tool_call_id":"call-2",
            "tool_name":"exec_command",
            "tool_kind":"exec",
            "title":"Execute shell",
            "purpose":"run requested local command",
            "status":"failed",
            "started_at":"2026-05-22T00:00:00Z",
            "duration_ms":7,
            "input_summary":"cmd=nonexistent_command_abc123",
            "output_summary":"exit_code=127, stdout=, stderr=zsh:1: command not found: nonexistent_command_abc123",
            "error_summary":"command not found"
        });
        let events = mobile_item_events_from_record(&record, "s1", "m1", None, "t1");
        assert_eq!(events.len(), 2);
        assert_eq!(events[1]["type"], "turn.item.failed");
        assert_eq!(events[1]["status"], "failed");
        assert_eq!(events[1]["title"], "Ran");
        assert_eq!(events[1]["label"], "nonexistent_command_abc123");
        assert_eq!(
            events[1]["purpose"],
            "exit_code=127, stdout=, stderr=zsh:1: command not found: nonexistent_command_abc123"
        );
        assert_eq!(events[1]["error_summary"], "command not found");
    }

    #[test]
    fn missing_required_fields_are_schema_errors_not_fallbacks() {
        let record = json!({
            "tool_call_id":"call-3",
            "status":"running"
        });
        let events = mobile_item_events_from_record(&record, "s1", "m1", None, "t1");
        assert_eq!(events.len(), 1);
        assert_eq!(events[0]["type"], "turn.item.started");
        assert_eq!(events[0]["label"], "schema_error:tool_name");
        assert_eq!(events[0]["title"], "schema_error:title");
        assert_eq!(events[0]["purpose"], "schema_error:purpose");
    }

    #[test]
    fn provider_route_error_maps_to_health_status() {
        assert_eq!(
            provider_health_status_from_error("503: 没有可用的内网节点"),
            Some("route_unavailable")
        );
        assert_eq!(
            provider_health_status_from_error("401 auth failed"),
            Some("auth_failed")
        );
    }

    #[test]
    fn provider_health_only_for_provider_records() {
        let exec_record = json!({
            "tool_name":"nonexistent_command_abc123",
            "tool_kind":"agent_tool",
            "target_kind":"local_shell",
            "error_summary":"command exited with non-zero status: 127"
        });
        assert_eq!(
            exec_record.get("target_kind").and_then(Value::as_str),
            Some("local_shell")
        );
        assert!(
            !exec_record
                .get("tool_name")
                .and_then(Value::as_str)
                .unwrap_or("")
                .contains("provider")
        );
    }

    #[test]
    fn unknown_post_handshake_message_is_not_handshake_protocol_mismatch() {
        let message = unknown_mobile_message("session.delete", true);
        assert_eq!(message["type"], "protocol.error");
        assert_eq!(message["reason"], "unknown_message_type");
        assert_eq!(message["message_type"], "session.delete");
    }

    #[test]
    fn unknown_pre_handshake_message_requires_auth_without_protocol_mismatch() {
        let message = unknown_mobile_message("session.delete", false);
        assert_eq!(message["type"], "handshake.auth_failed");
        assert_eq!(message["reason"], "handshake_required");
        assert_eq!(message["message_type"], "session.delete");
    }

    #[test]
    fn session_list_ignores_meta_track_and_filters_deleted_sessions() {
        let runtime_home = temp_runtime_home("mobile-session-meta-filter");
        let active_id = "session-active-mainline";
        let deleted_id = "session-deleted-mainline";
        let active_ledger = LedgerStore::for_session(&runtime_home, active_id).expect("ledger");
        active_ledger
            .init(
                Some("task-active"),
                Some(active_id),
                "2026-05-24T10:00:00+08:00",
            )
            .expect("init active");
        active_ledger
            .append(fin_runtime::AppendLedgerRecordInput {
                ts: "2026-05-24T10:00:00+08:00".into(),
                track: LedgerTrackKind::SessionSnapshot,
                record_id: "snapshot-active".into(),
                record_kind: "session_snapshot".into(),
                refs: fin_contracts::LedgerRefs {
                    agent_id: Some("worker-active".into()),
                    entity: fin_contracts::EntityRefs {
                        session_id: Some(active_id.into()),
                        task_id: Some("task-active".into()),
                        ..fin_contracts::EntityRefs::default()
                    },
                    ledger_id: Some(active_id.into()),
                    record_refs: Vec::new(),
                },
                payload: serde_json::to_value(SessionSnapshotRecord {
                    snapshot_id: "snapshot-active".into(),
                    operation_id: "op-active".into(),
                    trace_id: "trace-active".into(),
                    turn_id: "turn-active".into(),
                    refs: fin_contracts::EntityRefs {
                        session_id: Some(active_id.into()),
                        task_id: Some("task-active".into()),
                        ..fin_contracts::EntityRefs::default()
                    },
                    user_input: Some("active task".into()),
                    assistant_summary: "active summary".into(),
                    important_tool_refs: Vec::new(),
                    artifact_refs: Vec::new(),
                    summary: Some("active title".into()),
                    created_at: "2026-05-24T10:00:00+08:00".into(),
                })
                .expect("snapshot active"),
                caused_by: Some("detail-active".into()),
                supersedes: None,
            })
            .expect("append active");
        let deleted_ledger = LedgerStore::for_session(&runtime_home, deleted_id).expect("ledger");
        deleted_ledger
            .init(
                Some("task-deleted"),
                Some(deleted_id),
                "2026-05-24T10:00:01+08:00",
            )
            .expect("init deleted");
        deleted_ledger
            .append(fin_runtime::AppendLedgerRecordInput {
                ts: "2026-05-24T10:00:01+08:00".into(),
                track: LedgerTrackKind::SessionSnapshot,
                record_id: "snapshot-deleted".into(),
                record_kind: "session_snapshot".into(),
                refs: fin_contracts::LedgerRefs {
                    agent_id: Some("worker-deleted".into()),
                    entity: fin_contracts::EntityRefs {
                        session_id: Some(deleted_id.into()),
                        task_id: Some("task-deleted".into()),
                        ..fin_contracts::EntityRefs::default()
                    },
                    ledger_id: Some(deleted_id.into()),
                    record_refs: Vec::new(),
                },
                payload: serde_json::to_value(SessionSnapshotRecord {
                    snapshot_id: "snapshot-deleted".into(),
                    operation_id: "op-deleted".into(),
                    trace_id: "trace-deleted".into(),
                    turn_id: "turn-deleted".into(),
                    refs: fin_contracts::EntityRefs {
                        session_id: Some(deleted_id.into()),
                        task_id: Some("task-deleted".into()),
                        ..fin_contracts::EntityRefs::default()
                    },
                    user_input: Some("deleted task".into()),
                    assistant_summary: "deleted summary".into(),
                    important_tool_refs: Vec::new(),
                    artifact_refs: Vec::new(),
                    summary: Some("deleted title".into()),
                    created_at: "2026-05-24T10:00:01+08:00".into(),
                })
                .expect("snapshot deleted"),
                caused_by: Some("detail-deleted".into()),
                supersedes: None,
            })
            .expect("append deleted");
        write_json(
            &runtime_home
                .join("sessions/meta")
                .join(format!("{active_id}.json")),
            &json!({ "session_id": active_id, "title":"Renamed active" }),
        );
        write_json(
            &runtime_home
                .join("sessions/meta")
                .join(format!("{deleted_id}.json")),
            &json!({ "session_id": deleted_id, "deleted": true, "archived": true }),
        );

        let sessions = list_sessions(&runtime_home);

        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].session_id, active_id);
        assert_eq!(sessions[0].title, "Renamed active");
        assert_eq!(sessions[0].preview_100, "active summary");
        assert_eq!(sessions[0].task_id, "task-active");
    }

    #[test]
    fn deleted_session_is_not_visible_for_default_history() {
        let runtime_home = temp_runtime_home("mobile-history-deleted-filter");
        let session_id = "session-deleted-history";
        write_json(
            &runtime_home
                .join("sessions/2026/05")
                .join(session_id)
                .join("conversation/messages.json"),
            &json!([{ "role":"user", "content":"deleted should not render" }]),
        );
        write_json(
            &runtime_home.join("runtime/current/last_run.json"),
            &json!({
                "session_id": session_id,
                "session_messages_path": format!("sessions/2026/05/{session_id}/conversation/messages.json")
            }),
        );
        write_json(
            &runtime_home
                .join("sessions/meta")
                .join(format!("{session_id}.json")),
            &json!({ "session_id": session_id, "deleted": true, "archived": true }),
        );

        assert!(!session_is_visible(&runtime_home, session_id));
    }

    #[test]
    fn runtime_views_include_project_agent_activity_snapshot() {
        let runtime_home = temp_runtime_home("mobile-activity-cards");
        write_json(
            &runtime_home.join("runtime/current/last_run.json"),
            &json!({
                "session_id":"system-agent",
                "task_id":"task-local-multi-agent",
                "submitted_at":"2026-05-23T00:00:00Z"
            }),
        );
        write_json(
            &runtime_home.join("runtime/peers/registry.json"),
            &json!({
                "peers":[{
                    "peer_id":"local.project-fin",
                    "peer_kind":"project_agent",
                    "presence_state":"online",
                    "runtime_state":"network_registered",
                    "connectivity_state":"network_connected",
                    "binding_state":"agent_rpc_lease",
                    "lifecycle_state":"online",
                    "updated_at":"2026-05-23T00:00:00Z"
                }]
            }),
        );
        let ledger_dir = runtime_home.join("ledgers/project-fin-agent/tracks");
        fs::create_dir_all(&ledger_dir).expect("ledger dir");
        fs::write(
            ledger_dir.join("tools.jsonl"),
            r#"{"ledger_id":"project-fin-agent","seq":1,"ts":"2026-05-23T00:00:00Z","track":"tools","record_id":"tools-1","record_kind":"tools","refs":{},"payload":{"status":"completed","summary":"project agent consumed delegated turn","tool_call_id":"tool-local-progress"}}
"#,
        )
        .expect("tools track");
        write_json(
            &runtime_home.join("runtime/agents/control/runs.json"),
            &json!([{
                "agent_run_id":"project-run-local-multi-agent",
                "agent_id":"local.project-fin",
                "status":"running",
                "result_refs":[],
                "last_heartbeat_at":"2026-05-23T00:00:01Z",
                "path":"project:fin:local.project-fin"
            }]),
        );
        write_json(
            &runtime_home.join("runtime/agents/control/mailbox/local.project-fin/inbox.json"),
            &json!([{
                "message_id":"msg-dispatch-1",
                "seq":1,
                "from_agent_id":"local.system",
                "to_agent_id":"local.project-fin",
                "task_id":"task-local-multi-agent",
                "trigger_turn":true,
                "payload":{
                    "kind":"dispatch",
                    "agent_run_id":"project-run-local-multi-agent",
                    "task_summary":"research external multi-agent design and propose fin optimizations"
                },
                "consumed_at":"2026-05-23T00:00:01Z"
            }]),
        );

        let messages = runtime_view_messages(&runtime_home).expect("runtime views");
        let snapshot = messages
            .iter()
            .find(|message| message["type"] == "activity.cards.snapshot")
            .expect("activity cards snapshot");
        let cards = snapshot["snapshot"]["source_cards"]
            .as_array()
            .expect("source cards");
        let project = cards
            .iter()
            .find(|card| card["source_id"] == "local.project-fin")
            .expect("project agent card");
        assert_eq!(project["source_kind"], "project_agent");
        assert_eq!(project["state"], "running");
        assert_eq!(project["current_activity"], "delegated task executing");
        assert!(
            project["recent_actions"]
                .as_array()
                .expect("recent actions")
                .iter()
                .any(|action| action["summary"] == "project agent consumed delegated turn")
        );
        assert!(
            project["waiting_detail"]
                .as_str()
                .unwrap_or_default()
                .contains("research external multi-agent design")
        );
    }

    #[test]
    fn incremental_mobile_events_emit_activity_cards_snapshot_when_changed() {
        let runtime_home = temp_runtime_home("mobile-activity-cards-incremental");
        write_json(
            &runtime_home.join("runtime/current/last_run.json"),
            &json!({
                "session_id":"system-agent",
                "task_id":"task-local-multi-agent",
                "submitted_at":"2026-05-23T00:00:00Z"
            }),
        );
        write_json(
            &runtime_home.join("runtime/peers/registry.json"),
            &json!({
                "peers":[{
                    "peer_id":"local.project-fin",
                    "peer_kind":"project_agent",
                    "presence_state":"online",
                    "runtime_state":"network_registered",
                    "connectivity_state":"network_connected",
                    "binding_state":"agent_rpc_lease",
                    "lifecycle_state":"online",
                    "updated_at":"2026-05-23T00:00:00Z"
                }]
            }),
        );
        let ledger_dir = runtime_home.join("ledgers/project-fin-agent/tracks");
        fs::create_dir_all(&ledger_dir).expect("ledger dir");
        fs::write(
            ledger_dir.join("tools.jsonl"),
            r#"{"ledger_id":"project-fin-agent","seq":1,"ts":"2026-05-23T00:00:00Z","track":"tools","record_id":"tools-1","record_kind":"tools","refs":{},"payload":{"status":"completed","summary":"project agent consumed delegated turn","tool_call_id":"tool-local-progress"}}
"#,
        )
        .expect("tools track");
        write_json(
            &runtime_home.join("runtime/agents/control/runs.json"),
            &json!([{
                "agent_run_id":"project-run-local-multi-agent",
                "agent_id":"local.project-fin",
                "status":"running",
                "result_refs":[],
                "last_heartbeat_at":"2026-05-23T00:00:01Z",
                "path":"project:fin:local.project-fin"
            }]),
        );
        write_json(
            &runtime_home.join("runtime/agents/control/mailbox/local.project-fin/inbox.json"),
            &json!([{
                "message_id":"msg-dispatch-1",
                "seq":1,
                "from_agent_id":"local.system",
                "to_agent_id":"local.project-fin",
                "task_id":"task-local-multi-agent",
                "trigger_turn":true,
                "payload":{
                    "kind":"dispatch",
                    "agent_run_id":"project-run-local-multi-agent",
                    "task_summary":"research external multi-agent design and propose fin optimizations"
                },
                "consumed_at":"2026-05-23T00:00:01Z"
            }]),
        );

        let mut last_signature = None;
        let message =
            build_activity_cards_snapshot_message_if_changed(&runtime_home, &mut last_signature)
                .expect("message")
                .expect("snapshot event");
        assert_eq!(message["type"], "activity.cards.snapshot");
        assert_eq!(
            message["snapshot"]["source_cards"][0]["source_kind"],
            "project_agent"
        );

        let second =
            build_activity_cards_snapshot_message_if_changed(&runtime_home, &mut last_signature)
                .expect("second");
        assert!(second.is_none());
    }

    #[test]
    fn runtime_views_preserve_completed_project_agent_snapshot() {
        let runtime_home = temp_runtime_home("mobile-activity-cards-completed");
        write_json(
            &runtime_home.join("runtime/current/last_run.json"),
            &json!({
                "session_id":"system-agent",
                "task_id":"task-local-multi-agent",
                "submitted_at":"2026-05-23T00:00:00Z"
            }),
        );
        write_json(
            &runtime_home.join("runtime/peers/registry.json"),
            &json!({
                "peers":[{
                    "peer_id":"local.project-fin",
                    "peer_kind":"project_agent",
                    "presence_state":"online",
                    "runtime_state":"network_registered",
                    "connectivity_state":"network_connected",
                    "binding_state":"agent_rpc_lease",
                    "lifecycle_state":"online",
                    "updated_at":"2026-05-23T00:00:05Z"
                }]
            }),
        );
        write_json(
            &runtime_home.join("runtime/agents/control/runs.json"),
            &json!([{
                "agent_run_id":"project-run-local-multi-agent",
                "agent_id":"local.project-fin",
                "status":"completed",
                "result_refs":["artifact://report-1"],
                "last_heartbeat_at":"2026-05-23T00:00:05Z",
                "closed_at":"2026-05-23T00:00:05Z",
                "path":"project:fin:local.project-fin"
            }]),
        );
        write_json(
            &runtime_home.join("runtime/agents/control/mailbox/system-agent/inbox.json"),
            &json!([{
                "message_id":"msg-result-1",
                "seq":2,
                "from_agent_id":"local.project-fin",
                "to_agent_id":"system-agent",
                "task_id":"task-local-multi-agent",
                "trigger_turn":false,
                "payload":{
                    "kind":"project_result",
                    "agent_run_id":"project-run-local-multi-agent",
                    "result_summary":"project agent completed delegated research and published receipts"
                },
                "consumed_at":"2026-05-23T00:00:05Z"
            }]),
        );

        let messages = runtime_view_messages(&runtime_home).expect("runtime views");
        let snapshot = messages
            .iter()
            .find(|message| message["type"] == "activity.cards.snapshot")
            .expect("activity cards snapshot");
        let cards = snapshot["snapshot"]["source_cards"]
            .as_array()
            .expect("source cards");
        let project = cards
            .iter()
            .find(|card| card["source_id"] == "local.project-fin")
            .expect("project agent card");
        assert_eq!(project["source_kind"], "project_agent");
        assert_eq!(project["state"], "completed");
        assert_eq!(
            project["summary"],
            "project agent completed delegated research and published receipts"
        );
        assert_eq!(
            project["current_activity"],
            "project agent completed delegated research and published receipts"
        );
    }

    #[test]
    fn mobile_effective_config_uses_user_provider_truth_over_stale_system_provider() {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        let runtime_home = std::env::temp_dir().join(format!("fin-mobile-config-{unique}"));
        fs::create_dir_all(runtime_home.join("config")).expect("config dir");
        fs::write(
            runtime_home.join("config/user.toml"),
            r#"
default_provider = "good"

[providers.good]
protocol = "anthropic-wire"
base_url = "https://good.example/anthropic"
model = "good-model"
api_key = "good-key"
"#,
        )
        .expect("write user");
        fs::write(
            runtime_home.join("config/system.toml"),
            r#"
default_provider = "bad"

[providers.bad]
name = "bad"
protocol = "anthropic-wire"
base_url = "https://bad.example/anthropic"
model = "bad-model"

[providers.bad.credential]
kind = "direct_api_key"
api_key = "bad-key"

[providers.bad.headers]

[policy]
default_role = "project"
entry_role = "system"
protocol_version = "fin.m1"

[policy.roles.project]
stream = false
timeout_ms = 60000

[policy.roles.project.provider_path]
strategy = "priority"

[[policy.roles.project.provider_path.targets]]
provider_name = "bad"
model = "bad-model"

[policy.roles.system]
stream = false
timeout_ms = 60000

[policy.roles.system.provider_path]
strategy = "priority"

[[policy.roles.system.provider_path.targets]]
provider_name = "bad"
model = "bad-model"
"#,
        )
        .expect("write stale system");

        let system = read_effective_system_config(&runtime_home).expect("effective config");

        assert_eq!(system.default_provider, "good");
        assert!(system.providers.contains_key("good"));
        assert!(!system.providers.contains_key("bad"));
    }

    #[test]
    fn session_list_includes_session_dirs_without_ledger() {
        let runtime_home = temp_runtime_home("mobile-session-no-ledger");
        let session_id = "session-no-ledger-fresh";
        // Create session dir in sessions/ but NO ledger dir
        let session_dir = runtime_home.join("sessions/2026/05").join(session_id);
        std::fs::create_dir_all(session_dir.join("conversation")).expect("session dir");
        std::fs::write(session_dir.join("conversation/messages.json"), b"[]").expect("messages");
        // Point last_run.json to this session
        write_json(
            &runtime_home.join("runtime/current/last_run.json"),
            &json!({
                "session_id": session_id,
                "task_id": "task-no-ledger-fresh",
            }),
        );

        let sessions = list_sessions(&runtime_home);
        let found = sessions.iter().find(|s| s.session_id == session_id);
        assert!(
            found.is_some(),
            "new session without ledger must appear in list, got: {:?}",
            sessions.iter().map(|s| &s.session_id).collect::<Vec<_>>()
        );
        assert_eq!(found.unwrap().task_id, "task-no-ledger-fresh");
    }

    #[test]
    fn session_list_merges_ledger_and_session_dirs_without_duplicates() {
        let runtime_home = temp_runtime_home("mobile-session-merge");
        let session_id = "session-merge-both";
        // Create ledger entry
        let ledger = LedgerStore::for_session(&runtime_home, session_id).expect("ledger");
        ledger
            .init(
                Some("task-merge"),
                Some(session_id),
                "2026-05-24T10:00:00+08:00",
            )
            .expect("init");
        ledger
            .append(fin_runtime::AppendLedgerRecordInput {
                ts: "2026-05-24T10:00:00+08:00".into(),
                track: LedgerTrackKind::SessionSnapshot,
                record_id: "snapshot-merge".into(),
                record_kind: "session_snapshot".into(),
                refs: fin_contracts::LedgerRefs {
                    agent_id: Some("worker-merge".into()),
                    entity: fin_contracts::EntityRefs {
                        session_id: Some(session_id.into()),
                        task_id: Some("task-merge".into()),
                        ..fin_contracts::EntityRefs::default()
                    },
                    ledger_id: Some(session_id.into()),
                    record_refs: Vec::new(),
                },
                payload: serde_json::to_value(SessionSnapshotRecord {
                    snapshot_id: "snapshot-merge".into(),
                    operation_id: "op-merge".into(),
                    trace_id: "trace-merge".into(),
                    turn_id: "turn-merge".into(),
                    refs: fin_contracts::EntityRefs {
                        session_id: Some(session_id.into()),
                        task_id: Some("task-merge".into()),
                        ..fin_contracts::EntityRefs::default()
                    },
                    user_input: Some("merge task".into()),
                    assistant_summary: "merge summary".into(),
                    important_tool_refs: Vec::new(),
                    artifact_refs: Vec::new(),
                    summary: Some("merge title".into()),
                    created_at: "2026-05-24T10:00:00+08:00".into(),
                })
                .expect("snapshot merge"),
                caused_by: Some("detail-merge".into()),
                supersedes: None,
            })
            .expect("append merge");
        // Also create a matching session dir
        let session_dir = runtime_home.join("sessions/2026/05").join(session_id);
        std::fs::create_dir_all(session_dir.join("conversation")).expect("session dir");
        std::fs::write(session_dir.join("conversation/messages.json"), b"[]").expect("messages");

        let sessions = list_sessions(&runtime_home);
        let matches: Vec<_> = sessions
            .iter()
            .filter(|s| s.session_id == session_id)
            .collect();
        assert_eq!(
            matches.len(),
            1,
            "must not have duplicates, got: {:?}",
            sessions.iter().map(|s| &s.session_id).collect::<Vec<_>>()
        );
        // Should use ledger data (has summary), not fallback
        assert_eq!(matches[0].title, "merge title");
        assert_eq!(matches[0].preview_100, "merge summary");
    }
}
