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

fn context_with_runtime_home_and_cwd(runtime_home: &Path, cwd: &Path) -> MinimalContextView {
    MinimalContextView {
        project: Some(ProjectContextBlock {
            runtime_home: Some(runtime_home.display().to_string()),
            cwd: Some(cwd.display().to_string()),
            project_root: Some(cwd.display().to_string()),
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
fn apply_patch_replace_mode_updates_file_and_writes_receipt() {
    let runtime_home = temp_runtime_home("apply-patch-replace");
    fs::create_dir_all(&runtime_home).expect("runtime_home");
    let workspace = runtime_home.join("workspace");
    fs::create_dir_all(&workspace).expect("workspace");
    let file_path = workspace.join("sample.txt");
    fs::write(&file_path, "alpha\nbeta\n").expect("seed file");
    let context = context_with_runtime_home_and_cwd(&runtime_home, &workspace);

    let result = execute_model_tools(
        "op-apply-patch-replace",
        "trace-apply-patch-replace",
        &refs(),
        "2026-04-20T12:00:00+08:00",
        &context,
        1,
        &[ModelToolCall {
            tool_name: "apply_patch".into(),
            arguments: json!({
                "path": "sample.txt",
                "old_string": "beta",
                "new_string": "gamma",
            }),
        }],
    );
    let record = result
        .tool_records
        .iter()
        .find(|item| item.tool_name == "apply_patch")
        .expect("apply_patch record");
    assert_eq!(record.status, "completed");
    assert_eq!(
        fs::read_to_string(&file_path).expect("patched file"),
        "alpha\ngamma\n"
    );
    assert!(
        result
            .events
            .iter()
            .any(|(event_type, _)| event_type == "tool.apply_patch_completed")
    );
    assert!(
        record
            .artifact_refs
            .iter()
            .any(|value| value.contains("sample.txt"))
    );
}

#[test]
fn apply_patch_patch_mode_supports_update_and_add() {
    let runtime_home = temp_runtime_home("apply-patch-v4a");
    fs::create_dir_all(&runtime_home).expect("runtime_home");
    let workspace = runtime_home.join("workspace");
    fs::create_dir_all(&workspace).expect("workspace");
    let file_path = workspace.join("src.txt");
    fs::write(&file_path, "before\nstay\n").expect("seed file");
    let context = context_with_runtime_home_and_cwd(&runtime_home, &workspace);

    let result = execute_model_tools(
        "op-apply-patch-v4a",
        "trace-apply-patch-v4a",
        &refs(),
        "2026-04-20T12:00:01+08:00",
        &context,
        1,
        &[ModelToolCall {
            tool_name: "apply_patch".into(),
            arguments: json!({
                "mode": "patch",
                "patch": "*** Begin Patch\n*** Update File: src.txt\n@@\n-before\n+after\n*** Add File: added.txt\n+hello\n+world\n*** End Patch\n",
            }),
        }],
    );
    let record = result
        .tool_records
        .iter()
        .find(|item| item.tool_name == "apply_patch")
        .expect("apply_patch record");
    assert_eq!(record.status, "completed");
    assert_eq!(
        fs::read_to_string(&file_path).expect("updated file"),
        "after\nstay\n"
    );
    assert_eq!(
        fs::read_to_string(workspace.join("added.txt")).expect("added file"),
        "hello\nworld\n"
    );
    assert!(
        record
            .output_summary
            .as_deref()
            .unwrap_or_default()
            .contains("created=1")
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

#[test]
fn project_worker_assignment_and_mailbox_chain_produces_local_worker_truth() {
    let runtime_home = temp_runtime_home("project-worker-chain");
    fs::create_dir_all(&runtime_home).expect("runtime_home");
    let context = context_with_runtime_home(&runtime_home);

    let ensure = execute_model_tools(
        "op-worker-chain",
        "trace-worker-chain",
        &refs(),
        "2026-04-20T15:00:00+08:00",
        &context,
        1,
        &[ModelToolCall {
            tool_name: "daemon.ensure_peer".into(),
            arguments: json!({
                "peer_kind": "project_worker",
                "peer_id": "local-worker-b",
                "lease_ttl_ms": 30000,
            }),
        }],
    );
    assert!(
        ensure
            .events
            .iter()
            .any(|(event_type, _)| event_type == "peer.lease_opened")
    );

    let assign = execute_model_tools(
        "op-worker-chain",
        "trace-worker-chain",
        &refs(),
        "2026-04-20T15:00:01+08:00",
        &context,
        2,
        &[ModelToolCall {
            tool_name: "agent.assign".into(),
            arguments: json!({
                "target_worker_id": "worker-b",
                "task_summary": "inspect logs and report blocker summary",
            }),
        }],
    );
    let assign_record = assign
        .tool_records
        .iter()
        .find(|item| item.tool_name == "agent.assign")
        .expect("assign record");
    assert_eq!(assign_record.status, "completed");
    assert!(
        assign_record
            .input_summary
            .as_deref()
            .unwrap_or_default()
            .contains("target_worker_id=worker-b")
    );
    assert!(
        assign
            .events
            .iter()
            .any(|(event_type, _)| event_type == "agent.assignment_requested")
    );

    let pending_path = runtime_home.join("runtime/assignments/pending.json");
    let pending_text = fs::read_to_string(&pending_path).expect("pending assignments");
    let pending: Vec<serde_json::Value> =
        serde_json::from_str(&pending_text).expect("pending assignments json");
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0]["peer_id"], "local-worker-b");
    assert_eq!(pending[0]["target_worker_id"], "worker-b");
    assert_eq!(pending[0]["requested_role_id"], "project");
    assert_eq!(pending[0]["owner_worker_id"], "worker-tool-dispatch");

    let send = execute_model_tools(
        "op-worker-chain",
        "trace-worker-chain",
        &refs(),
        "2026-04-20T15:00:02+08:00",
        &context,
        3,
        &[ModelToolCall {
            tool_name: "mailbox.send".into(),
            arguments: json!({
                "target_worker_id": "worker-b",
                "message": { "kind": "assignment", "text": "inspect logs and report blocker summary" },
            }),
        }],
    );
    assert!(
        send.events
            .iter()
            .any(|(event_type, _)| event_type == "mailbox.message_enqueued")
    );

    let worker_b_refs = EntityRefs {
        worker_id: Some("worker-b".into()),
        ..refs()
    };
    let poll = execute_model_tools(
        "op-worker-chain",
        "trace-worker-chain",
        &worker_b_refs,
        "2026-04-20T15:00:03+08:00",
        &context,
        4,
        &[ModelToolCall {
            tool_name: "mailbox.poll".into(),
            arguments: json!({
                "worker_id": "worker-b",
                "consume": true,
            }),
        }],
    );
    let poll_record = poll
        .tool_records
        .iter()
        .find(|item| item.tool_name == "mailbox.poll")
        .expect("poll record");
    assert!(
        poll_record
            .input_summary
            .as_deref()
            .unwrap_or_default()
            .contains("worker_id=worker-b")
    );
    assert!(
        poll_record
            .output_summary
            .as_deref()
            .unwrap_or_default()
            .contains("messages=1")
    );

    let inbox_path = runtime_home.join("runtime/mailbox/local-worker-b/inbox.json");
    let inbox_text = fs::read_to_string(&inbox_path).expect("worker mailbox");
    let inbox: Vec<serde_json::Value> = serde_json::from_str(&inbox_text).expect("worker inbox");
    assert!(inbox.is_empty());
}

#[test]
fn update_plan_persists_runtime_plan_artifact() {
    let runtime_home = temp_runtime_home("plan");
    fs::create_dir_all(&runtime_home).expect("runtime_home");
    let session_dir = runtime_home.join("sessions/2026/04/session-tool-dispatch");
    fs::create_dir_all(&session_dir).expect("session_dir");
    let context = context_with_runtime_home(&runtime_home);

    let result = execute_model_tools(
        "op-plan",
        "trace-plan",
        &refs(),
        "2026-04-20T10:00:00+08:00",
        &context,
        1,
        &[ModelToolCall {
            tool_name: "update_plan".into(),
            arguments: json!({
                "explanation": "close current runtime gap",
                "steps": [
                    {"step":"inspect tool catalog","status":"completed"},
                    {"step":"patch runtime","status":"in_progress"},
                    {"step":"run tests","status":"pending"}
                ]
            }),
        }],
    );
    let record = result
        .tool_records
        .iter()
        .find(|item| item.tool_name == "update_plan")
        .expect("update_plan record");
    assert_eq!(record.status, "completed");
    assert!(
        result
            .events
            .iter()
            .any(|(event_type, _)| event_type == "plan.updated")
    );
    let runtime_plan: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(runtime_home.join("runtime/current/current_plan_update.json"))
            .expect("runtime plan"),
    )
    .expect("runtime plan json");
    assert_eq!(
        runtime_plan["steps"].as_array().map(|items| items.len()),
        Some(3)
    );
}

#[test]
fn session_list_returns_recent_session_ids() {
    let runtime_home = temp_runtime_home("session-list");
    fs::create_dir_all(runtime_home.join("sessions/2026/04/session-a")).expect("session-a");
    fs::create_dir_all(runtime_home.join("sessions/2026/04/session-b")).expect("session-b");
    fs::create_dir_all(runtime_home.join("sessions/2026/05/session-c")).expect("session-c");
    let context = context_with_runtime_home(&runtime_home);

    let result = execute_model_tools(
        "op-sessions",
        "trace-sessions",
        &refs(),
        "2026-04-20T10:01:00+08:00",
        &context,
        1,
        &[ModelToolCall {
            tool_name: "session.list".into(),
            arguments: json!({ "limit": 2 }),
        }],
    );
    let record = result
        .tool_records
        .iter()
        .find(|item| item.tool_name == "session.list")
        .expect("session.list record");
    assert_eq!(record.status, "completed");
    let output = record.output_summary.as_deref().unwrap_or_default();
    assert!(output.contains("sessions=2"));
    assert!(output.contains("session-c"));
    assert!(
        result
            .events
            .iter()
            .any(|(event_type, _)| event_type == "session.list_completed")
    );
}
