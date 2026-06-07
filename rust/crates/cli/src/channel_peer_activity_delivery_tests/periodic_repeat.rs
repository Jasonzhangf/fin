use super::*;

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
    mark_delivered(&runtime_home, &first.signature, &first.text, &first.reason)
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
    mark_delivered(&runtime_home, &first.signature, &first.text, &first.reason)
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
