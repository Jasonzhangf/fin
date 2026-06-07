use super::dispatch;
use super::dispatch::execute_model_tools;
use crate::model::parser::ModelToolCall;
use crate::tools::test_helpers::{context_with_runtime_home, refs, temp_runtime_home};
use fin_contracts::EntityRefs;
use serde_json::json;
use std::{
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

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
            tool_call_id: None,
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
            tool_call_id: None,
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
            tool_call_id: None,
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
    let ensure_requests =
        fs::read_to_string(runtime_home.join("runtime/peers/ensure_requests.json"))
            .expect("ensure requests");
    assert!(ensure_requests.contains("\"peer_id\": \"local-worker-b\""));
    assert!(ensure_requests.contains("\"status\": \"pending\""));

    let assign = execute_model_tools(
        "op-worker-chain",
        "trace-worker-chain",
        &refs(),
        "2026-04-20T15:00:01+08:00",
        &context,
        2,
        &[ModelToolCall {
            tool_name: "agent.assign".into(),
            tool_call_id: None,
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
            tool_call_id: None,
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
            tool_call_id: None,
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
fn daemon_ensure_peer_persists_project_agent_request_contract() {
    let runtime_home = temp_runtime_home("ensure-project-agent");
    fs::create_dir_all(&runtime_home).expect("runtime_home");
    let context = context_with_runtime_home(&runtime_home);

    let ensure = execute_model_tools(
        "op-ensure-project-agent",
        "trace-ensure-project-agent",
        &refs(),
        "2026-04-21T15:00:00+08:00",
        &context,
        1,
        &[ModelToolCall {
            tool_name: "daemon.ensure_peer".into(),
            tool_call_id: None,
            arguments: json!({
                "peer_kind": "project_agent",
                "peer_id": "peer-project-agent-fin",
                "project_id": "fin",
                "agent_name": "builder",
                "mode": "local",
                "project_root": "/tmp/fin",
                "lease_ttl_ms": 45000,
            }),
        }],
    );
    assert!(
        ensure
            .events
            .iter()
            .any(|(event_type, _)| event_type == "daemon.ensure_peer_requested")
    );

    let requests = fs::read_to_string(runtime_home.join("runtime/peers/ensure_requests.json"))
        .expect("ensure requests");
    assert!(requests.contains("\"peer_id\": \"peer-project-agent-fin\""));
    assert!(requests.contains("\"project_id\": \"fin\""));
    assert!(requests.contains("\"agent_name\": \"builder\""));
    assert!(requests.contains("\"mode_hint\": \"local\""));
    assert!(requests.contains("\"status\": \"pending\""));
}
