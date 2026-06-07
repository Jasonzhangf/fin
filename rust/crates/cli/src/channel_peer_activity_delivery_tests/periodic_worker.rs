use super::*;

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
