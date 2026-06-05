use crate::tools::test_helpers::{context_with_runtime_home, refs, temp_runtime_home};
use crate::model_output::ModelToolCall;
use super::tool_dispatch::execute_model_tools;
use super::tool_dispatch as tool_dispatch;
use serde_json::json;
use std::{
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

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
            tool_call_id: None,
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
            tool_call_id: None,
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
