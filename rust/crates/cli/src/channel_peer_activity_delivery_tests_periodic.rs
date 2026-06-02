use super::*;

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

    mark_delivered(
        &runtime_home,
        &first.signature,
        &first.text,
        &first.reason,
    )
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

#[test]
fn periodic_delivery_stays_silent_when_text_is_unchanged_even_if_interval_elapsed() {
    let runtime_home = temp_runtime_home();
    let session_rel = "sessions/2026/04/session-1";
    write_json(
        &runtime_home.join("runtime/current/last_run.json"),
        &serde_json::json!({
            "session_id": "session-1",
            "task_id": "task-1",
            "submitted_at": "2026-04-23T12:00:00+08:00",
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
            "reason": "dispatching workers",
            "updated_at": "2026-04-23T12:00:04+08:00"
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
        .expect("prepare first")
        .expect("first delivery");
    mark_delivered(
        &runtime_home,
        &first.signature,
        &first.text,
        &first.reason,
    )
    .expect("mark first");
    write_json(
        &runtime_home.join("runtime/peers/qqbot/activity_delivery_state.json"),
        &serde_json::json!({
            "session_id": "session-1",
            "target": "qqbot:c2c:user-1",
            "last_user_signature": first.signature,
            "last_delivered_text": first.text,
            
            "last_delivery_reason": first.reason,
            "last_delivery_at": "2000-01-01T00:00:00+08:00",
            
        }),
    );

    let heartbeat = prepare_periodic_delivery(&runtime_home).expect("prepare heartbeat");
    assert!(heartbeat.is_none());
}

#[test]
fn periodic_delivery_does_not_repeat_stale_waiting_failure_snapshot() {
    let runtime_home = temp_runtime_home();
    let session_rel = "sessions/2026/04/session-stale-heartbeat";
    write_json(
        &runtime_home.join("runtime/current/last_run.json"),
        &serde_json::json!({
            "session_id": "session-stale-heartbeat",
            "task_id": "task-stale-heartbeat",
            "submitted_at": "2026-04-24T18:00:00+08:00",
            "session_recent_tool_records_path": format!("{session_rel}/tools/recent_tool_records.json"),
            "session_recent_turns_path": format!("{session_rel}/turns/recent_turns.json"),
            "current_execution_state_path": "runtime/current/current_execution_state.json"
        }),
    );
    write_json(
        &runtime_home.join("runtime/current/current_execution_state.json"),
        &serde_json::json!({
            "state_id": "state-stale-heartbeat",
            "session_id": "session-stale-heartbeat",
            "task_id": "task-stale-heartbeat",
            "status": "waiting",
            "pending_input_count": 0,
            "accepts_user_input": true,
            "reason": "最近已派发 5 个任务；当前关注 task-fin-peer-mcp-computer-toolu_b8...",
            "updated_at": "2026-04-24T18:00:04+08:00"
        }),
    );
    write_json(
        &runtime_home.join(format!("{session_rel}/turns/recent_turns.json")),
        &Vec::<serde_json::Value>::new(),
    );
    write_json(
        &runtime_home.join(format!("{session_rel}/tools/recent_tool_records.json")),
        &vec![
            serde_json::json!({
                "tool_call_id": "tool-failed-plan",
                "operation_id": "op-stale-heartbeat",
                "trace_id": "trace-stale-heartbeat",
                "session_id": "session-stale-heartbeat",
                "task_id": "task-stale-heartbeat",
                "worker_id": "worker-system",
                "tool_name": "update_plan",
                "tool_kind": "model_tool",
                "title": "Updated plan",
                "purpose": "plan",
                "target_kind": "plan",
                "target_ref": "plan",
                "input_summary": "steps missing",
                "output_summary": "failure_kind=validation_error · correction=补全 steps 数组 · retry_hint=补充 steps[{step,status}] 后重试",
                "status": "failed",
                "started_at": "2026-04-24T18:00:01+08:00",
                "ended_at": "2026-04-24T18:00:01+08:00",
                "side_effects": [],
                "artifact_refs": []
            }),
            serde_json::json!({
                "tool_call_id": "tool-task-list",
                "operation_id": "op-stale-heartbeat",
                "trace_id": "trace-stale-heartbeat",
                "session_id": "session-stale-heartbeat",
                "task_id": "task-stale-heartbeat",
                "worker_id": "worker-system",
                "tool_name": "project.task.list",
                "tool_kind": "model_tool",
                "title": "Listed tasks",
                "purpose": "query",
                "target_kind": "task_board",
                "target_ref": "tasks",
                "input_summary": "limit=10",
                "output_summary": "tasks=1 [task-20260420075356@ready]",
                "status": "completed",
                "started_at": "2026-04-24T18:00:02+08:00",
                "ended_at": "2026-04-24T18:00:02+08:00",
                "side_effects": [],
                "artifact_refs": []
            }),
        ],
    );
    write_json(
        &runtime_home.join("runtime/peers/registry.json"),
        &serde_json::json!({"peers":[]}),
    );
    write_json(
        &runtime_home.join("runtime/current/current_agent_presence_registry.json"),
        &serde_json::json!({
            "agents": [
                {
                    "agent_id":"mac.system",
                    "agent_name":"system",
                    "device_name":"mac",
                    "role_id":"system",
                    "agent_kind":"system_entry",
                    "status":"waiting",
                    "updated_at":"2026-04-24T18:00:04+08:00",
                    "progress_summary":"owner waiting for delegated workers"
                }
            ]
        }),
    );

    complete_builtin_qqbot_pairing(&runtime_home, "session-stale-heartbeat", Some(5))
        .expect("pair");
    bind_target(
        &runtime_home,
        "session-stale-heartbeat",
        "qqbot:c2c:user-stale-heartbeat",
    )
    .expect("bind");

    let first = prepare_periodic_delivery(&runtime_home)
        .expect("prepare first")
        .expect("first delivery");
    mark_delivered(
        &runtime_home,
        &first.signature,
        &first.text,
        &first.reason,
    )
    .expect("mark delivered");
    write_json(
        &runtime_home.join("runtime/peers/qqbot/activity_delivery_state.json"),
        &serde_json::json!({
            "session_id": "session-stale-heartbeat",
            "target": "qqbot:c2c:user-stale-heartbeat",
            "last_user_signature": first.signature,
            "last_delivered_text": first.text,
            
            "last_delivery_reason": first.reason,
            "last_delivery_at": "2000-01-01T00:00:00+08:00",
            
        }),
    );

    let prepared = prepare_periodic_delivery(&runtime_home).expect("prepare second");
    assert!(prepared.is_none());
}

#[test]
fn periodic_delivery_discloses_task_list_detail_once_until_task_set_changes() {
    let runtime_home = temp_runtime_home();
    let session_rel = "sessions/2026/04/session-task-list-memory";
    write_json(
        &runtime_home.join("runtime/peers/registry.json"),
        &serde_json::json!({"peers":[]}),
    );
    write_json(
        &runtime_home.join("runtime/current/current_agent_presence_registry.json"),
        &serde_json::json!({
            "agents": [
                {
                    "agent_id":"mac.system",
                    "agent_name":"system",
                    "device_name":"mac",
                    "role_id":"system",
                    "agent_kind":"system_entry",
                    "status":"waiting",
                    "updated_at":"2026-04-24T18:10:04+08:00",
                    "progress_summary":"owner waiting for delegated workers"
                }
            ]
        }),
    );
    write_json(
        &runtime_home.join(format!("{session_rel}/turns/recent_turns.json")),
        &Vec::<serde_json::Value>::new(),
    );
    complete_builtin_qqbot_pairing(&runtime_home, "session-task-list-memory", Some(5))
        .expect("pair");
    bind_target(
        &runtime_home,
        "session-task-list-memory",
        "qqbot:c2c:user-task-list-memory",
    )
    .expect("bind");

    let write_round = |operation_id: &str,
                       task_line: &str,
                       mailbox_messages: usize,
                       submitted_at: &str,
                       updated_at: &str| {
        write_json(
            &runtime_home.join("runtime/current/last_run.json"),
            &serde_json::json!({
                "session_id": "session-task-list-memory",
                "operation_id": operation_id,
                "task_id": "task-task-list-memory",
                "submitted_at": submitted_at,
                "session_recent_tool_records_path": format!("{session_rel}/tools/recent_tool_records.json"),
                "session_recent_turns_path": format!("{session_rel}/turns/recent_turns.json"),
                "current_execution_state_path": "runtime/current/current_execution_state.json"
            }),
        );
        write_json(
            &runtime_home.join("runtime/current/current_execution_state.json"),
            &serde_json::json!({
                "state_id": format!("state-{operation_id}"),
                "session_id": "session-task-list-memory",
                "pending_input_count": 0,
                "accepts_user_input": true,
                "status": "waiting",
                "reason": "等待 worker 回报",
                "updated_at": updated_at
            }),
        );
        write_json(
            &runtime_home.join(format!("{session_rel}/tools/recent_tool_records.json")),
            &vec![
                serde_json::json!({
                    "tool_call_id": format!("tool-task-list-{operation_id}"),
                    "operation_id": operation_id,
                    "trace_id": format!("trace-{operation_id}"),
                    "session_id": "session-task-list-memory",
                    "task_id": "task-task-list-memory",
                    "worker_id": "worker-system",
                    "tool_name": "project.task.list",
                    "tool_kind": "model_tool",
                    "title": "Listed tasks",
                    "purpose": "query",
                    "target_kind": "task_board",
                    "target_ref": "tasks",
                    "input_summary": "limit=10",
                    "output_summary": task_line,
                    "status": "completed",
                    "started_at": submitted_at,
                    "ended_at": submitted_at,
                    "side_effects": [],
                    "artifact_refs": []
                }),
                serde_json::json!({
                    "tool_call_id": format!("tool-mailbox-{operation_id}"),
                    "operation_id": operation_id,
                    "trace_id": format!("trace-{operation_id}"),
                    "session_id": "session-task-list-memory",
                    "task_id": "task-task-list-memory",
                    "worker_id": "worker-system",
                    "tool_name": "mailbox.poll",
                    "tool_kind": "model_tool",
                    "title": "Poll mailbox",
                    "purpose": "query",
                    "target_kind": "peer_mailbox",
                    "target_ref": "worker-system",
                    "input_summary": "peer_id=worker-system, limit=10, consume=false",
                    "output_summary": format!("messages={mailbox_messages}, remaining=0, ids="),
                    "status": "completed",
                    "started_at": submitted_at,
                    "ended_at": submitted_at,
                    "side_effects": [],
                    "artifact_refs": []
                }),
            ],
        );
    };

    write_round(
        "op-task-list-1",
        "tasks=2 [task-alpha@session-a, task-beta@session-b]",
        0,
        "2026-04-24T18:10:01+08:00",
        "2026-04-24T18:10:04+08:00",
    );
    let first = prepare_periodic_delivery(&runtime_home)
        .expect("prepare first")
        .expect("first delivery");
    assert!(first.text.contains("task-alpha"));
    assert!(!first.text.contains("收取 worker-system：0 条，余 0"));
    mark_delivered(
        &runtime_home,
        &first.signature,
        &first.text,
        &first.reason,
    )
    .expect("mark first");
    // Reset last_delivery_at to bypass MIN_PERIODIC_DELIVERY_SECS in test
    write_json(
        &runtime_home.join("runtime/peers/qqbot/activity_delivery_state.json"),
        &serde_json::json!({
            "session_id": "session-task-list-memory",
            "target": "qqbot:c2c:user-task-list-memory",
            "last_user_signature": first.signature,
            "last_delivered_text": first.text,
            
            "last_delivery_reason": first.reason,
            "last_delivery_at": "2000-01-01T00:00:00+08:00",
        }),
    );

    write_round(
        "op-task-list-2",
        "tasks=2 [task-alpha@session-a, task-beta@session-b]",
        1,
        "2026-04-24T18:11:01+08:00",
        "2026-04-24T18:11:04+08:00",
    );
    // second round: same task list text + different mailbox count → rendered text identical → skipped
    let second = prepare_periodic_delivery(&runtime_home).expect("prepare second");
    assert!(second.is_none());

    write_round(
        "op-task-list-3",
        "tasks=2 [task-alpha@session-a, task-gamma@session-c]",
        2,
        "2026-04-24T18:12:01+08:00",
        "2026-04-24T18:12:04+08:00",
    );
    let third = prepare_periodic_delivery(&runtime_home)
        .expect("prepare third")
        .expect("third delivery");
    assert!(third.text.contains("task-gamma"));
}

#[test]
fn periodic_delivery_emits_when_project_worker_is_running_even_if_system_is_idle() {
    let runtime_home = temp_runtime_home();
    let session_rel = "sessions/2026/04/session-1";
    write_json(
        &runtime_home.join("runtime/current/last_run.json"),
        &serde_json::json!({
            "session_id": "session-1",
            "task_id": "task-1",
            "submitted_at": "2026-04-23T12:30:00+08:00",
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
            "reason": "owner dispatched work",
            "updated_at": "2026-04-23T12:30:04+08:00"
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
    write_json(
        &runtime_home.join("runtime/current/current_agent_presence_registry.json"),
        &serde_json::json!({
            "agents": [
                {
                    "agent_id":"mac.system",
                    "agent_name":"system",
                    "device_name":"mac",
                    "role_id":"system",
                    "agent_kind":"system_entry",
                    "status":"idle",
                    "updated_at":"2026-04-23T12:30:01+08:00",
                    "progress_summary":"system idle"
                },
                {
                    "agent_id":"mac.worker-builder",
                    "agent_name":"worker-builder",
                    "device_name":"mac",
                    "role_id":"project",
                    "agent_kind":"project_worker",
                    "project_id":"fin",
                    "status":"busy",
                    "current_task_id":"task-1",
                    "current_session_id":"session-1",
                    "current_phase":"reasoning",
                    "updated_at":"2026-04-23T12:30:02+08:00",
                    "progress_summary":"running patch task"
                }
            ]
        }),
    );

    complete_builtin_qqbot_pairing(&runtime_home, "session-1", Some(5)).expect("pair");
    bind_target(&runtime_home, "session-1", "qqbot:c2c:user-1").expect("bind");

    let prepared = prepare_periodic_delivery(&runtime_home)
        .expect("prepare")
        .expect("worker progress should emit");
    assert!(prepared.text.contains("worker-builder"));
}

#[test]
fn periodic_delivery_stays_bound_to_frontstage_session_when_worker_session_becomes_last_run() {
    let runtime_home = temp_runtime_home();
    let frontstage_rel = "sessions/2026/04/session-frontstage";
    let worker_rel = "sessions/2026/04/session-worker";
    write_json(
        &runtime_home.join("runtime/current/last_run.json"),
        &serde_json::json!({
            "session_id": "session-worker",
            "task_id": "task-worker",
            "submitted_at": "2026-04-24T15:30:00+08:00",
            "session_recent_tool_records_path": format!("{worker_rel}/tools/recent_tool_records.json"),
            "session_recent_turns_path": format!("{worker_rel}/turns/recent_turns.json"),
            "current_execution_state_path": "runtime/current/current_execution_state.json"
        }),
    );
    write_json(
        &runtime_home.join(format!("{frontstage_rel}/tools/recent_tool_records.json")),
        &Vec::<serde_json::Value>::new(),
    );
    write_json(
        &runtime_home.join(format!("{frontstage_rel}/turns/recent_turns.json")),
        &Vec::<serde_json::Value>::new(),
    );
    write_json(
        &runtime_home.join(format!("{worker_rel}/tools/recent_tool_records.json")),
        &Vec::<serde_json::Value>::new(),
    );
    write_json(
        &runtime_home.join(format!("{worker_rel}/turns/recent_turns.json")),
        &Vec::<serde_json::Value>::new(),
    );
    write_json(
        &runtime_home.join("runtime/current/current_execution_state.json"),
        &serde_json::json!({
            "state_id": "state-worker",
            "session_id": "session-worker",
            "task_id": "task-worker",
            "status": "idle",
            "pending_input_count": 0,
            "accepts_user_input": true,
            "updated_at": "2026-04-24T15:30:01+08:00"
        }),
    );
    write_json(
        &runtime_home.join("runtime/peers/registry.json"),
        &serde_json::json!({"peers":[]}),
    );
    write_json(
        &runtime_home.join("runtime/current/current_agent_presence_registry.json"),
        &serde_json::json!({
            "agents": [
                {
                    "agent_id":"mac.system",
                    "agent_name":"system",
                    "device_name":"mac",
                    "role_id":"system",
                    "agent_kind":"system_entry",
                    "status":"idle",
                    "updated_at":"2026-04-24T15:30:00+08:00",
                    "progress_summary":"frontstage idle"
                },
                {
                    "agent_id":"mac.atlas",
                    "agent_name":"atlas",
                    "device_name":"mac",
                    "worker_id":"worker-atlas",
                    "role_id":"project",
                    "agent_kind":"project_worker",
                    "project_id":"fin",
                    "status":"busy",
                    "current_task_id":"task-frontstage",
                    "current_session_id":"session-frontstage",
                    "current_phase":"reasoning",
                    "updated_at":"2026-04-24T15:30:02+08:00",
                    "progress_summary":"worker running delegated task",
                    "recent_actions":[
                        {
                            "tool_call_id":"tool-1",
                            "operation_id":"op-1",
                            "tool_name":"agent.assign",
                            "category":"delegation",
                            "verb":"Delegated",
                            "object_kind":"peer",
                            "object_label":"atlas",
                            "summary":"Delegated atlas",
                            "detail":"agent_name=atlas, worker_id=worker-atlas; task=inspect logs → assignment queued",
                            "status":"completed",
                            "started_at":"2026-04-24T15:30:02+08:00"
                        }
                    ]
                }
            ]
        }),
    );

    complete_builtin_qqbot_pairing(&runtime_home, "session-frontstage", Some(5)).expect("pair");
    bind_target(
        &runtime_home,
        "session-frontstage",
        "qqbot:c2c:user-frontstage",
    )
    .expect("bind");

    let prepared = prepare_periodic_delivery(&runtime_home)
        .expect("prepare")
        .expect("frontstage delivery");
    assert!(prepared.text.contains("atlas"));
    let state = load_state(&runtime_home).expect("state");
    assert_eq!(state.session_id.as_deref(), Some("session-frontstage"));
}
