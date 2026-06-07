use crate::{
    CliError,
    scheduler_driver::{
        drive_scheduler, load_latest_owner_loop_action, load_latest_scheduler_decision,
    },
};
use fin_config::RuntimeRetentionConfig;
use fin_contracts::InputAttachmentSummary;
use fin_debug_server::{ChatSendResponse, DebugBinding};
use serde::{Serialize, de::DeserializeOwned};
use std::{
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

fn temp_runtime_home() -> PathBuf {
    std::env::temp_dir().join(format!(
        "fin-scheduler-driver-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time should work")
            .as_nanos()
    ))
}

fn binding(home: &Path) -> DebugBinding {
    DebugBinding {
        project_id: "fin".into(),
        project_label: "fin".into(),
        runtime_home: home.display().to_string(),
        session_id: Some("session-scheduler".into()),
        task_id: Some("task-scheduler".into()),
        session_messages_path: Some(
            "sessions/2026/04/session-scheduler/conversation/messages.json".into(),
        ),
        recent_contexts_path: None,
        recent_digests_path: None,
    }
}

fn entity_refs(binding: &DebugBinding) -> fin_contracts::EntityRefs {
    fin_contracts::EntityRefs {
        session_id: binding.session_id.clone(),
        task_id: binding.task_id.clone(),
        ..fin_contracts::EntityRefs::default()
    }
}

fn write_json<T: Serialize>(path: &Path, value: &T) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("parent");
    }
    fs::write(path, serde_json::to_vec_pretty(value).expect("json")).expect("write");
}

fn read_json_or_empty<T: DeserializeOwned>(path: &Path) -> Vec<T> {
    match fs::read_to_string(path) {
        Ok(content) => serde_json::from_str(&content).expect("json"),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Vec::new(),
        Err(err) => panic!("read failed: {err}"),
    }
}

fn write_task_registry(session_dir: &Path, task_id: &str, status: &str, claimed_by: Option<&str>) {
    write_json(
        &session_dir.join(format!("tasks/registry/{task_id}.json")),
        &serde_json::json!({
            "task_id": task_id,
            "session_id": "session-scheduler",
            "title": task_id,
            "summary": format!("{task_id} summary"),
            "status": status,
            "claimed_by_worker_id": claimed_by,
            "created_at": "2026-04-21T10:00:00+08:00",
            "updated_at": "2026-04-21T10:00:00+08:00"
        }),
    );
}

#[test]
fn drive_scheduler_runs_pending_until_queue_is_empty() {
    let home = temp_runtime_home();
    let session_dir = home.join("sessions/2026/04/session-scheduler");
    fs::create_dir_all(session_dir.join("conversation")).expect("conversation dir");
    fs::create_dir_all(session_dir.join("control")).expect("control dir");
    fs::create_dir_all(session_dir.join("queue")).expect("queue dir");
    fs::create_dir_all(session_dir.join("tasks/routing")).expect("routing dir");
    write_json(
        &session_dir.join("control/execution_state.json"),
        &fin_contracts::ExecutionStateRecord {
            state_id: "exec-1".into(),
            refs: entity_refs(&binding(&home)),
            status: "idle".into(),
            active_turn_id: None,
            active_step_id: None,
            resume_from_step_id: None,
            resume_checkpoint_ready: false,
            resume_checkpoint_id: None,
            pending_input_count: 2,
            accepts_user_input: true,
            reason: None,
            updated_at: "2026-04-19T22:30:00+08:00".into(),
        },
    );
    write_json(
        &session_dir.join("queue/pending_inputs.json"),
        &vec![
            fin_contracts::PendingInputRecord {
                pending_input_id: "pending-1".into(),
                refs: entity_refs(&binding(&home)),
                input_kind: "chat".into(),
                source: "channel.qqbot".into(),
                message: "a".into(),
                attachments: vec![InputAttachmentSummary {
                    name: Some("a.png".into()),
                    kind: "image/png".into(),
                    ..Default::default()
                }],
                status: "pending".into(),
                enqueue_reason: "test".into(),
                enqueued_at: "2026-04-19T22:30:01+08:00".into(),
            },
            fin_contracts::PendingInputRecord {
                pending_input_id: "pending-2".into(),
                refs: entity_refs(&binding(&home)),
                input_kind: "chat".into(),
                source: "cli.user".into(),
                message: "b".into(),
                attachments: Vec::new(),
                status: "pending".into(),
                enqueue_reason: "test".into(),
                enqueued_at: "2026-04-19T22:30:02+08:00".into(),
            },
        ],
    );
    write_json(
        &session_dir.join("tasks/routing/latest_action.json"),
        &fin_contracts::RoutingActionRecord {
            action_id: "routing-action-1".into(),
            decision_id: "routing-1".into(),
            operation_id: "op-1".into(),
            trace_id: "trace-1".into(),
            refs: entity_refs(&binding(&home)),
            created_at: "2026-04-19T22:30:00+08:00".into(),
            action_kind: "continue_current_task".into(),
            source_disposition: "continue_current_task".into(),
            apply_immediately: true,
            prompt_user: false,
            prompt_text: None,
            suggested_task_id: Some("task-scheduler".into()),
            suggested_topic_thread_id: None,
            confidence: 91,
            reason: "same task".into(),
        },
    );

    let mut seen = Vec::new();
    let mut seen_sources = Vec::new();
    let mut seen_attachment_names = Vec::new();
    let response = drive_scheduler(
        &home,
        &binding(&home),
        &RuntimeRetentionConfig::default(),
        16,
        |binding, message, source, attachments, _merge_segment| {
            seen.push(message.clone());
            seen_sources.push(source);
            seen_attachment_names.push(
                attachments
                    .iter()
                    .filter_map(|item| item.name.clone())
                    .collect::<Vec<_>>(),
            );
            Ok(ChatSendResponse {
                binding,
                answer: format!("ran:{message}"),
                digest_id: "digest-test".into(),
                events_count: 0,
                response_kind: "assistant_message".into(),
                freshness: None,
                control_feedback: None,
                progress: None,
                note: None,
                routing_action: None,
            })
        },
    )
    .expect("drive");

    assert_eq!(seen, vec!["a".to_string(), "b".to_string()]);
    assert_eq!(
        seen_sources,
        vec!["channel.qqbot".to_string(), "cli.user".to_string()]
    );
    assert_eq!(seen_attachment_names[0], vec!["a.png".to_string()]);
    assert!(seen_attachment_names[1].is_empty());
    assert_eq!(response.last_response.expect("response").answer, "ran:b");
    assert_eq!(response.drove_count, 2);
    assert!(!response.decisions.is_empty());
    let remaining: Vec<fin_contracts::PendingInputRecord> =
        read_json_or_empty(&session_dir.join("queue/pending_inputs.json"));
    assert!(remaining.is_empty());
    let latest = load_latest_scheduler_decision(&home, &binding(&home))
        .expect("latest")
        .expect("decision");
    assert!(matches!(
        latest.action_kind.as_str(),
        "run_next_pending" | "stay_idle"
    ));
}

#[test]
fn drive_scheduler_blocks_when_prompt_user_is_required() {
    let home = temp_runtime_home();
    let session_dir = home.join("sessions/2026/04/session-scheduler");
    fs::create_dir_all(session_dir.join("conversation")).expect("conversation dir");
    fs::create_dir_all(session_dir.join("control")).expect("control dir");
    fs::create_dir_all(session_dir.join("queue")).expect("queue dir");
    fs::create_dir_all(session_dir.join("tasks/routing")).expect("routing dir");
    write_json(
        &session_dir.join("control/execution_state.json"),
        &fin_contracts::ExecutionStateRecord {
            state_id: "exec-2".into(),
            refs: entity_refs(&binding(&home)),
            status: "idle".into(),
            active_turn_id: None,
            active_step_id: None,
            resume_from_step_id: None,
            resume_checkpoint_ready: false,
            resume_checkpoint_id: None,
            pending_input_count: 1,
            accepts_user_input: true,
            reason: None,
            updated_at: "2026-04-19T22:31:00+08:00".into(),
        },
    );
    write_json(
        &session_dir.join("queue/pending_inputs.json"),
        &vec![fin_contracts::PendingInputRecord {
            pending_input_id: "pending-1".into(),
            refs: entity_refs(&binding(&home)),
            input_kind: "chat".into(),
            source: "cli.user".into(),
            message: "a".into(),
            attachments: Vec::new(),
            status: "pending".into(),
            enqueue_reason: "test".into(),
            enqueued_at: "2026-04-19T22:31:01+08:00".into(),
        }],
    );
    write_json(
        &session_dir.join("tasks/routing/latest_action.json"),
        &fin_contracts::RoutingActionRecord {
            action_id: "routing-action-2".into(),
            decision_id: "routing-2".into(),
            operation_id: "op-2".into(),
            trace_id: "trace-2".into(),
            refs: entity_refs(&binding(&home)),
            created_at: "2026-04-19T22:31:00+08:00".into(),
            action_kind: "ask_topic_switch".into(),
            source_disposition: "candidate_topic_switch".into(),
            apply_immediately: false,
            prompt_user: true,
            prompt_text: Some("switch?".into()),
            suggested_task_id: None,
            suggested_topic_thread_id: Some("topic-2".into()),
            confidence: 83,
            reason: "topic changed".into(),
        },
    );

    let mut called = false;
    let response = drive_scheduler(
        &home,
        &binding(&home),
        &RuntimeRetentionConfig::default(),
        16,
        |_binding, _message, _source, _attachments, _merge_segment| {
            called = true;
            Err(CliError::Usage)
        },
    )
    .expect("drive");
    assert!(response.last_response.is_none());
    assert_eq!(response.drove_count, 0);
    assert_eq!(response.decisions.len(), 1);
    assert!(!called);
    let latest = load_latest_scheduler_decision(&home, &binding(&home))
        .expect("latest")
        .expect("decision");
    assert_eq!(latest.action_kind, "await_user_confirmation");
}

#[test]
fn drive_scheduler_persists_owner_loop_review_decision_from_managed_tasks() {
    let home = temp_runtime_home();
    let session_dir = home.join("sessions/2026/04/session-scheduler");
    fs::create_dir_all(session_dir.join("conversation")).expect("conversation dir");
    fs::create_dir_all(session_dir.join("control")).expect("control dir");
    fs::create_dir_all(session_dir.join("queue")).expect("queue dir");
    fs::create_dir_all(session_dir.join("tasks/routing")).expect("routing dir");
    fs::create_dir_all(session_dir.join("tasks/registry")).expect("registry dir");
    write_json(
        &session_dir.join("control/execution_state.json"),
        &fin_contracts::ExecutionStateRecord {
            state_id: "exec-owner-loop".into(),
            refs: entity_refs(&binding(&home)),
            status: "idle".into(),
            active_turn_id: None,
            active_step_id: None,
            resume_from_step_id: None,
            resume_checkpoint_ready: false,
            resume_checkpoint_id: None,
            pending_input_count: 0,
            accepts_user_input: true,
            reason: None,
            updated_at: "2026-04-21T10:00:00+08:00".into(),
        },
    );
    write_json(
        &session_dir.join("queue/pending_inputs.json"),
        &Vec::<fin_contracts::PendingInputRecord>::new(),
    );
    write_json(
        &session_dir.join("tasks/routing/latest_action.json"),
        &fin_contracts::RoutingActionRecord {
            action_id: "routing-action-3".into(),
            decision_id: "routing-3".into(),
            operation_id: "op-3".into(),
            trace_id: "trace-3".into(),
            refs: entity_refs(&binding(&home)),
            created_at: "2026-04-21T10:00:00+08:00".into(),
            action_kind: "continue_current_task".into(),
            source_disposition: "continue_current_task".into(),
            apply_immediately: true,
            prompt_user: false,
            prompt_text: None,
            suggested_task_id: Some("task-submitted".into()),
            suggested_topic_thread_id: None,
            confidence: 88,
            reason: "same task".into(),
        },
    );
    write_task_registry(
        &session_dir,
        "task-submitted",
        "submitted",
        Some("worker-a"),
    );

    let mut called = false;
    let response = drive_scheduler(
        &home,
        &binding(&home),
        &RuntimeRetentionConfig::default(),
        16,
        |binding, _message, _source, _attachments, _merge_segment| {
            called = true;
            Ok(ChatSendResponse {
                binding,
                answer: "owner-loop".into(),
                digest_id: "digest-owner-loop".into(),
                events_count: 0,
                response_kind: "assistant_message".into(),
                freshness: None,
                control_feedback: None,
                progress: None,
                note: None,
                routing_action: None,
            })
        },
    )
    .expect("drive");

    assert!(called);
    assert_eq!(response.drove_count, 1);
    assert_eq!(response.owner_loop_actions.len(), 2);
    assert_eq!(
        response.owner_loop_actions[0].action_kind,
        "review_submitted_task"
    );
    let latest_owner = load_latest_owner_loop_action(&home, &binding(&home))
        .expect("owner loop")
        .expect("owner loop record");
    assert_eq!(latest_owner.action_kind, "review_submitted_task");
    assert_eq!(
        latest_owner.target_task_ids,
        vec!["task-submitted".to_string()]
    );
    let latest = load_latest_scheduler_decision(&home, &binding(&home))
        .expect("latest")
        .expect("decision");
    assert_eq!(latest.action_kind, "review_submitted_task");
}

#[test]
fn drive_scheduler_executes_one_framework_owner_loop_turn_for_submitted_task() {
    let home = temp_runtime_home();
    let session_dir = home.join("sessions/2026/04/session-scheduler");
    fs::create_dir_all(session_dir.join("conversation")).expect("conversation dir");
    fs::create_dir_all(session_dir.join("control")).expect("control dir");
    fs::create_dir_all(session_dir.join("queue")).expect("queue dir");
    fs::create_dir_all(session_dir.join("tasks/routing")).expect("routing dir");
    fs::create_dir_all(session_dir.join("tasks/registry")).expect("registry dir");
    write_json(
        &session_dir.join("control/execution_state.json"),
        &fin_contracts::ExecutionStateRecord {
            state_id: "exec-owner-loop-run".into(),
            refs: entity_refs(&binding(&home)),
            status: "idle".into(),
            active_turn_id: None,
            active_step_id: None,
            resume_from_step_id: None,
            resume_checkpoint_ready: false,
            resume_checkpoint_id: None,
            pending_input_count: 0,
            accepts_user_input: true,
            reason: None,
            updated_at: "2026-04-21T10:10:00+08:00".into(),
        },
    );
    write_json(
        &session_dir.join("queue/pending_inputs.json"),
        &Vec::<fin_contracts::PendingInputRecord>::new(),
    );
    write_json(
        &session_dir.join("tasks/routing/latest_action.json"),
        &fin_contracts::RoutingActionRecord {
            action_id: "routing-action-4".into(),
            decision_id: "routing-4".into(),
            operation_id: "op-4".into(),
            trace_id: "trace-4".into(),
            refs: entity_refs(&binding(&home)),
            created_at: "2026-04-21T10:10:00+08:00".into(),
            action_kind: "continue_current_task".into(),
            source_disposition: "continue_current_task".into(),
            apply_immediately: true,
            prompt_user: false,
            prompt_text: None,
            suggested_task_id: Some("task-submitted".into()),
            suggested_topic_thread_id: None,
            confidence: 90,
            reason: "same task".into(),
        },
    );
    write_task_registry(
        &session_dir,
        "task-submitted",
        "submitted",
        Some("worker-a"),
    );

    let mut seen_messages = Vec::new();
    let mut seen_sources = Vec::new();
    let response = drive_scheduler(
        &home,
        &binding(&home),
        &RuntimeRetentionConfig::default(),
        16,
        |binding, message, source, _attachments, _merge_segment| {
            seen_messages.push(message);
            seen_sources.push(source);
            Ok(ChatSendResponse {
                binding,
                answer: "owner-loop-ran".into(),
                digest_id: "digest-owner-loop".into(),
                events_count: 0,
                response_kind: "assistant_message".into(),
                freshness: None,
                control_feedback: None,
                progress: None,
                note: None,
                routing_action: None,
            })
        },
    )
    .expect("drive");

    assert_eq!(response.drove_count, 1);
    assert_eq!(
        seen_sources,
        vec!["framework.owner_loop.review_submitted_task"]
    );
    assert_eq!(seen_messages.len(), 1);
    assert!(seen_messages[0].contains("review submitted managed tasks now"));
    assert!(seen_messages[0].contains("task-submitted"));
    assert_eq!(
        response.last_response.expect("response").answer,
        "owner-loop-ran"
    );
    assert_eq!(response.decisions.len(), 2);
    assert!(
        response
            .decisions
            .iter()
            .all(|item| item.action_kind == "review_submitted_task")
    );
}
