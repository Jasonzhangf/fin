use crate::scheduler_tick::run_scheduler_tick;
use fin_config::RuntimeRetentionConfig;
use fin_debug_server::{ChatSendResponse, DebugBinding};
use std::{
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

fn temp_runtime_home() -> PathBuf {
    std::env::temp_dir().join(format!(
        "fin-scheduler-tick-{}",
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
        session_id: Some("session-tick".into()),
        task_id: Some("task-tick".into()),
        session_messages_path: Some(
            "sessions/2026/04/session-tick/conversation/messages.json".into(),
        ),
        recent_contexts_path: None,
        recent_digests_path: None,
    }
}

fn write_file(path: &Path, bytes: &[u8]) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("parent");
    }
    fs::write(path, bytes).expect("write");
}

#[test]
fn run_scheduler_tick_routes_framework_events_through_archive_rebalance() {
    let home = temp_runtime_home();
    let session_dir = home.join("sessions/2026/04/session-tick");
    fs::create_dir_all(session_dir.join("conversation")).expect("conversation");
    fs::create_dir_all(session_dir.join("control")).expect("control");
    fs::create_dir_all(session_dir.join("queue")).expect("queue");
    fs::create_dir_all(session_dir.join("tasks/routing")).expect("routing");
    write_file(&session_dir.join("conversation/messages.json"), b"[]");
    write_file(
        &session_dir.join("control/execution_state.json"),
        br#"{
  "state_id":"exec-state-idle",
  "session_id":"session-tick",
  "task_id":"task-tick",
  "status":"idle",
  "active_turn_id":null,
  "active_step_id":null,
  "resume_from_step_id":null,
  "pending_input_count":0,
  "accepts_user_input":true,
  "reason":"idle",
  "updated_at":"2026-04-19T23:10:00+08:00"
}"#,
    );
    write_file(&session_dir.join("queue/pending_inputs.json"), b"[]");
    write_file(&session_dir.join("events/stream.jsonl"), b"");
    write_file(&home.join("runtime/current/last_run.json"), b"{}");

    let mut retention = RuntimeRetentionConfig::default();
    retention.session_event_hot_limit = 2;
    retention.session_event_local_archive_file_limit = 1;

    let outcome = run_scheduler_tick(
        &home,
        &binding(&home),
        "manual_tick",
        &retention,
        8,
        |_binding, _message, _merge_segment| {
            Ok(ChatSendResponse {
                binding: binding(&home),
                answer: "noop".into(),
                digest_id: "digest-noop".into(),
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
    .expect("tick should work");

    assert!(outcome.record.is_some());
    assert_eq!(outcome.drive.drove_count, 0);

    let live_stream =
        fs::read_to_string(session_dir.join("events/stream.jsonl")).expect("live stream");
    assert!(live_stream.contains("scheduler.tick_completed"));
    assert!(live_stream.lines().count() <= 2);

    let archive_index =
        fs::read_to_string(session_dir.join("events/archive_index.json")).expect("archive index");
    assert!(archive_index.contains("\"local_archive_file_count\": 1"));

    let archive_segment =
        fs::read_to_string(session_dir.join("events/archive/segment-000001.jsonl"))
            .expect("archive segment");
    assert!(archive_segment.contains("scheduler.tick_started"));
}
