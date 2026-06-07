use super::*;

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
    mark_delivered(&runtime_home, &first.signature, &first.text, &first.reason)
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
