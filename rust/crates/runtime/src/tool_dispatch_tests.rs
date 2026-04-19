use crate::{model_output::ModelToolCall, tool_dispatch::execute_model_tools};
use fin_contracts::{EntityRefs, MinimalContextView, ProjectContextBlock};
use serde_json::json;
use std::{
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

fn temp_runtime_home(prefix: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "fin-runtime-{prefix}-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos()
    ))
}

fn context_with_runtime_home(runtime_home: &Path) -> MinimalContextView {
    MinimalContextView {
        project: Some(ProjectContextBlock {
            runtime_home: Some(runtime_home.display().to_string()),
            ..ProjectContextBlock::default()
        }),
        ..MinimalContextView::default()
    }
}

fn refs() -> EntityRefs {
    EntityRefs {
        session_id: Some("session-tool-dispatch".into()),
        task_id: Some("task-tool-dispatch".into()),
        worker_id: Some("worker-tool-dispatch".into()),
        ..EntityRefs::default()
    }
}

#[test]
fn exec_command_and_write_stdin_replay_session_work() {
    let runtime_home = temp_runtime_home("exec");
    fs::create_dir_all(&runtime_home).expect("runtime_home");
    let context = context_with_runtime_home(&runtime_home);

    let first = execute_model_tools(
        "op-tools",
        "trace-tools",
        &refs(),
        "2026-04-18T21:00:00+08:00",
        &context,
        1,
        &[ModelToolCall {
            tool_name: "exec_command".into(),
            arguments: json!({
                "cmd": "cat",
                "open_stdin_session": true,
            }),
        }],
    );
    let record = first
        .tool_records
        .iter()
        .find(|item| item.tool_name == "exec_command")
        .expect("exec_command record");
    assert_eq!(record.status, "completed");
    let session_path = record.artifact_refs.first().expect("session artifact path");
    let session_id = Path::new(session_path)
        .file_stem()
        .and_then(|value| value.to_str())
        .expect("session file stem")
        .to_string();

    let second = execute_model_tools(
        "op-tools",
        "trace-tools",
        &refs(),
        "2026-04-18T21:00:01+08:00",
        &context,
        2,
        &[ModelToolCall {
            tool_name: "write_stdin".into(),
            arguments: json!({
                "session_id": session_id,
                "chars": "hello-from-stdin",
            }),
        }],
    );
    let write_record = second
        .tool_records
        .iter()
        .find(|item| item.tool_name == "write_stdin")
        .expect("write_stdin record");
    assert_eq!(write_record.status, "completed");
    assert!(
        write_record
            .output_summary
            .as_deref()
            .unwrap_or_default()
            .contains("hello-from-stdin")
    );
    assert!(
        second
            .events
            .iter()
            .any(|(event_type, _)| event_type == "tool.write_stdin_completed")
    );
}

#[test]
fn mailbox_send_then_poll_consume_produces_expected_events() {
    let runtime_home = temp_runtime_home("mailbox");
    fs::create_dir_all(&runtime_home).expect("runtime_home");
    let context = context_with_runtime_home(&runtime_home);

    let send = execute_model_tools(
        "op-mailbox",
        "trace-mailbox",
        &refs(),
        "2026-04-18T21:10:00+08:00",
        &context,
        1,
        &[ModelToolCall {
            tool_name: "mailbox.send".into(),
            arguments: json!({
                "target_peer_id": "peer-reviewer",
                "message": { "kind": "note", "text": "check logs" },
            }),
        }],
    );
    assert!(
        send.events
            .iter()
            .any(|(event_type, _)| event_type == "mailbox.message_enqueued")
    );

    let poll = execute_model_tools(
        "op-mailbox",
        "trace-mailbox",
        &refs(),
        "2026-04-18T21:10:01+08:00",
        &context,
        2,
        &[ModelToolCall {
            tool_name: "mailbox.poll".into(),
            arguments: json!({
                "peer_id": "peer-reviewer",
                "consume": true,
            }),
        }],
    );
    let poll_record = poll
        .tool_records
        .iter()
        .find(|item| item.tool_name == "mailbox.poll")
        .expect("poll record");
    assert_eq!(poll_record.status, "completed");
    assert!(
        poll_record
            .output_summary
            .as_deref()
            .unwrap_or_default()
            .contains("messages=1")
    );
    assert!(
        poll.events
            .iter()
            .any(|(event_type, _)| event_type == "mailbox.polled")
    );

    let inbox_path = runtime_home.join("runtime/mailbox/peer-reviewer/inbox.json");
    let inbox_text = fs::read_to_string(&inbox_path).expect("inbox text");
    let inbox: Vec<serde_json::Value> = serde_json::from_str(&inbox_text).expect("inbox json");
    assert!(inbox.is_empty());
}
