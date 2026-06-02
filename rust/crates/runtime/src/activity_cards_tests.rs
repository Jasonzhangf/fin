use crate::build_activity_cards;
use fin_contracts::{EntityRefs, ToolExecutionRecord, TurnRecord};
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
        "fin-activity-cards-{}-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos(),
        seq
    ))
}

#[test]
fn provider_call_is_not_preferred_over_real_tool_actions_in_recent_items() {
    let runtime_home = temp_runtime_home();
    let session_rel = "sessions/2026/04/session-1";
    write_json(
        &runtime_home.join("runtime/current/last_run.json"),
        &serde_json::json!({
            "session_id": "session-1",
            "task_id": "task-1",
            "submitted_at": "2026-04-19T12:00:00+08:00",
            "session_recent_tool_records_path": format!("{session_rel}/tools/recent_tool_records.json"),
            "session_recent_turns_path": format!("{session_rel}/turns/recent_turns.json"),
            "current_execution_state_path": "runtime/current/current_execution_state.json"
        }),
    );
    write_json(
        &runtime_home.join(format!("{session_rel}/tools/recent_tool_records.json")),
        &vec![
            ToolExecutionRecord {
                tool_call_id: "tool-provider".into(),
                operation_id: "op-1".into(),
                trace_id: "trace-1".into(),
                refs: EntityRefs::default(),
                tool_name: "provider.call".into(),
                target_ref: Some(
                    "ali-coding-plan.qwen3.6-plus @ https://example.com/v1/messages".into(),
                ),
                input_summary: Some("用户提示词".into()),
                output_summary: Some("模型输出".into()),
                status: "completed".into(),
                started_at: "2026-04-19T12:00:02+08:00".into(),
                ended_at: Some("2026-04-19T12:00:03+08:00".into()),
                ..ToolExecutionRecord::default()
            },
            ToolExecutionRecord {
                tool_call_id: "tool-exec".into(),
                operation_id: "op-1".into(),
                trace_id: "trace-1".into(),
                refs: EntityRefs::default(),
                tool_name: "exec_command".into(),
                input_summary: Some("cmd=cargo test".into()),
                output_summary: Some("exit_code=0".into()),
                status: "completed".into(),
                started_at: "2026-04-19T12:00:01+08:00".into(),
                ended_at: Some("2026-04-19T12:00:02+08:00".into()),
                ..ToolExecutionRecord::default()
            },
        ],
    );
    write_json(
        &runtime_home.join(format!("{session_rel}/turns/recent_turns.json")),
        &Vec::<serde_json::Value>::new(),
    );
    write_json(
        &runtime_home.join("runtime/current/current_execution_state.json"),
        &serde_json::json!({
            "state_id": "state-1",
            "status": "running",
            "pending_input_count": 0,
            "accepts_user_input": true,
            "updated_at": "2026-04-19T12:00:04+08:00"
        }),
    );
    write_json(
        &runtime_home.join("runtime/peers/registry.json"),
        &serde_json::json!({"peers":[]}),
    );
    let cards = build_activity_cards(&runtime_home).expect("cards");
    let user = cards.user_card.expect("user card");
    assert!(
        user.recent_items
            .iter()
            .any(|item| item.contains("Ran cmd=cargo test"))
    );
    assert!(
        !user
            .recent_items
            .iter()
            .any(|item| item.contains("https://"))
    );
    assert!(
        !user
            .recent_items
            .iter()
            .any(|item| item.contains("用户提示词"))
    );
}

fn write_json(path: &Path, value: &impl serde::Serialize) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("create parent");
    }
    fs::write(path, serde_json::to_vec_pretty(value).expect("json")).expect("write");
}

#[test]
fn build_activity_cards_collects_system_and_peer_views() {
    let runtime_home = temp_runtime_home();
    let session_rel = "sessions/2026/04/session-1";
    write_json(
        &runtime_home.join("runtime/current/last_run.json"),
        &serde_json::json!({
            "session_id": "session-1",
            "task_id": "task-1",
            "submitted_at": "2026-04-19T12:00:00+08:00",
            "session_recent_tool_records_path": format!("{session_rel}/tools/recent_tool_records.json"),
            "session_recent_turns_path": format!("{session_rel}/turns/recent_turns.json"),
            "current_execution_state_path": "runtime/current/current_execution_state.json"
        }),
    );
    write_json(
        &runtime_home.join(format!("{session_rel}/tools/recent_tool_records.json")),
        &vec![ToolExecutionRecord {
            tool_call_id: "tool-1".into(),
            operation_id: "op-1".into(),
            trace_id: "trace-1".into(),
            refs: EntityRefs {
                session_id: Some("session-1".into()),
                task_id: Some("task-1".into()),
                ..EntityRefs::default()
            },
            tool_name: "exec_command".into(),
            tool_kind: "agent_tool".into(),
            title: "Execute Local Command".into(),
            purpose: "run one command".into(),
            input_summary: Some("cmd=cargo test".into()),
            output_summary: Some("exit_code=0".into()),
            status: "completed".into(),
            started_at: "2026-04-19T12:00:01+08:00".into(),
            ended_at: Some("2026-04-19T12:00:02+08:00".into()),
            duration_ms: Some(1000),
            ..ToolExecutionRecord::default()
        }],
    );
    write_json(
        &runtime_home.join(format!("{session_rel}/turns/recent_turns.json")),
        &vec![TurnRecord {
            turn_id: "turn-1".into(),
            operation_id: "op-1".into(),
            status: "completed".into(),
            user_input: "run tests".into(),
            progress_summary: Some("running regression".into()),
            created_at: "2026-04-19T12:00:00+08:00".into(),
            completed_at: Some("2026-04-19T12:00:03+08:00".into()),
            ..TurnRecord::default()
        }],
    );
    write_json(
        &runtime_home.join("runtime/current/current_execution_state.json"),
        &serde_json::json!({
            "state_id": "state-1",
            "status": "running",
            "pending_input_count": 0,
            "accepts_user_input": true,
            "updated_at": "2026-04-19T12:00:04+08:00"
        }),
    );
    write_json(
        &runtime_home.join("runtime/peers/registry.json"),
        &serde_json::json!({
            "peers": [{
                "peer_id": "peer-channel-gateway-qqbot-local",
                "peer_kind": "channel_gateway.qqbot",
                "presence_state": "online",
                "runtime_state": "bridge_ready",
                "connectivity_state": "connected",
                "binding_state": "bound",
                "lifecycle_state": "paired_active",
                "updated_at": "2026-04-19T12:00:05+08:00",
                "pairing_required": false,
                "session_valid": true,
                "session_id": "session-1"
            }]
        }),
    );

    let cards = build_activity_cards(&runtime_home).expect("cards");
    assert_eq!(cards.session_id.as_deref(), Some("session-1"));
    assert_eq!(cards.source_cards.len(), 2);
    assert_eq!(cards.tool_semantics.len(), 1);
    assert_eq!(cards.tool_semantics[0].category, "command");
    assert_eq!(cards.source_cards[0].source_id, "system-agent");
    assert_eq!(cards.source_cards[0].state, "running");
    assert!(
        cards
            .user_card
            .as_ref()
            .expect("user card")
            .header
            .contains("system frontstage")
    );
}

#[test]
fn build_activity_cards_falls_back_to_startup_summary_after_restart() {
    let runtime_home = temp_runtime_home();
    write_json(
        &runtime_home.join("runtime/current/last_run.json"),
        &serde_json::json!({
            "session_id": "session-restart",
            "task_id": "task-restart",
            "submitted_at": "2026-04-20T23:10:00+08:00"
        }),
    );
    write_json(
        &runtime_home.join("runtime/current/current_startup_control_summary.json"),
        &serde_json::json!({
            "startup_config_summary": "startup config · system_workers=4 · project_workers=2 · projects=1",
            "startup_state_summary": "startup state · started=3 [mbp.system-worker-01:idle, mbp.builder:busy] · busy=1 [mbp.builder] · waiting=0 · wake_actions=1",
            "started_resource_count": 3,
            "busy_resource_count": 1
        }),
    );
    write_json(
        &runtime_home.join("runtime/peers/registry.json"),
        &serde_json::json!({"peers":[]}),
    );

    let cards = build_activity_cards(&runtime_home).expect("cards");
    let system = cards
        .source_cards
        .iter()
        .find(|card| card.source_id == "system-agent")
        .expect("system card");
    assert_eq!(
        system.summary,
        "startup config · system_workers=4 · project_workers=2 · projects=1"
    );
    assert!(
        system
            .current_activity
            .as_deref()
            .is_some_and(|value| value.contains("started=3"))
    );
}

#[test]
fn unbound_peer_card_reports_restore_state_not_stale_bound_session() {
    let runtime_home = temp_runtime_home();
    write_json(
        &runtime_home.join("runtime/current/last_run.json"),
        &serde_json::json!({
            "session_id": "session-new",
            "task_id": "task-new",
            "submitted_at": "2026-04-20T07:53:56+08:00"
        }),
    );
    write_json(
        &runtime_home.join("runtime/peers/registry.json"),
        &serde_json::json!({
            "peers": [{
                "peer_id": "peer-channel-gateway-qqbot-local",
                "peer_kind": "channel_gateway.qqbot",
                "presence_state": "online",
                "runtime_state": "bridge_ready",
                "connectivity_state": "connected",
                "binding_state": "unbound",
                "lifecycle_state": "idle_ready",
                "updated_at": "2026-04-20T07:53:56+08:00",
                "pairing_required": false,
                "session_valid": false,
                "session_id": null
            }]
        }),
    );

    let cards = build_activity_cards(&runtime_home).expect("cards");
    let peer = cards
        .source_cards
        .iter()
        .find(|card| card.source_id == "peer-channel-gateway-qqbot-local")
        .expect("peer card");
    assert_eq!(
        peer.current_activity.as_deref(),
        Some("waiting inbound session restore")
    );
    assert_eq!(
        cards
            .user_card
            .as_ref()
            .map(|card| card.focus_source_id.as_deref()),
        Some(Some("system-agent"))
    );
}

#[test]
fn pending_inbound_notice_overrides_stale_system_recent_action() {
    let runtime_home = temp_runtime_home();
    let session_rel = "sessions/2026/04/session-1";
    write_json(
        &runtime_home.join("runtime/current/last_run.json"),
        &serde_json::json!({
            "session_id": "session-1",
            "task_id": "task-1",
            "submitted_at": "2026-04-20T14:21:56+08:00",
            "session_recent_tool_records_path": format!("{session_rel}/tools/recent_tool_records.json"),
            "session_recent_turns_path": format!("{session_rel}/turns/recent_turns.json"),
            "current_execution_state_path": "runtime/current/current_execution_state.json"
        }),
    );
    write_json(
        &runtime_home.join(format!("{session_rel}/tools/recent_tool_records.json")),
        &vec![ToolExecutionRecord {
            tool_call_id: "tool-stop".into(),
            operation_id: "op-prev".into(),
            trace_id: "trace-prev".into(),
            refs: EntityRefs::default(),
            tool_name: "reasoning.stop".into(),
            status: "completed".into(),
            input_summary: Some("stop".into()),
            output_summary: Some("stopped".into()),
            started_at: "2026-04-20T14:21:00+08:00".into(),
            ended_at: Some("2026-04-20T14:21:00+08:00".into()),
            ..ToolExecutionRecord::default()
        }],
    );
    write_json(
        &runtime_home.join(format!("{session_rel}/turns/recent_turns.json")),
        &vec![TurnRecord {
            turn_id: "turn-prev".into(),
            operation_id: "op-prev".into(),
            status: "completed".into(),
            progress_summary: Some("Stopped current_turn".into()),
            created_at: "2026-04-20T14:21:00+08:00".into(),
            completed_at: Some("2026-04-20T14:21:00+08:00".into()),
            ..TurnRecord::default()
        }],
    );
    write_json(
        &runtime_home.join("runtime/current/current_execution_state.json"),
        &serde_json::json!({
            "state_id": "state-1",
            "session_id": "session-1",
            "task_id": "task-1",
            "status": "running",
            "pending_input_count": 0,
            "accepts_user_input": false,
            "reason": "active closure running",
            "updated_at": "2026-04-20T14:21:56+08:00"
        }),
    );
    write_json(
        &runtime_home.join("runtime/peers/registry.json"),
        &serde_json::json!({
            "peers": [{
                "peer_id": "peer-channel-gateway-qqbot-local",
                "peer_kind": "channel_gateway.qqbot",
                "presence_state": "online",
                "runtime_state": "bridge_ready",
                "connectivity_state": "connected",
                "binding_state": "bound",
                "lifecycle_state": "paired_active",
                "updated_at": "2026-04-20T14:21:56+08:00",
                "pairing_required": false,
                "session_valid": true,
                "session_id": "session-1"
            }]
        }),
    );
    write_json(
        &runtime_home.join("runtime/channels/qqbot/conversations.json"),
        &serde_json::json!({
            "conversations": [{
                "session_id": "session-1",
                "status": "bound",
                "last_inbound_message_id": "inbound-1",
                "last_inbound_at": "2026-04-20T14:21:55+08:00",
                "last_delivered_message_id": "assistant-old",
                "last_delivery_at": "2026-04-20T14:21:54+08:00"
            }]
        }),
    );

    let cards = build_activity_cards(&runtime_home).expect("cards");
    let user_card = cards.user_card.expect("user card");
    assert_eq!(user_card.state, "waiting");
    assert_eq!(user_card.focus_source_id.as_deref(), Some("system-agent"));
    assert_eq!(user_card.stage.as_deref(), Some("已收到，正在处理"));
    let system = cards
        .source_cards
        .iter()
        .find(|card| card.source_id == "system-agent")
        .expect("system");
    assert_eq!(system.summary, "已收到，正在处理");
    assert!(system.recent_actions.is_empty());
}

#[test]
fn pending_inbound_notice_clears_after_delivery_time_passes_inbound() {
    let runtime_home = temp_runtime_home();
    write_json(
        &runtime_home.join("runtime/current/last_run.json"),
        &serde_json::json!({
            "session_id": "session-1",
            "task_id": "task-1",
            "submitted_at": "2026-04-20T14:22:43+08:00"
        }),
    );
    write_json(
        &runtime_home.join("runtime/peers/registry.json"),
        &serde_json::json!({"peers":[]}),
    );
    write_json(
        &runtime_home.join("runtime/channels/qqbot/conversations.json"),
        &serde_json::json!({
            "conversations": [{
                "session_id": "session-1",
                "status": "bound",
                "last_inbound_message_id": "inbound-1",
                "last_inbound_at": "2026-04-20T14:21:55+08:00",
                "last_delivered_message_id": "assistant-1",
                "last_delivery_at": "2026-04-20T14:22:43+08:00"
            }]
        }),
    );

    let cards = build_activity_cards(&runtime_home).expect("cards");
    let user_card = cards.user_card.expect("user card");
    assert_ne!(user_card.stage.as_deref(), Some("已收到，正在处理"));
}
