use crate::{
    API_ACTIVITY_CARDS_PATH, API_BINDING_PATH, API_CHAT_SEND_PATH, API_CURRENT_CONTEXT_PATH,
    API_CURRENT_EXECUTION_STATE_PATH, API_CURRENT_INTERRUPTED_SEGMENT_PATH,
    API_CURRENT_PAUSE_CHECKPOINT_PATH, API_CURRENT_PENDING_INPUTS_PATH,
    API_CURRENT_ROUTING_DECISION_PATH, API_CURRENT_SEGMENT_MERGE_PATH, API_EVENTS_PATH,
    API_LAST_RUN_PATH, API_LOG_INGEST_PATH, API_LOG_LATEST_PATH, API_PROJECTION_PATH,
    API_QQBOT_CONVERSATIONS_PATH, API_QQBOT_EVENTS_PATH, API_QQBOT_STATE_PATH,
    API_RECENT_CLOSURES_PATH, API_RECENT_CONTEXTS_PATH, API_RECENT_DIGESTS_PATH,
    API_RECENT_REASONING_VIEWS_PATH, API_RECENT_TOOL_RECORDS_PATH, API_RECENT_TURNS_PATH,
    API_SESSION_EVENT_ARCHIVE_INDEX_PATH, API_SESSION_EVENTS_PATH, API_SESSION_EVENTS_SEGMENT_PATH,
    API_SESSION_MESSAGES_PATH, API_SNAPSHOT_PATH, API_UPDATE_DIR, API_UPDATE_LATEST_PATH,
    API_WATCH_PATH, ChatSendRequest, DebugActionHandler, DebugDataError, HttpRequest, HttpResponse,
    INDEX_HTML_PATH, STYLES_CSS_PATH, bad_request_response, css_response, file_response,
    head_response, html_response, internal_error_response, javascript_response, json_response,
    not_found_response, session_view, web_app, web_assets, web_styles, write_http_response,
};
use std::{net::TcpStream, path::Path};

fn update_dist_dir(runtime_home: &Path) -> std::path::PathBuf {
    let primary = runtime_home.join(API_UPDATE_DIR);
    if primary.join("latest.json").exists() {
        return primary;
    }
    let fallback = std::env::current_dir()
        .ok()
        .map(|cwd| cwd.join("android-client").join(API_UPDATE_DIR))
        .unwrap_or_else(|| runtime_home.join(API_UPDATE_DIR));
    fallback
}

fn update_file_response(runtime_home: &Path, route_path: &str) -> HttpResponse {
    let updates_dir = update_dist_dir(runtime_home);
    if route_path == API_UPDATE_LATEST_PATH {
        return file_response(
            &updates_dir.join("latest.json"),
            "application/json; charset=utf-8",
        );
    }
    if let Some(name) = route_path.strip_prefix("/updates/") {
        if name.is_empty() || name.contains("..") || name.contains('/') || name.contains('\\') {
            return not_found_response(route_path);
        }
        let content_type = if name.ends_with(".apk") {
            "application/vnd.android.package-archive"
        } else {
            "application/octet-stream"
        };
        return file_response(&updates_dir.join(name), content_type);
    }
    not_found_response(route_path)
}

pub(crate) fn handle_connection(
    stream: &mut TcpStream,
    runtime_home: &Path,
    handler: &(impl DebugActionHandler + Sync),
) -> Result<(), DebugDataError> {
    let request = crate::read_http_request(stream)?;
    if request.method == "GET" && request.path == API_WATCH_PATH {
        return crate::event_stream::stream_runtime_updates(stream, runtime_home);
    }
    let response = response_for_request(&request, runtime_home, handler);
    write_http_response(stream, &response)
}

pub(crate) fn response_for_request(
    request: &HttpRequest,
    runtime_home: &Path,
    handler: &(impl DebugActionHandler + Sync),
) -> HttpResponse {
    let route_path = session_view::request_path(&request.path);
    if (request.method == "GET" || request.method == "HEAD") && route_path.starts_with("/updates/")
    {
        let response = update_file_response(runtime_home, route_path);
        return if request.method == "HEAD" {
            head_response(&response)
        } else {
            response
        };
    }
    match (request.method.as_str(), route_path) {
        ("GET", INDEX_HTML_PATH) => html_response(web_assets::INDEX_HTML),
        ("GET", path) if web_app::javascript_for_path(path).is_some() => {
            javascript_response(web_app::javascript_for_path(route_path).unwrap_or(""))
        }
        ("GET", STYLES_CSS_PATH) => css_response(web_styles::STYLES_CSS),
        ("GET", API_UPDATE_LATEST_PATH) => update_file_response(runtime_home, route_path),
        ("GET", path) if path.starts_with("/updates/") => {
            update_file_response(runtime_home, route_path)
        }
        ("GET", API_BINDING_PATH) => match handler.read_binding(runtime_home) {
            Ok(binding) => json_response(200, &binding),
            Err(message) => internal_error_response(&message),
        },
        ("GET", API_ACTIVITY_CARDS_PATH) => match fin_runtime::build_activity_cards(runtime_home) {
            Ok(cards) => json_response(200, &cards),
            Err(fin_runtime::RuntimeError::Io { .. }) => not_found_response("activity_cards"),
            Err(err) => internal_error_response(&err.to_string()),
        },
        ("GET", API_PROJECTION_PATH) => file_response(
            &runtime_home.join("runtime/projections/current_projection.json"),
            "application/json; charset=utf-8",
        ),
        ("GET", API_SNAPSHOT_PATH) => file_response(
            &runtime_home.join("runtime/projections/current_snapshot.json"),
            "application/json; charset=utf-8",
        ),
        ("GET", API_EVENTS_PATH) => file_response(
            &runtime_home.join("runtime/projections/latest_events.jsonl"),
            "application/x-ndjson; charset=utf-8",
        ),
        ("GET", API_LAST_RUN_PATH) => file_response(
            &runtime_home.join("runtime/current/last_run.json"),
            "application/json; charset=utf-8",
        ),
        ("GET", API_CURRENT_CONTEXT_PATH) => file_response(
            &runtime_home.join("runtime/current/current_context.json"),
            "application/json; charset=utf-8",
        ),
        ("GET", API_RECENT_CONTEXTS_PATH) => last_run_artifact_response(
            runtime_home,
            "session_recent_contexts_path",
            "application/json; charset=utf-8",
        ),
        ("GET", API_RECENT_DIGESTS_PATH) => last_run_artifact_response(
            runtime_home,
            "session_recent_digests_path",
            "application/json; charset=utf-8",
        ),
        ("GET", API_RECENT_REASONING_VIEWS_PATH) => last_run_artifact_response(
            runtime_home,
            "session_recent_reasoning_path",
            "application/json; charset=utf-8",
        ),
        ("GET", API_RECENT_TOOL_RECORDS_PATH) => last_run_artifact_response(
            runtime_home,
            "session_recent_tool_records_path",
            "application/json; charset=utf-8",
        ),
        ("GET", API_RECENT_CLOSURES_PATH) => last_run_artifact_response(
            runtime_home,
            "session_recent_closures_path",
            "application/json; charset=utf-8",
        ),
        ("GET", API_RECENT_TURNS_PATH) => last_run_artifact_response(
            runtime_home,
            "session_recent_turns_path",
            "application/json; charset=utf-8",
        ),
        ("GET", API_SESSION_MESSAGES_PATH) => last_run_artifact_response(
            runtime_home,
            "session_messages_path",
            "application/json; charset=utf-8",
        ),
        ("GET", API_SESSION_EVENTS_PATH) => session_events_response(runtime_home),
        ("GET", API_SESSION_EVENT_ARCHIVE_INDEX_PATH) => {
            session_event_archive_index_response(runtime_home)
        }
        ("GET", API_SESSION_EVENTS_SEGMENT_PATH) => {
            session_events_segment_response(runtime_home, request.path.as_str())
        }
        ("GET", API_CURRENT_EXECUTION_STATE_PATH) => last_run_artifact_response(
            runtime_home,
            "current_execution_state_path",
            "application/json; charset=utf-8",
        ),
        ("GET", API_CURRENT_PENDING_INPUTS_PATH) => last_run_artifact_response(
            runtime_home,
            "current_pending_inputs_path",
            "application/json; charset=utf-8",
        ),
        ("GET", API_CURRENT_PAUSE_CHECKPOINT_PATH) => last_run_artifact_response(
            runtime_home,
            "current_pause_checkpoint_path",
            "application/json; charset=utf-8",
        ),
        ("GET", API_CURRENT_INTERRUPTED_SEGMENT_PATH) => last_run_artifact_response(
            runtime_home,
            "current_interrupted_segment_path",
            "application/json; charset=utf-8",
        ),
        ("GET", API_CURRENT_SEGMENT_MERGE_PATH) => last_run_artifact_response(
            runtime_home,
            "current_segment_merge_path",
            "application/json; charset=utf-8",
        ),
        ("GET", API_CURRENT_ROUTING_DECISION_PATH) => last_run_artifact_response(
            runtime_home,
            "current_routing_decision_path",
            "application/json; charset=utf-8",
        ),
        ("GET", API_QQBOT_STATE_PATH) => file_response(
            &runtime_home.join("runtime/peers/qqbot/state.json"),
            "application/json; charset=utf-8",
        ),
        ("GET", API_QQBOT_EVENTS_PATH) => file_response(
            &runtime_home.join("runtime/peers/qqbot/events.jsonl"),
            "application/x-ndjson; charset=utf-8",
        ),
        ("GET", API_QQBOT_CONVERSATIONS_PATH) => file_response(
            &runtime_home.join("runtime/channels/qqbot/conversations.json"),
            "application/json; charset=utf-8",
        ),
        ("POST", API_CHAT_SEND_PATH) => chat_send_response(runtime_home, request, handler),
        ("POST", API_LOG_INGEST_PATH) => {
            let body_str = String::from_utf8_lossy(&request.body).trim().to_string();
            if body_str.is_empty() {
                json_response(
                    400,
                    &serde_json::json!({"ok": false, "error": "empty_body"}),
                )
            } else if let Err(e) = handler.append_mobile_log_event(runtime_home, &body_str) {
                json_response(
                    500,
                    &serde_json::json!({"ok": false, "error": e.to_string()}),
                )
            } else {
                json_response(200, &serde_json::json!({"ok": true}))
            }
        }
        ("GET", API_LOG_LATEST_PATH) => match handler.read_mobile_log_events(runtime_home) {
            Ok(events) => json_response(200, &serde_json::json!({"ok": true, "events": events})),
            Err(e) => json_response(
                500,
                &serde_json::json!({"ok": false, "error": e.to_string()}),
            ),
        },
        _ => not_found_response(&request.path),
    }
}

fn chat_send_response(
    runtime_home: &Path,
    request: &HttpRequest,
    handler: &(impl DebugActionHandler + Sync),
) -> HttpResponse {
    let payload: ChatSendRequest = match serde_json::from_slice(&request.body) {
        Ok(value) => value,
        Err(error) => return bad_request_response(&format!("invalid chat body: {error}")),
    };
    if payload.message.trim().is_empty() {
        return bad_request_response("message is required");
    }
    match handler.send_chat_message(runtime_home, payload) {
        Ok(response) => json_response(200, &response),
        Err(message) => internal_error_response(&message),
    }
}

fn last_run_artifact_response(
    runtime_home: &Path,
    field: &str,
    content_type: &'static str,
) -> HttpResponse {
    let Some(path) = (match session_view::last_run_artifact_path(runtime_home, field) {
        Ok(value) => value,
        Err(DebugDataError::Io { .. }) => return not_found_response(field),
        Err(err) => return internal_error_response(&err.to_string()),
    }) else {
        return not_found_response(field);
    };
    file_response(&path, content_type)
}

fn session_events_response(runtime_home: &Path) -> HttpResponse {
    let Some(path) = (match session_view::session_event_stream_path(runtime_home) {
        Ok(value) => value,
        Err(DebugDataError::Io { .. }) => return not_found_response("session_events_path"),
        Err(err) => return internal_error_response(&err.to_string()),
    }) else {
        return not_found_response("session_events_path");
    };

    match session_view::read_json_lines(&path) {
        Ok(events) => json_response(200, &events),
        Err(err) => internal_error_response(&err.to_string()),
    }
}

fn session_event_archive_index_response(runtime_home: &Path) -> HttpResponse {
    let Some(index) = (match session_view::read_event_archive_index(runtime_home) {
        Ok(value) => value,
        Err(DebugDataError::Io { .. }) => return not_found_response("session_event_archive_index"),
        Err(err) => return internal_error_response(&err.to_string()),
    }) else {
        return not_found_response("session_event_archive_index");
    };
    json_response(200, &index)
}

fn session_events_segment_response(runtime_home: &Path, request_path: &str) -> HttpResponse {
    let Some(tier) = session_view::query_value(request_path, "tier") else {
        return bad_request_response("tier is required");
    };
    let Some(segment) = session_view::query_value(request_path, "segment") else {
        return bad_request_response("segment is required");
    };
    let Some(path) = (match session_view::event_archive_segment_path(runtime_home, tier, segment) {
        Ok(value) => value,
        Err(DebugDataError::Io { source, .. })
            if source.kind() == std::io::ErrorKind::InvalidInput =>
        {
            return bad_request_response("invalid archive segment request");
        }
        Err(DebugDataError::Io { .. }) => return not_found_response("session_event_segment"),
        Err(err) => return internal_error_response(&err.to_string()),
    }) else {
        return not_found_response("session_event_segment");
    };
    match session_view::read_json_lines(&path) {
        Ok(events) => json_response(200, &events),
        Err(DebugDataError::Io { source, .. }) if source.kind() == std::io::ErrorKind::NotFound => {
            not_found_response("session_event_segment")
        }
        Err(err) => internal_error_response(&err.to_string()),
    }
}

#[cfg(test)]
pub(crate) fn response_for_path(path: &str, runtime_home: &Path) -> HttpResponse {
    let handler = crate::NoopDebugActionHandler;
    response_for_request(
        &HttpRequest {
            method: "GET".into(),
            path: path.into(),
            body: Vec::new(),
        },
        runtime_home,
        &handler,
    )
}
