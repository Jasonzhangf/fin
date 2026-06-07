use super::*;
use fin_contracts::{RoutingActionRecord, RoutingDecisionRecord};

fn long_non_simple_message() -> String {
    "please create a formal project task for building runtime routing state machine and session formalization flow".into()
}

fn read_json_value(path: &std::path::Path) -> serde_json::Value {
    serde_json::from_str(&fs::read_to_string(path).expect("json file should exist"))
        .expect("json should parse")
}

fn session_dir_for(home: &std::path::Path, session_id: &str) -> std::path::PathBuf {
    crate::session_binding::find_session_dir(home, session_id)
        .map(|(_, _, dir)| dir)
        .expect("session dir should exist")
}

#[test]
fn first_turn_creates_tentative_session_without_task_and_prompts_formalize() {
    let home = temp_runtime_home();
    ensure_runtime_home_layout(&home).expect("runtime home should init");
    let system = map_system_config(&sample_user_toml()).expect("system config");
    let handler =
        CliDebugActionHandler::new(sample_user_toml(), system).expect("handler should build");

    let response = handler
        .send_message_internal_with_provider(
            &home,
            ChatSendRequest {
                message: long_non_simple_message(),
                input_kind: None,
                attachments: Vec::new(),
            },
            &static_provider(&handler.system),
        )
        .expect("first turn should run");

    assert_eq!(response.response_kind, "assistant_message");
    assert!(response.answer.contains("/formalize"));
    assert!(response.binding.session_id.is_some());
    assert_eq!(response.binding.task_id, None);
    let action = response
        .routing_action
        .expect("routing action should exist");
    assert_eq!(action.action_kind, "ask_formalize_task");
    assert!(action.prompt_user);

    let last_run = read_last_run_value(&home).expect("last run should exist");
    assert!(last_run.get("session_id").is_some());
    assert!(last_run.get("task_id").is_none());

    let session_id = response.binding.session_id.expect("session id");
    let rebound = handler
        .resolve_session_binding(&home, &session_id)
        .expect("binding resolution should work");
    assert_eq!(rebound.session_id.as_deref(), Some(session_id.as_str()));
    assert_eq!(rebound.task_id, None);

    let session_dir = session_dir_for(&home, &session_id);
    let latest_action: RoutingActionRecord = serde_json::from_str(
        &fs::read_to_string(session_dir.join("tasks/routing/latest_action.json"))
            .expect("routing action should persist"),
    )
    .expect("routing action should parse");
    assert_eq!(latest_action.action_kind, "ask_formalize_task");
    assert!(latest_action.prompt_user);
}

#[test]
fn formalize_binds_tentative_session_into_formal_task_and_topic() {
    let home = temp_runtime_home();
    ensure_runtime_home_layout(&home).expect("runtime home should init");
    let system = map_system_config(&sample_user_toml()).expect("system config");
    let handler =
        CliDebugActionHandler::new(sample_user_toml(), system).expect("handler should build");

    let tentative = handler
        .send_message_internal_with_provider(
            &home,
            ChatSendRequest {
                message: long_non_simple_message(),
                input_kind: None,
                attachments: Vec::new(),
            },
            &static_provider(&handler.system),
        )
        .expect("first turn should run");
    let session_id = tentative.binding.session_id.clone().expect("session id");
    let session_dir = session_dir_for(&home, &session_id);

    let formalized = handler
        .send_message_internal_with_provider(
            &home,
            ChatSendRequest {
                message: "/formalize".into(),
                input_kind: None,
                attachments: Vec::new(),
            },
            &static_provider(&handler.system),
        )
        .expect("formalize should run");

    assert_eq!(formalized.response_kind, "system_notice");
    assert_eq!(
        formalized.binding.session_id.as_deref(),
        Some(session_id.as_str())
    );
    let task_id = formalized
        .binding
        .task_id
        .clone()
        .expect("task id should exist");
    assert!(formalized.answer.contains(&task_id));

    let last_run = read_last_run_value(&home).expect("last run");
    assert_eq!(
        last_run
            .get("session_id")
            .and_then(serde_json::Value::as_str),
        Some(session_id.as_str())
    );
    assert_eq!(
        last_run.get("task_id").and_then(serde_json::Value::as_str),
        Some(task_id.as_str())
    );
    let topic_thread_id = last_run
        .get("topic_thread_id")
        .and_then(serde_json::Value::as_str)
        .expect("topic thread id should persist")
        .to_string();

    let task_path = session_dir.join(format!("tasks/registry/{task_id}.json"));
    assert!(task_path.exists());
    let task_json = read_json_value(&task_path);
    assert_eq!(
        task_json.get("task_id").and_then(serde_json::Value::as_str),
        Some(task_id.as_str())
    );
    assert_eq!(
        task_json
            .get("session_id")
            .and_then(serde_json::Value::as_str),
        Some(session_id.as_str())
    );

    let latest_topic = read_json_value(&session_dir.join("topics/latest.json"));
    assert_eq!(
        latest_topic
            .get("task_id")
            .and_then(serde_json::Value::as_str),
        Some(task_id.as_str())
    );
    assert_eq!(
        latest_topic
            .get("topic_thread_id")
            .and_then(serde_json::Value::as_str),
        Some(topic_thread_id.as_str())
    );
    assert!(
        session_dir
            .join(format!("topics/registry/{topic_thread_id}.json"))
            .exists()
    );

    let latest_action: RoutingActionRecord = serde_json::from_str(
        &fs::read_to_string(session_dir.join("tasks/routing/latest_action.json"))
            .expect("routing action should persist"),
    )
    .expect("routing action should parse");
    assert_eq!(latest_action.prompt_user, false);
    let events = fs::read_to_string(session_dir.join("events/stream.jsonl")).expect("events");
    assert!(events.contains("session.formalized"));
}

#[test]
fn stay_resolves_pending_prompt_and_next_input_is_not_blocked_by_old_prompt() {
    let home = temp_runtime_home();
    ensure_runtime_home_layout(&home).expect("runtime home should init");
    let system = map_system_config(&sample_user_toml()).expect("system config");
    let handler =
        CliDebugActionHandler::new(sample_user_toml(), system).expect("handler should build");

    let first = handler
        .send_message_internal_with_provider(
            &home,
            ChatSendRequest {
                message: long_non_simple_message(),
                input_kind: None,
                attachments: Vec::new(),
            },
            &static_provider(&handler.system),
        )
        .expect("first turn should run");
    let session_id = first.binding.session_id.clone().expect("session id");
    let session_dir = session_dir_for(&home, &session_id);

    let stayed = handler
        .send_message_internal(
            &home,
            ChatSendRequest {
                message: "/stay".into(),
                input_kind: None,
                attachments: Vec::new(),
            },
        )
        .expect("stay should run");
    assert_eq!(stayed.response_kind, "system_notice");
    assert!(stayed.answer.contains("已保持当前会话"));

    let latest_action: RoutingActionRecord = serde_json::from_str(
        &fs::read_to_string(session_dir.join("tasks/routing/latest_action.json"))
            .expect("routing action should persist"),
    )
    .expect("routing action should parse");
    assert!(!latest_action.prompt_user);
    assert_eq!(latest_action.action_kind, "stay_tentative_session");

    let next = handler
        .send_message_internal_with_provider(
            &home,
            ChatSendRequest {
                message: "follow up with another long message that still needs more planning and implementation details".into(),
                input_kind: None,
                attachments: Vec::new(),
            },
            &static_provider(&handler.system),
        )
        .expect("next turn should run instead of being blocked by stale prompt");
    assert_eq!(next.response_kind, "assistant_message");
    assert!(next.answer.contains("simulated response for"));
}

#[test]
fn formalize_can_reuse_existing_task_from_pending_routing_action() {
    let home = temp_runtime_home();
    ensure_runtime_home_layout(&home).expect("runtime home should init");
    let system = map_system_config(&sample_user_toml()).expect("system config");
    let handler =
        CliDebugActionHandler::new(sample_user_toml(), system).expect("handler should build");

    let old_session_dir = home.join("sessions/2026/06/session-existing");
    fs::create_dir_all(old_session_dir.join("tasks/registry")).expect("old registry");
    fs::create_dir_all(old_session_dir.join("topics/registry")).expect("old topics registry");
    fs::create_dir_all(old_session_dir.join("conversation")).expect("old conversation");
    write_file(&old_session_dir.join("conversation/messages.json"), b"[]").expect("messages");
    write_file(
        &old_session_dir.join("tasks/registry/task-existing.json"),
        serde_json::to_vec_pretty(&serde_json::json!({
            "task_id": "task-existing",
            "session_id": "session-existing",
            "title": "existing task",
            "summary": "existing summary",
            "status": "ready",
            "artifact_refs": [],
            "created_at": "2026-04-21T10:00:00+08:00",
            "updated_at": "2026-04-21T10:00:00+08:00"
        }))
        .expect("task json")
        .as_slice(),
    )
    .expect("task file");
    write_file(
        &old_session_dir.join("topics/latest.json"),
        br#"{"topic_thread_id":"topic-existing","task_id":"task-existing","summary":"existing topic","created_at":"2026-04-21T10:00:00+08:00"}"#,
    )
    .expect("topic latest");
    write_file(
        &old_session_dir.join("topics/registry/topic-existing.json"),
        br#"{"topic_thread_id":"topic-existing","task_id":"task-existing","summary":"existing topic","created_at":"2026-04-21T10:00:00+08:00"}"#,
    )
    .expect("topic registry");

    let tentative_session_dir = home.join("sessions/2026/06/session-tentative-reuse");
    fs::create_dir_all(tentative_session_dir.join("tasks/routing")).expect("routing dir");
    fs::create_dir_all(tentative_session_dir.join("conversation")).expect("conversation dir");
    write_file(
        &tentative_session_dir.join("conversation/messages.json"),
        b"[]",
    )
    .expect("messages");
    write_file(
        &tentative_session_dir.join("tasks/routing/latest.json"),
        serde_json::to_vec_pretty(&RoutingDecisionRecord {
            decision_id: "routing-op-reuse".into(),
            operation_id: "op-reuse".into(),
            trace_id: "trace-reuse".into(),
            refs: fin_contracts::EntityRefs {
                session_id: Some("session-tentative-reuse".into()),
                ..fin_contracts::EntityRefs::default()
            },
            created_at: "2026-04-21T10:01:00+08:00".into(),
            disposition: "candidate_existing_task".into(),
            requires_user_confirmation: true,
            candidate_task_id: Some("task-existing".into()),
            candidate_topic_thread_id: Some("topic-existing".into()),
            continuity_confidence: 86,
            topic_shift_confidence: 14,
            simple_query_confidence: 8,
            previous_topic_summary: Some("existing topic".into()),
            current_topic_summary: Some("existing topic".into()),
            reason: "reuse existing task".into(),
        })
        .expect("decision json")
        .as_slice(),
    )
    .expect("decision file");
    write_file(
        &tentative_session_dir.join("tasks/routing/latest_action.json"),
        serde_json::to_vec_pretty(&RoutingActionRecord {
            action_id: "routing-action-op-reuse".into(),
            decision_id: "routing-op-reuse".into(),
            operation_id: "op-reuse".into(),
            trace_id: "trace-reuse".into(),
            refs: fin_contracts::EntityRefs {
                session_id: Some("session-tentative-reuse".into()),
                ..fin_contracts::EntityRefs::default()
            },
            created_at: "2026-04-21T10:01:00+08:00".into(),
            action_kind: "ask_reuse_existing_task".into(),
            source_disposition: "candidate_existing_task".into(),
            apply_immediately: false,
            prompt_user: true,
            prompt_text: Some("reuse it".into()),
            suggested_task_id: Some("task-existing".into()),
            suggested_topic_thread_id: Some("topic-existing".into()),
            confidence: 86,
            reason: "reuse existing task".into(),
        })
        .expect("action json")
        .as_slice(),
    )
    .expect("action file");
    write_file(
        &home.join("runtime/current/last_run.json"),
        br#"{
  "session_id":"session-tentative-reuse",
  "session_messages_path":"sessions/2026/06/session-tentative-reuse/conversation/messages.json",
  "session_recent_contexts_path":"sessions/2026/06/session-tentative-reuse/context/recent_contexts.json",
  "session_recent_digests_path":"sessions/2026/06/session-tentative-reuse/digests/recent_digests.json"
}"#,
    )
    .expect("last run");

    let reused = handler
        .send_message_internal(
            &home,
            ChatSendRequest {
                message: "/formalize".into(),
                input_kind: None,
                attachments: Vec::new(),
            },
        )
        .expect("formalize reuse should run");

    assert_eq!(
        reused.binding.session_id.as_deref(),
        Some("session-existing")
    );
    assert_eq!(reused.binding.task_id.as_deref(), Some("task-existing"));
    let last_run = read_last_run_value(&home).expect("last run");
    assert_eq!(
        last_run
            .get("session_id")
            .and_then(serde_json::Value::as_str),
        Some("session-existing")
    );
    assert_eq!(
        last_run.get("task_id").and_then(serde_json::Value::as_str),
        Some("task-existing")
    );
    assert_eq!(
        last_run
            .get("topic_thread_id")
            .and_then(serde_json::Value::as_str),
        Some("topic-existing")
    );
}
