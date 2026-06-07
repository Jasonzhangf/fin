use super::tests::{temp_runtime_home, write_json};
use super::*;
use fin_contracts::{EntityRefs, ToolExecutionRecord, TurnRecord};

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
        &vec![
            ToolExecutionRecord {
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
            },
            ToolExecutionRecord {
                tool_call_id: "tool-failed".into(),
                operation_id: "op-prev".into(),
                trace_id: "trace-prev".into(),
                refs: EntityRefs::default(),
                tool_name: "project.task.status".into(),
                status: "failed".into(),
                error_summary: Some("task not found in runtime truth: task-missing".into()),
                started_at: "2026-04-20T14:21:01+08:00".into(),
                ended_at: Some("2026-04-20T14:21:01+08:00".into()),
                ..ToolExecutionRecord::default()
            },
        ],
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
    assert_eq!(user_card.failure_detail, None);
    let system = cards
        .source_cards
        .iter()
        .find(|card| card.source_id == "system-agent")
        .expect("system");
    assert_eq!(system.summary, "已收到，正在处理");
    assert!(system.recent_actions.is_empty());
    assert_eq!(system.failure_detail, None);
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

#[test]
fn failed_activity_prefers_humanized_detail_over_raw_failed_summary() {
    let runtime_home = temp_runtime_home();
    let session_rel = "sessions/2026/04/session-1";
    write_json(
        &runtime_home.join("runtime/current/last_run.json"),
        &serde_json::json!({
            "session_id": "session-1",
            "submitted_at": "2026-04-22T22:30:34+08:00",
            "session_recent_tool_records_path": format!("{session_rel}/tools/recent_tool_records.json"),
            "session_recent_turns_path": format!("{session_rel}/turns/recent_turns.json"),
            "current_execution_state_path": "runtime/current/current_execution_state.json"
        }),
    );
    write_json(
        &runtime_home.join(format!("{session_rel}/tools/recent_tool_records.json")),
        &vec![ToolExecutionRecord {
            tool_call_id: "tool-failed".into(),
            operation_id: "op-1".into(),
            trace_id: "trace-1".into(),
            refs: EntityRefs::default(),
            tool_name: "project.task.status".into(),
            status: "failed".into(),
            error_summary: Some("task not found in runtime truth: task-missing".into()),
            started_at: "2026-04-22T22:30:34+08:00".into(),
            ended_at: Some("2026-04-22T22:30:34+08:00".into()),
            ..ToolExecutionRecord::default()
        }],
    );
    write_json(
        &runtime_home.join(format!("{session_rel}/turns/recent_turns.json")),
        &Vec::<serde_json::Value>::new(),
    );
    write_json(
        &runtime_home.join("runtime/current/current_execution_state.json"),
        &serde_json::json!({
            "state_id": "state-1",
            "session_id": "session-1",
            "task_id": null,
            "status": "failed",
            "pending_input_count": 0,
            "accepts_user_input": true,
            "updated_at": "2026-04-22T22:30:34+08:00"
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
        .expect("system");
    assert_eq!(
        system.failure_detail.as_deref(),
        Some("任务不存在：task-missing")
    );
}

#[test]
fn superseded_same_tool_failure_does_not_leak_into_current_failure_detail() {
    let runtime_home = temp_runtime_home();
    let session_rel = "sessions/2026/04/session-1";
    write_json(
        &runtime_home.join("runtime/current/last_run.json"),
        &serde_json::json!({
            "session_id": "session-1",
            "submitted_at": "2026-04-24T22:30:34+08:00",
            "session_recent_tool_records_path": format!("{session_rel}/tools/recent_tool_records.json"),
            "session_recent_turns_path": format!("{session_rel}/turns/recent_turns.json"),
            "current_execution_state_path": "runtime/current/current_execution_state.json"
        }),
    );
    write_json(
        &runtime_home.join(format!("{session_rel}/tools/recent_tool_records.json")),
        &vec![
            ToolExecutionRecord {
                tool_call_id: "tool-status-failed".into(),
                operation_id: "op-1".into(),
                trace_id: "trace-1".into(),
                refs: EntityRefs::default(),
                tool_name: "project.task.status".into(),
                status: "failed".into(),
                output_summary: Some("failure_kind=missing_task_binding · correction=task-bound tool call lacked task identity · retry_hint=retry with explicit task_id".into()),
                error_summary: Some("missing task_id and current refs.task_id is unavailable".into()),
                started_at: "2026-04-24T22:30:34+08:00".into(),
                ended_at: Some("2026-04-24T22:30:34+08:00".into()),
                ..ToolExecutionRecord::default()
            },
            ToolExecutionRecord {
                tool_call_id: "tool-status-ok".into(),
                operation_id: "op-1".into(),
                trace_id: "trace-1".into(),
                refs: EntityRefs::default(),
                tool_name: "project.task.status".into(),
                status: "completed".into(),
                output_summary: Some("session=session-1 status=running pending_inputs=0".into()),
                started_at: "2026-04-24T22:30:35+08:00".into(),
                ended_at: Some("2026-04-24T22:30:35+08:00".into()),
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
            "session_id": "session-1",
            "pending_input_count": 0,
            "accepts_user_input": true,
            "status": "waiting",
            "reason": "等待 worker 回报",
            "updated_at": "2026-04-24T22:30:35+08:00"
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
        .expect("system");
    assert_eq!(system.failure_detail, None);
}

#[test]
fn activity_cards_tool_updates_only_use_current_operation_scope() {
    let runtime_home = temp_runtime_home();
    let session_rel = "sessions/2026/04/session-1";
    write_json(
        &runtime_home.join("runtime/current/last_run.json"),
        &serde_json::json!({
            "session_id": "session-1",
            "operation_id": "op-current",
            "submitted_at": "2026-04-24T22:35:34+08:00",
            "session_recent_tool_records_path": format!("{session_rel}/tools/recent_tool_records.json"),
            "session_recent_turns_path": format!("{session_rel}/turns/recent_turns.json"),
            "current_execution_state_path": "runtime/current/current_execution_state.json"
        }),
    );
    write_json(
        &runtime_home.join(format!("{session_rel}/tools/recent_tool_records.json")),
        &vec![
            ToolExecutionRecord {
                tool_call_id: "tool-old-failed".into(),
                operation_id: "op-old".into(),
                trace_id: "trace-old".into(),
                refs: EntityRefs::default(),
                tool_name: "update_plan".into(),
                status: "failed".into(),
                output_summary: Some("failure_kind=missing_argument · correction=missing required argument `steps[{step,status}]` · retry_hint=retry update_plan with the required argument `steps[{step,status}]` filled in using a concrete value".into()),
                error_summary: Some("missing required argument: steps[{step,status}]".into()),
                started_at: "2026-04-24T22:34:34+08:00".into(),
                ended_at: Some("2026-04-24T22:34:34+08:00".into()),
                ..ToolExecutionRecord::default()
            },
            ToolExecutionRecord {
                tool_call_id: "tool-current-ok".into(),
                operation_id: "op-current".into(),
                trace_id: "trace-current".into(),
                refs: EntityRefs::default(),
                tool_name: "project.task.list".into(),
                status: "completed".into(),
                output_summary: Some("tasks=1 [task-1@ready]".into()),
                started_at: "2026-04-24T22:35:34+08:00".into(),
                ended_at: Some("2026-04-24T22:35:34+08:00".into()),
                ..ToolExecutionRecord::default()
            },
        ],
    );
    write_json(
        &runtime_home.join(format!("{session_rel}/turns/recent_turns.json")),
        &vec![TurnRecord {
            turn_id: "turn-op-current".into(),
            operation_id: "op-current".into(),
            status: "completed".into(),
            progress_summary: Some("current round done".into()),
            created_at: "2026-04-24T22:35:34+08:00".into(),
            completed_at: Some("2026-04-24T22:35:34+08:00".into()),
            ..TurnRecord::default()
        }],
    );
    write_json(
        &runtime_home.join("runtime/current/current_execution_state.json"),
        &serde_json::json!({
            "state_id": "state-1",
            "session_id": "session-1",
            "operation_id": "op-current",
            "pending_input_count": 0,
            "accepts_user_input": true,
            "status": "waiting",
            "reason": "等待当前轮收口",
            "updated_at": "2026-04-24T22:35:34+08:00"
        }),
    );
    write_json(
        &runtime_home.join("runtime/peers/registry.json"),
        &serde_json::json!({"peers":[]}),
    );

    let cards = build_activity_cards(&runtime_home).expect("cards");
    assert_eq!(cards.tool_semantics.len(), 1);
    assert_eq!(cards.tool_semantics[0].operation_id, "op-current");
    let system = cards
        .source_cards
        .iter()
        .find(|card| card.source_id == "system-agent")
        .expect("system");
    assert_eq!(system.failure_detail, None);
    assert!(
        system
            .recent_actions
            .iter()
            .all(|action| action.operation_id == "op-current")
    );
}
