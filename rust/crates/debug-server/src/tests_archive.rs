use super::*;

#[test]
fn response_for_session_events_defaults_to_live_stream_only() {
    let runtime_home = prepare_event_archive_runtime_home("live-only");
    let response = response_for_request(
        &HttpRequest {
            method: "GET".into(),
            path: API_SESSION_EVENTS_PATH.into(),
            headers: Vec::new(),
            body: Vec::new(),
        },
        &runtime_home,
        &TestHandler,
    );
    assert_eq!(response.status_code, 200);
    let body = String::from_utf8(response.body).expect("utf8");
    assert!(body.contains("\"event_id\": \"evt-live-1\""));
    assert!(!body.contains("evt-local-1"));
    assert!(!body.contains("evt-cold-1"));
}

#[test]
fn response_for_session_event_archive_index_includes_segment_lists() {
    let runtime_home = prepare_event_archive_runtime_home("archive-index");
    let response = response_for_request(
        &HttpRequest {
            method: "GET".into(),
            path: API_SESSION_EVENT_ARCHIVE_INDEX_PATH.into(),
            headers: Vec::new(),
            body: Vec::new(),
        },
        &runtime_home,
        &TestHandler,
    );
    assert_eq!(response.status_code, 200);
    let body = String::from_utf8(response.body).expect("utf8");
    assert!(body.contains("\"live_event_count\": 1"));
    assert!(body.contains("\"segment\": \"segment-000001.jsonl\""));
    assert!(body.contains("\"segment\": \"segment-000002.jsonl\""));
    assert!(body.contains("\"local_segments\""));
    assert!(body.contains("\"cold_segments\""));
}

#[test]
fn response_for_session_event_segment_reads_local_archive() {
    let runtime_home = prepare_event_archive_runtime_home("local-segment");
    let response = response_for_request(
        &HttpRequest {
            method: "GET".into(),
            path: format!(
                "{API_SESSION_EVENTS_SEGMENT_PATH}?tier=local&segment=segment-000001.jsonl"
            ),
            headers: Vec::new(),
            body: Vec::new(),
        },
        &runtime_home,
        &TestHandler,
    );
    assert_eq!(response.status_code, 200);
    let body = String::from_utf8(response.body).expect("utf8");
    assert!(body.contains("evt-local-1"));
    assert!(!body.contains("evt-cold-1"));
}

#[test]
fn response_for_session_event_segment_reads_cold_archive() {
    let runtime_home = prepare_event_archive_runtime_home("cold-segment");
    let response = response_for_request(
        &HttpRequest {
            method: "GET".into(),
            path: format!(
                "{API_SESSION_EVENTS_SEGMENT_PATH}?tier=cold&segment=segment-000002.jsonl"
            ),
            headers: Vec::new(),
            body: Vec::new(),
        },
        &runtime_home,
        &TestHandler,
    );
    assert_eq!(response.status_code, 200);
    let body = String::from_utf8(response.body).expect("utf8");
    assert!(body.contains("evt-cold-1"));
    assert!(!body.contains("evt-local-1"));
}

pub(super) fn assert_runtime_artifact_response(
    label: &str,
    api_path: &str,
    last_run_field: &str,
    relative_path: &str,
    artifact_body: &[u8],
    expected_fragment: &str,
) {
    let runtime_home = std::env::temp_dir().join(format!(
        "fin-debug-{label}-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time should work")
            .as_nanos()
    ));
    let current_dir = runtime_home.join("runtime/current");
    let artifact_path = runtime_home.join(relative_path);
    let session_dir = artifact_path
        .parent()
        .expect("artifact parent")
        .to_path_buf();
    fs::create_dir_all(&current_dir).expect("current dir should exist");
    fs::create_dir_all(&session_dir).expect("session dir should exist");
    fs::write(
        current_dir.join("last_run.json"),
        format!(r#"{{"{last_run_field}":"{relative_path}"}}"#),
    )
    .expect("last run should write");
    fs::write(&artifact_path, artifact_body).expect("artifact should write");

    let response = response_for_request(
        &HttpRequest {
            method: "GET".into(),
            path: api_path.into(),
            headers: Vec::new(),
            body: Vec::new(),
        },
        &runtime_home,
        &TestHandler,
    );
    assert_eq!(response.status_code, 200);
    assert_eq!(response.content_type, "application/json; charset=utf-8");
    assert!(
        String::from_utf8(response.body)
            .unwrap()
            .contains(expected_fragment)
    );
}

fn prepare_event_archive_runtime_home(label: &str) -> std::path::PathBuf {
    let runtime_home = std::env::temp_dir().join(format!(
        "fin-debug-event-archive-{label}-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time should work")
            .as_nanos()
    ));
    let current_dir = runtime_home.join("runtime/current");
    let session_dir = runtime_home.join("sessions/2026/04/session-archive");
    let local_archive_dir = session_dir.join("events/archive");
    let cold_archive_dir = runtime_home.join("archive/sessions/2026/04/session-archive/events");
    fs::create_dir_all(&current_dir).expect("current dir");
    fs::create_dir_all(session_dir.join("conversation")).expect("conversation dir");
    fs::create_dir_all(local_archive_dir.clone()).expect("local archive dir");
    fs::create_dir_all(cold_archive_dir.clone()).expect("cold archive dir");

    fs::write(
        current_dir.join("last_run.json"),
        serde_json::to_vec_pretty(&serde_json::json!({
            "session_messages_path": "sessions/2026/04/session-archive/conversation/messages.json",
            "session_event_archive_index_path": "sessions/2026/04/session-archive/events/archive_index.json",
        }))
        .expect("last run json"),
    )
    .expect("last run write");
    fs::write(session_dir.join("conversation/messages.json"), br#"[]"#).expect("messages");
    fs::write(
        session_dir.join("events/stream.jsonl"),
        br#"{"event_id":"evt-live-1","sequence":10,"event_type":"operation.completed"}"#,
    )
    .expect("live stream");
    fs::write(
        local_archive_dir.join("segment-000001.jsonl"),
        br#"{"event_id":"evt-local-1","sequence":1,"event_type":"provider.operation_accepted"}"#,
    )
    .expect("local segment");
    fs::write(
        cold_archive_dir.join("segment-000002.jsonl"),
        br#"{"event_id":"evt-cold-1","sequence":2,"event_type":"provider.completed"}"#,
    )
    .expect("cold segment");
    fs::write(
        session_dir.join("events/archive_index.json"),
        serde_json::to_vec_pretty(&serde_json::json!({
            "session_id": "session-archive",
            "live_stream_path": "sessions/2026/04/session-archive/events/stream.jsonl",
            "local_archive_dir": "sessions/2026/04/session-archive/events/archive",
            "cold_archive_dir": "archive/sessions/2026/04/session-archive/events",
            "live_event_count": 1,
            "local_archive_file_count": 1,
            "cold_archive_file_count": 1,
        }))
        .expect("archive index json"),
    )
    .expect("archive index");
    runtime_home
}
