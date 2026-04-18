use super::*;
use fin_contracts::{
    ControlFeedback, DebugVisibility, EntityRefs, EventEnvelope, ExecutionNote, ProgressBlock,
    ProviderEventPayload, SanitizedProviderDebug, Severity,
};
use std::collections::BTreeMap;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Default)]
struct TestHandler;

impl DebugActionHandler for TestHandler {
    fn read_binding(&self, runtime_home: &Path) -> Result<DebugBinding, String> {
        Ok(DebugBinding {
            project_id: "fin-test".into(),
            project_label: "fin-test".into(),
            runtime_home: runtime_home.display().to_string(),
            session_id: Some("session-1".into()),
            task_id: Some("task-1".into()),
            session_messages_path: Some(
                "sessions/2026/04/session-1/conversation/messages.json".into(),
            ),
            recent_contexts_path: Some(
                "sessions/2026/04/session-1/context/recent_contexts.json".into(),
            ),
            recent_digests_path: Some(
                "sessions/2026/04/session-1/digests/recent_digests.json".into(),
            ),
        })
    }

    fn send_chat_message(
        &self,
        runtime_home: &Path,
        request: ChatSendRequest,
    ) -> Result<ChatSendResponse, String> {
        let binding = self.read_binding(runtime_home)?;
        Ok(ChatSendResponse {
            binding,
            answer: format!("echo:{}", request.message),
            digest_id: "digest-1".into(),
            events_count: 3,
            response_kind: if request.is_status_probe() {
                "status_probe".into()
            } else {
                "assistant_message".into()
            },
            freshness: if request.is_status_probe() {
                Some("recent".into())
            } else {
                None
            },
            control_feedback: None,
            progress: None,
            note: None,
        })
    }
}

#[test]
fn projector_tracks_latest_progress_note_digest_and_provider_activity() {
    let refs = EntityRefs {
        session_id: Some("session-1".into()),
        task_id: Some("task-1".into()),
        topic_thread_id: Some("topic-1".into()),
        dispatch_id: None,
        worker_id: None,
    };
    let progress = ProgressBlock {
        progress_id: "progress-1".into(),
        refs: refs.clone(),
        phase: "running".into(),
        blocker: None,
        next_step: None,
        health_hint: None,
        tool_snapshots: vec![],
    };
    let note = ExecutionNote {
        note_id: "note-1".into(),
        refs: refs.clone(),
        summary: "progress made".into(),
        decision: None,
        lesson: None,
        blocker: None,
        next_step: None,
        control_feedback: None,
        created_at: "2026-04-17T00:00:00Z".into(),
    };
    let feedback = ControlFeedback {
        origin: "runtime_heuristic".into(),
        continuity_confidence: 92,
        topic_shift_confidence: 8,
        simple_query_confidence: 20,
        ..ControlFeedback::default()
    };
    let provider_payload = ProviderEventPayload {
        provider_name: "ali-coding-plan".into(),
        model: "qwen3.6-plus".into(),
        endpoint: "https://provider.example/v1/messages".into(),
        output_text: None,
        response_id: None,
        stop_reason: None,
        status: None,
        debug: Some(SanitizedProviderDebug {
            user_agent: Some("opencode/1.2.27".into()),
            request_headers: BTreeMap::from([
                ("user-agent".into(), "opencode/1.2.27".into()),
                ("x-api-key".into(), "<redacted>".into()),
            ]),
        }),
    };
    let mut projector = InMemoryProjector::default();

    for (idx, (kind, payload)) in [
        (
            "provider.gateway_request_sent",
            serde_json::to_value(provider_payload).unwrap(),
        ),
        ("progress.updated", serde_json::to_value(progress).unwrap()),
        (
            "execution_note.appended",
            serde_json::to_value(note).unwrap(),
        ),
        (
            "control.feedback_recorded",
            serde_json::to_value(feedback).unwrap(),
        ),
        (
            "digest.finalized",
            serde_json::json!({"digest_id": "digest-1"}),
        ),
    ]
    .into_iter()
    .enumerate()
    {
        let mut event = EventEnvelope::new(
            format!("evt-{}", idx + 1),
            kind,
            "2026-04-17T00:00:00Z",
            "runtime",
            "trace-1",
            (idx + 1) as u64,
            payload,
        );
        event.refs = refs.clone();
        event.severity = Severity::Info;
        event.debug_visibility = DebugVisibility::Normal;
        projector.apply(&event);
    }

    assert_eq!(projector.current.current_phase.as_deref(), Some("running"));
    assert_eq!(
        projector.current.latest_progress_id.as_deref(),
        Some("progress-1")
    );
    assert_eq!(projector.current.latest_note_id.as_deref(), Some("note-1"));
    assert_eq!(
        projector.current.latest_digest_id.as_deref(),
        Some("digest-1")
    );
    assert_eq!(
        projector.current.latest_provider_activity.as_deref(),
        Some("provider.gateway_request_sent")
    );
    assert_eq!(
        projector.current.latest_provider_user_agent.as_deref(),
        Some("opencode/1.2.27")
    );
    assert_eq!(
        projector.current.latest_provider_header_names,
        vec!["user-agent".to_string(), "x-api-key".to_string()]
    );
    assert_eq!(
        projector.current.latest_control_origin.as_deref(),
        Some("runtime_heuristic")
    );
    assert_eq!(projector.current.latest_continuity_confidence, Some(92));
    assert_eq!(projector.current.latest_topic_shift_confidence, Some(8));
    assert_eq!(projector.current.latest_simple_query_confidence, Some(20));
    assert_eq!(projector.current.task_id.as_deref(), Some("task-1"));
}

#[test]
fn persist_snapshot_writes_projection_and_event_files() {
    let tmp = std::env::temp_dir().join(format!(
        "fin-debug-server-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time should work")
            .as_nanos()
    ));
    let event = EventEnvelope::new(
        "evt-1",
        "provider.gateway_response_received",
        "2026-04-17T00:00:00Z",
        "runtime",
        "trace-1",
        1,
        serde_json::json!({"status": "ok"}),
    );
    let paths = persist_snapshot(&tmp, &[event]).expect("snapshot should persist");
    assert!(paths.projection_json.exists());
    assert!(paths.events_jsonl.exists());
    assert!(paths.snapshot_json.exists());
}

#[test]
fn response_for_root_serves_html_shell() {
    let response = response_for_path("/", Path::new("/tmp/unused"));
    let body = String::from_utf8(response.body).expect("html should be utf8");
    assert_eq!(response.status_code, 200);
    assert!(body.contains("Conversation"));
    assert!(body.contains("Debug Dashboard"));
}

#[test]
fn response_for_chat_js_serves_compiled_module() {
    let response = response_for_path("/chat.js", Path::new("/tmp/unused"));
    let body = String::from_utf8(response.body).expect("js should be utf8");
    assert_eq!(response.status_code, 200);
    assert!(body.contains("export class ChatPane"));
}

#[test]
fn response_for_section_renderers_js_serves_compiled_module() {
    let response = response_for_path("/section_renderers.js", Path::new("/tmp/unused"));
    let body = String::from_utf8(response.body).expect("js should be utf8");
    assert_eq!(response.status_code, 200);
    assert!(body.contains("renderInspectorSection"));
}

#[test]
fn response_for_sidebar_js_serves_compiled_module() {
    let response = response_for_path("/sidebar.js", Path::new("/tmp/unused"));
    let body = String::from_utf8(response.body).expect("js should be utf8");
    assert_eq!(response.status_code, 200);
    assert!(body.contains("renderSidebar"));
}

#[test]
fn response_for_binding_uses_handler() {
    let handler = TestHandler;
    let runtime_home = Path::new("/tmp/fin-binding");
    let response = response_for_request(
        &HttpRequest {
            method: "GET".into(),
            path: API_BINDING_PATH.into(),
            body: Vec::new(),
        },
        runtime_home,
        &handler,
    );
    assert_eq!(response.status_code, 200);
    let body = String::from_utf8(response.body).unwrap();
    assert!(body.contains("fin-test"));
    assert!(body.contains("session-1"));
}

#[test]
fn response_for_chat_send_uses_handler() {
    let handler = TestHandler;
    let runtime_home = Path::new("/tmp/fin-chat-send");
    let response = response_for_request(
        &HttpRequest {
            method: "POST".into(),
            path: API_CHAT_SEND_PATH.into(),
            body: br#"{"message":"hello"}"#.to_vec(),
        },
        runtime_home,
        &handler,
    );
    assert_eq!(response.status_code, 200);
    let body = String::from_utf8(response.body).unwrap();
    assert!(body.contains("echo:hello"));
    assert!(body.contains("digest-1"));
}

#[test]
fn response_for_chat_send_status_probe_round_trips_kind_and_freshness() {
    let handler = TestHandler;
    let runtime_home = Path::new("/tmp/fin-chat-send-status");
    let response = response_for_request(
        &HttpRequest {
            method: "POST".into(),
            path: API_CHAT_SEND_PATH.into(),
            body: br#"{"message":"/status current","input_kind":"status_probe"}"#.to_vec(),
        },
        runtime_home,
        &handler,
    );
    assert_eq!(response.status_code, 200);
    let body: ChatSendResponse = serde_json::from_slice(&response.body).unwrap();
    assert_eq!(body.response_kind, "status_probe");
    assert_eq!(body.freshness.as_deref(), Some("recent"));
}

#[test]
fn response_for_session_messages_reads_runtime_artifact_via_last_run() {
    assert_runtime_artifact_response(
        "messages",
        API_SESSION_MESSAGES_PATH,
        "session_messages_path",
        "sessions/2026/04/session-1/conversation/messages.json",
        br#"[{"role":"user","content":"hello"}]"#,
        "hello",
    );
}

#[test]
fn response_for_recent_closures_reads_runtime_artifact_via_last_run() {
    assert_runtime_artifact_response(
        "closures",
        API_RECENT_CLOSURES_PATH,
        "session_recent_closures_path",
        "sessions/2026/04/session-1/closures/recent_closures.json",
        br#"[{"operation_id":"op-1","assistant_response":"TRACE"}]"#,
        "TRACE",
    );
}

#[test]
fn response_for_qqbot_state_reads_peer_state_file() {
    let runtime_home = std::env::temp_dir().join(format!(
        "fin-debug-qqbot-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time should work")
            .as_nanos()
    ));
    let peer_dir = runtime_home.join("runtime/peers/qqbot");
    fs::create_dir_all(&peer_dir).expect("peer dir");
    fs::write(
        peer_dir.join("state.json"),
        br#"{"peer_id":"peer-channel-gateway-qqbot-local","lifecycle_state":"paired_active"}"#,
    )
    .expect("state");
    let response = response_for_path(API_QQBOT_STATE_PATH, &runtime_home);
    assert_eq!(response.status_code, 200);
    let body = String::from_utf8(response.body).expect("utf8");
    assert!(body.contains("paired_active"));
}

fn assert_runtime_artifact_response(
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
