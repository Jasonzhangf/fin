use super::*;

#[test]
fn periodic_delivery_skips_first_ack_equivalent_waiting_snapshot() {
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
        &runtime_home.join("runtime/current/current_execution_state.json"),
        &serde_json::json!({
            "state_id": "state-1",
            "session_id": "session-1",
            "task_id": "task-1",
            "status": "waiting",
            "pending_input_count": 0,
            "accepts_user_input": false,
            "reason": "已收到，正在处理",
            "updated_at": "2026-04-19T12:00:04+08:00"
        }),
    );
    write_json(
        &runtime_home.join(format!("{session_rel}/tools/recent_tool_records.json")),
        &Vec::<serde_json::Value>::new(),
    );
    write_json(
        &runtime_home.join(format!("{session_rel}/turns/recent_turns.json")),
        &Vec::<serde_json::Value>::new(),
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

    complete_builtin_qqbot_pairing(&runtime_home, "session-1", Some(5)).expect("pair");
    bind_target(&runtime_home, "session-1", "qqbot:c2c:user-1").expect("bind");

    let prepared = prepare_periodic_delivery(&runtime_home).expect("prepare");
    assert!(prepared.is_none());
}

#[test]
fn periodic_delivery_emits_once_for_changed_snapshot() {
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
        &runtime_home.join("runtime/current/current_execution_state.json"),
        &serde_json::json!({
            "state_id": "state-1",
            "session_id": "session-1",
            "task_id": "task-1",
            "status": "running",
            "pending_input_count": 0,
            "accepts_user_input": true,
            "updated_at": "2026-04-19T12:00:04+08:00"
        }),
    );
    write_json(
        &runtime_home.join(format!("{session_rel}/tools/recent_tool_records.json")),
        &Vec::<serde_json::Value>::new(),
    );
    write_json(
        &runtime_home.join(format!("{session_rel}/turns/recent_turns.json")),
        &Vec::<serde_json::Value>::new(),
    );
    write_json(
        &runtime_home.join("runtime/peers/registry.json"),
        &serde_json::json!({"peers":[]}),
    );

    complete_builtin_qqbot_pairing(&runtime_home, "session-1", Some(5)).expect("pair");
    bind_target(&runtime_home, "session-1", "qqbot:c2c:user-1").expect("bind");

    let first = prepare_periodic_delivery(&runtime_home)
        .expect("prepare")
        .expect("first delivery");
    assert_eq!(first.reason, "diff");
    assert!(!first.text.is_empty());

    mark_delivered(&runtime_home, &first.signature, &first.text, &first.reason)
        .expect("mark delivered");
    let second = prepare_periodic_delivery(&runtime_home).expect("prepare second");
    assert!(second.is_none());
}

#[test]
fn periodic_delivery_skips_when_rendered_text_is_unchanged_even_if_signature_differs() {
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
        &runtime_home.join("runtime/current/current_execution_state.json"),
        &serde_json::json!({
            "state_id": "state-1",
            "session_id": "session-1",
            "task_id": "task-1",
            "status": "running",
            "pending_input_count": 0,
            "accepts_user_input": true,
            "updated_at": "2026-04-19T12:00:59+08:00"
        }),
    );
    write_json(
        &runtime_home.join(format!("{session_rel}/tools/recent_tool_records.json")),
        &Vec::<serde_json::Value>::new(),
    );
    write_json(
        &runtime_home.join(format!("{session_rel}/turns/recent_turns.json")),
        &Vec::<serde_json::Value>::new(),
    );
    write_json(
        &runtime_home.join("runtime/peers/registry.json"),
        &serde_json::json!({"peers":[]}),
    );

    complete_builtin_qqbot_pairing(&runtime_home, "session-1", Some(5)).expect("pair");
    bind_target(&runtime_home, "session-1", "qqbot:c2c:user-1").expect("bind");

    let current = prepare_periodic_delivery(&runtime_home)
        .expect("prepare current")
        .expect("current delivery");
    write_json(
        &runtime_home.join("runtime/peers/qqbot/activity_delivery_state.json"),
        &serde_json::json!({
            "session_id": "session-1",
            "target": "qqbot:c2c:user-1",
            "last_user_signature": "older-signature-that-does-not-match",
            "last_delivered_text": current.text,

            "last_delivery_reason": current.reason,
            "last_delivery_at": "2026-04-19T12:00:58+08:00",

        }),
    );

    let prepared = prepare_periodic_delivery(&runtime_home).expect("prepare");
    assert!(prepared.is_none());
}

#[test]
fn periodic_delivery_does_not_emit_ack_equivalent_waiting_snapshot() {
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
        &runtime_home.join("runtime/current/current_execution_state.json"),
        &serde_json::json!({
            "state_id": "state-1",
            "session_id": "session-1",
            "task_id": "task-1",
            "status": "waiting",
            "pending_input_count": 0,
            "accepts_user_input": false,
            "reason": "已收到，正在处理",
            "updated_at": "2026-04-19T12:00:04+08:00"
        }),
    );
    write_json(
        &runtime_home.join(format!("{session_rel}/tools/recent_tool_records.json")),
        &Vec::<serde_json::Value>::new(),
    );
    write_json(
        &runtime_home.join(format!("{session_rel}/turns/recent_turns.json")),
        &Vec::<serde_json::Value>::new(),
    );
    write_json(
        &runtime_home.join("runtime/peers/registry.json"),
        &serde_json::json!({"peers":[]}),
    );

    complete_builtin_qqbot_pairing(&runtime_home, "session-1", Some(5)).expect("pair");
    bind_target(&runtime_home, "session-1", "qqbot:c2c:user-1").expect("bind");
    mark_delivered(
        &runtime_home,
        "older-non-ack-signature",
        "older delivery text",
        "diff",
    )
    .expect("seed prior delivery");
    let first = prepare_periodic_delivery(&runtime_home).expect("prepare");
    assert!(first.is_none());
}

#[test]
fn periodic_delivery_skips_idle_peer_only_changes() {
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
        &runtime_home.join("runtime/current/current_execution_state.json"),
        &serde_json::json!({
            "state_id": "state-1",
            "session_id": "session-1",
            "task_id": "task-1",
            "status": "idle",
            "pending_input_count": 0,
            "accepts_user_input": true,
            "reason": "closure completed",
            "updated_at": "2026-04-19T12:00:04+08:00"
        }),
    );
    write_json(
        &runtime_home.join(format!("{session_rel}/tools/recent_tool_records.json")),
        &Vec::<serde_json::Value>::new(),
    );
    write_json(
        &runtime_home.join(format!("{session_rel}/turns/recent_turns.json")),
        &Vec::<serde_json::Value>::new(),
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

    complete_builtin_qqbot_pairing(&runtime_home, "session-1", Some(5)).expect("pair");
    bind_target(&runtime_home, "session-1", "qqbot:c2c:user-1").expect("bind");

    let prepared = prepare_periodic_delivery(&runtime_home).expect("prepare");
    assert!(prepared.is_none());
}

#[test]
fn current_delivery_signature_only_exists_for_deliverable_snapshot() {
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
        &runtime_home.join("runtime/current/current_execution_state.json"),
        &serde_json::json!({
            "state_id": "state-1",
            "session_id": "session-1",
            "task_id": "task-1",
            "status": "running",
            "pending_input_count": 0,
            "accepts_user_input": false,
            "reason": "active closure running",
            "updated_at": "2026-04-19T12:00:04+08:00"
        }),
    );
    write_json(
        &runtime_home.join(format!("{session_rel}/tools/recent_tool_records.json")),
        &Vec::<serde_json::Value>::new(),
    );
    write_json(
        &runtime_home.join(format!("{session_rel}/turns/recent_turns.json")),
        &Vec::<serde_json::Value>::new(),
    );
    write_json(
        &runtime_home.join("runtime/peers/registry.json"),
        &serde_json::json!({"peers":[]}),
    );

    let signature = current_delivery_signature_if_deliverable(&runtime_home).expect("signature");
    assert!(signature.is_some());
}
