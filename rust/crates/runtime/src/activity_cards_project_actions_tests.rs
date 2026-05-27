use crate::build_activity_cards;
use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

fn temp_runtime_home() -> PathBuf {
    static TEMP_SEQ: AtomicU64 = AtomicU64::new(1);
    let seq = TEMP_SEQ.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!(
        "fin-project-activity-cards-{}-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos(),
        seq
    ))
}

fn write_json(path: &Path, value: &impl serde::Serialize) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("create parent");
    }
    fs::write(path, serde_json::to_vec_pretty(value).expect("json")).expect("write");
}

#[test]
fn project_agent_card_includes_project_ledger_actions() {
    let runtime_home = temp_runtime_home();
    write_json(
        &runtime_home.join("runtime/current/last_run.json"),
        &serde_json::json!({
            "session_id": "system-agent",
            "task_id": "task-local-multi-agent",
            "submitted_at": "2026-05-23T00:00:00Z"
        }),
    );
    write_json(
        &runtime_home.join("runtime/peers/registry.json"),
        &serde_json::json!({
            "peers": [{
                "peer_id": "local.project-fin",
                "peer_kind": "project_agent",
                "presence_state": "online",
                "runtime_state": "network_registered",
                "connectivity_state": "network_connected",
                "binding_state": "agent_rpc_lease",
                "lifecycle_state": "online",
                "updated_at": "2026-05-23T00:00:00Z"
            }]
        }),
    );
    let ledger_dir = runtime_home.join("ledgers/project-fin-agent/tracks");
    fs::create_dir_all(&ledger_dir).expect("ledger dir");
    fs::write(
        ledger_dir.join("tools.jsonl"),
        r#"{"ledger_id":"project-fin-agent","seq":4,"ts":"2026-05-23T00:00:00Z","track":"tools","record_id":"tools-4","record_kind":"tools","refs":{},"payload":{"status":"completed","summary":"project tool turn completed","tool_call_id":"tool-local-progress"}}
"#,
    )
    .expect("tools track");
    fs::write(
        ledger_dir.join("provider.jsonl"),
        r#"{"ledger_id":"project-fin-agent","seq":5,"ts":"2026-05-23T00:00:01Z","track":"provider","record_id":"provider-5","record_kind":"provider","refs":{},"payload":{"status":200,"summary":"provider turn completed","request_id":"provider-local-progress"}}
"#,
    )
    .expect("provider track");

    let cards = build_activity_cards(&runtime_home).expect("cards");
    let project = cards
        .source_cards
        .iter()
        .find(|card| card.source_id == "local.project-fin")
        .expect("project card");
    assert_eq!(project.source_kind, "project_agent");
    assert_eq!(project.recent_actions.len(), 2);
    assert!(
        project
            .recent_actions
            .iter()
            .any(|action| action.tool_name == "provider.call")
    );
    assert!(
        project
            .recent_actions
            .iter()
            .any(|action| action.tool_name == "project.tool")
    );
}

#[test]
fn project_agent_card_reflects_delegated_run_lifecycle() {
    let runtime_home = temp_runtime_home();
    write_json(
        &runtime_home.join("runtime/current/last_run.json"),
        &serde_json::json!({
            "session_id": "system-agent",
            "task_id": "task-local-multi-agent",
            "submitted_at": "2026-05-23T00:00:00Z"
        }),
    );
    write_json(
        &runtime_home.join("runtime/peers/registry.json"),
        &serde_json::json!({
            "peers": [{
                "peer_id": "local.project-fin",
                "peer_kind": "project_agent",
                "presence_state": "online",
                "runtime_state": "network_registered",
                "connectivity_state": "network_connected",
                "binding_state": "agent_rpc_lease",
                "lifecycle_state": "online",
                "updated_at": "2026-05-23T00:00:00Z"
            }]
        }),
    );
    write_json(
        &runtime_home.join("runtime/agents/control/runs.json"),
        &serde_json::json!([{
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
        &serde_json::json!([{
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

    let cards = build_activity_cards(&runtime_home).expect("cards");
    let project = cards
        .source_cards
        .iter()
        .find(|card| card.source_id == "local.project-fin")
        .expect("project card");
    assert_eq!(project.state, "running");
    assert_eq!(
        project.current_activity.as_deref(),
        Some("delegated task executing")
    );
    assert!(
        project
            .waiting_detail
            .as_deref()
            .unwrap_or_default()
            .contains("research external multi-agent design")
    );
}

#[test]
fn project_agent_card_falls_back_to_supervision_and_pickup_truth() {
    let runtime_home = temp_runtime_home();
    write_json(
        &runtime_home.join("runtime/current/last_run.json"),
        &serde_json::json!({
            "session_id": "system-agent",
            "task_id": "task-local-multi-agent",
            "submitted_at": "2026-05-23T00:00:00Z"
        }),
    );
    write_json(
        &runtime_home.join("runtime/peers/registry.json"),
        &serde_json::json!({
            "peers": [{
                "peer_id": "local.project-fin",
                "peer_kind": "project_agent",
                "presence_state": "online",
                "runtime_state": "network_registered",
                "connectivity_state": "network_connected",
                "binding_state": "agent_rpc_lease",
                "lifecycle_state": "online",
                "updated_at": "2026-05-23T00:00:00Z"
            }]
        }),
    );
    write_json(
        &runtime_home.join("runtime/current/current_project_supervision.json"),
        &serde_json::json!({
            "projects":[{
                "project_id":"fin",
                "agent_id":"local.project-fin",
                "supervision_state":"resume_ready",
                "desired_action":"resume_project_task",
                "summary":"project agent ready to resume delegated task"
            }]
        }),
    );
    write_json(
        &runtime_home.join("runtime/current/current_project_runtime_pickups.json"),
        &serde_json::json!({
            "projects":[{
                "project_id":"fin",
                "agent_id":"local.project-fin",
                "pickup_state":"ready_to_resume",
                "next_action":"scheduler_tick_needed",
                "summary":"project runtime ready to resume task task-local-multi-agent; pending_inputs=1"
            }]
        }),
    );

    let cards = build_activity_cards(&runtime_home).expect("cards");
    let project = cards
        .source_cards
        .iter()
        .find(|card| card.source_id == "local.project-fin")
        .expect("project card");
    assert_eq!(
        project.summary,
        "project runtime ready to resume task task-local-multi-agent; pending_inputs=1"
    );
    assert_eq!(
        project.current_activity.as_deref(),
        Some("ready_to_resume · scheduler_tick_needed")
    );
}

#[test]
fn project_agent_card_preserves_completed_lifecycle_and_result_summary() {
    let runtime_home = temp_runtime_home();
    write_json(
        &runtime_home.join("runtime/current/last_run.json"),
        &serde_json::json!({
            "session_id": "system-agent",
            "task_id": "task-local-multi-agent",
            "submitted_at": "2026-05-23T00:00:00Z"
        }),
    );
    write_json(
        &runtime_home.join("runtime/peers/registry.json"),
        &serde_json::json!({
            "peers": [{
                "peer_id": "local.project-fin",
                "peer_kind": "project_agent",
                "presence_state": "online",
                "runtime_state": "network_registered",
                "connectivity_state": "network_connected",
                "binding_state": "agent_rpc_lease",
                "lifecycle_state": "online",
                "updated_at": "2026-05-23T00:00:05Z"
            }]
        }),
    );
    write_json(
        &runtime_home.join("runtime/agents/control/runs.json"),
        &serde_json::json!([{
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
        &serde_json::json!([{
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

    let cards = build_activity_cards(&runtime_home).expect("cards");
    let project = cards
        .source_cards
        .iter()
        .find(|card| card.source_id == "local.project-fin")
        .expect("project card");
    assert_eq!(project.state, "completed");
    assert_eq!(
        project.summary,
        "project agent completed delegated research and published receipts"
    );
    assert_eq!(
        project.current_activity.as_deref(),
        Some("project agent completed delegated research and published receipts")
    );
}

#[test]
fn project_agent_card_reads_completed_summary_from_runtime_system_identity_mailbox() {
    let runtime_home = temp_runtime_home();
    write_json(
        &runtime_home.join("runtime/current/last_run.json"),
        &serde_json::json!({
            "session_id": "system-agent",
            "task_id": "task-local-multi-agent",
            "submitted_at": "2026-05-23T00:00:00Z"
        }),
    );
    write_json(
        &runtime_home.join("runtime/peers/registry.json"),
        &serde_json::json!({
            "peers": [{
                "peer_id": "local.project-fin",
                "peer_kind": "project_agent",
                "presence_state": "online",
                "runtime_state": "network_registered",
                "connectivity_state": "network_connected",
                "binding_state": "agent_rpc_lease",
                "lifecycle_state": "online",
                "updated_at": "2026-05-23T00:00:05Z"
            },{
                "peer_id": "local.system",
                "peer_kind": "system_agent",
                "presence_state": "online",
                "runtime_state": "network_registered",
                "connectivity_state": "network_connected",
                "binding_state": "agent_rpc_lease",
                "lifecycle_state": "online",
                "updated_at": "2026-05-23T00:00:05Z"
            }]
        }),
    );
    write_json(
        &runtime_home.join("runtime/agents/control/runs.json"),
        &serde_json::json!([{
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
        &runtime_home.join("runtime/agents/control/mailbox/local.system/inbox.json"),
        &serde_json::json!([{
            "message_id":"msg-result-1",
            "seq":2,
            "from_agent_id":"local.project-fin",
            "to_agent_id":"local.system",
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

    let cards = build_activity_cards(&runtime_home).expect("cards");
    let project = cards
        .source_cards
        .iter()
        .find(|card| card.source_id == "local.project-fin")
        .expect("project card");
    assert_eq!(
        project.summary,
        "project agent completed delegated research and published receipts"
    );
    assert_eq!(
        project.current_activity.as_deref(),
        Some("project agent completed delegated research and published receipts")
    );
}

#[test]
fn project_agent_card_reflects_failed_timeout_and_closed_lifecycle() {
    let runtime_home = temp_runtime_home();
    write_json(
        &runtime_home.join("runtime/current/last_run.json"),
        &serde_json::json!({
            "session_id": "system-agent",
            "task_id": "task-local-multi-agent",
            "submitted_at": "2026-05-23T00:00:00Z"
        }),
    );
    write_json(
        &runtime_home.join("runtime/peers/registry.json"),
        &serde_json::json!({
            "peers": [{
                "peer_id": "local.project-fin",
                "peer_kind": "project_agent",
                "presence_state": "online",
                "runtime_state": "network_registered",
                "connectivity_state": "network_connected",
                "binding_state": "agent_rpc_lease",
                "lifecycle_state": "online",
                "updated_at": "2026-05-23T00:00:05Z"
            }]
        }),
    );

    let runs_path = runtime_home.join("runtime/agents/control/runs.json");
    let system_mailbox_path =
        runtime_home.join("runtime/agents/control/mailbox/system-agent/inbox.json");

    write_json(
        &runs_path,
        &serde_json::json!([{
            "agent_run_id":"project-run-failed",
            "agent_id":"local.project-fin",
            "status":"failed",
            "result_refs":["artifact://failed-report"],
            "last_heartbeat_at":"2026-05-23T00:00:03Z",
            "closed_at":"2026-05-23T00:00:03Z",
            "path":"project:fin:local.project-fin"
        }]),
    );
    let cards = build_activity_cards(&runtime_home).expect("cards failed");
    let project = cards
        .source_cards
        .iter()
        .find(|card| card.source_id == "local.project-fin")
        .expect("project card failed");
    assert_eq!(project.state, "failed");
    assert_eq!(
        project.current_activity.as_deref(),
        Some("delegated task failed")
    );
    assert!(
        project
            .failure_detail
            .as_deref()
            .unwrap_or_default()
            .contains("failed")
    );

    write_json(
        &runs_path,
        &serde_json::json!([{
            "agent_run_id":"project-run-timeout",
            "agent_id":"local.project-fin",
            "status":"timeout",
            "result_refs":[],
            "last_heartbeat_at":"2026-05-23T00:00:04Z",
            "path":"project:fin:local.project-fin"
        }]),
    );
    write_json(
        &runtime_home.join("runtime/agents/control/mailbox/local.project-fin/inbox.json"),
        &serde_json::json!([{
            "message_id":"msg-dispatch-timeout",
            "seq":3,
            "from_agent_id":"local.system",
            "to_agent_id":"local.project-fin",
            "task_id":"task-local-multi-agent",
            "trigger_turn":true,
            "payload":{
                "kind":"dispatch",
                "agent_run_id":"project-run-timeout",
                "task_summary":"collect long-running delegated task status"
            },
            "consumed_at":"2026-05-23T00:00:04Z"
        }]),
    );
    let cards = build_activity_cards(&runtime_home).expect("cards timeout");
    let project = cards
        .source_cards
        .iter()
        .find(|card| card.source_id == "local.project-fin")
        .expect("project card timeout");
    assert_eq!(project.state, "waiting");
    assert_eq!(
        project.current_activity.as_deref(),
        Some("delegated task waiting")
    );
    assert!(
        project
            .waiting_detail
            .as_deref()
            .unwrap_or_default()
            .contains("collect long-running delegated task status")
    );

    write_json(
        &runs_path,
        &serde_json::json!([{
            "agent_run_id":"project-run-closed",
            "agent_id":"local.project-fin",
            "status":"closed",
            "result_refs":[],
            "last_heartbeat_at":"2026-05-23T00:00:05Z",
            "closed_at":"2026-05-23T00:00:05Z",
            "path":"project:fin:local.project-fin"
        }]),
    );
    write_json(&system_mailbox_path, &serde_json::json!([]));
    let cards = build_activity_cards(&runtime_home).expect("cards closed");
    let project = cards
        .source_cards
        .iter()
        .find(|card| card.source_id == "local.project-fin")
        .expect("project card closed");
    assert_eq!(project.state, "offline");
    assert_eq!(project.current_activity.as_deref(), Some("idle"));
}

#[test]
fn project_agent_card_reflects_disconnect_and_reconnect_from_peer_truth() {
    let runtime_home = temp_runtime_home();
    write_json(
        &runtime_home.join("runtime/current/last_run.json"),
        &serde_json::json!({
            "session_id": "system-agent",
            "task_id": "task-local-multi-agent",
            "submitted_at": "2026-05-23T00:00:00Z"
        }),
    );
    write_json(
        &runtime_home.join("runtime/peers/registry.json"),
        &serde_json::json!({
            "peers": [{
                "peer_id": "local.project-fin",
                "peer_kind": "project_agent",
                "presence_state": "offline",
                "runtime_state": "network_registered",
                "connectivity_state": "disconnected",
                "binding_state": "agent_rpc_lease",
                "lifecycle_state": "degraded",
                "updated_at": "2026-05-23T00:00:05Z"
            }]
        }),
    );
    let cards = build_activity_cards(&runtime_home).expect("cards disconnected");
    let project = cards
        .source_cards
        .iter()
        .find(|card| card.source_id == "local.project-fin")
        .expect("project card disconnected");
    assert_eq!(project.state, "offline");
    assert!(
        project
            .failure_detail
            .as_deref()
            .unwrap_or_default()
            .contains("connectivity disconnected")
    );

    write_json(
        &runtime_home.join("runtime/peers/registry.json"),
        &serde_json::json!({
            "peers": [{
                "peer_id": "local.project-fin",
                "peer_kind": "project_agent",
                "presence_state": "online",
                "runtime_state": "network_registered",
                "connectivity_state": "network_connected",
                "binding_state": "agent_rpc_lease",
                "lifecycle_state": "online",
                "updated_at": "2026-05-23T00:00:06Z"
            }]
        }),
    );
    let cards = build_activity_cards(&runtime_home).expect("cards reconnected");
    let project = cards
        .source_cards
        .iter()
        .find(|card| card.source_id == "local.project-fin")
        .expect("project card reconnected");
    assert_eq!(project.state, "ready");
    assert_eq!(project.current_activity.as_deref(), Some("idle"));
}
