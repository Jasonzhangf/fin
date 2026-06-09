use super::mobile_items::{mobile_history_turns, mobile_tool_item_frame, mobile_tool_records};
use crate::{ChatSendResponse, DebugBinding};
use serde_json::json;
use std::{
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

#[test]
fn mobile_tool_records_filters_current_operation_and_projects_item_frames() {
    let runtime_home = temp_runtime_home("fin-ws-mobile-items");
    write_mobile_tool_truth(
        &runtime_home,
        "op-ws-tools",
        json!([
            tool_record(
                "tool-old",
                "op-old",
                "completed",
                "update_plan",
                json!(null)
            ),
            tool_record(
                "tool-visible-plan",
                "op-ws-tools",
                "completed",
                "update_plan",
                json!(null)
            ),
            tool_record(
                "tool-visible-failure",
                "op-ws-tools",
                "failed",
                "shell.exec",
                json!("command failed")
            )
        ]),
    );

    let records = mobile_tool_records(&runtime_home).expect("mobile tool records");
    assert_eq!(records.len(), 2);
    assert!(
        records
            .iter()
            .any(|record| record.tool_call_id == "tool-visible-plan")
    );
    assert!(
        records
            .iter()
            .all(|record| record.operation_id == "op-ws-tools")
    );

    let completed = mobile_tool_item_frame("m-tools", "turn-m-tools", &records[0]);
    let failed = mobile_tool_item_frame("m-tools", "turn-m-tools", &records[1]);
    let joined = format!("{completed}\n{failed}");
    assert!(joined.contains("\"type\":\"turn.item.completed\""));
    assert!(joined.contains("\"item_id\":\"tool-visible-plan\""));
    assert!(joined.contains("\"label\":\"update_plan\""));
    assert!(joined.contains("\"type\":\"turn.item.failed\""));
    assert!(joined.contains("\"item_id\":\"tool-visible-failure\""));
    assert!(joined.contains("\"error_summary\":\"command failed\""));
}

#[test]
fn render_user_input_result_includes_item_frames_and_rendered_tool_records() {
    let runtime_home = temp_runtime_home("fin-ws-mobile-render-items");
    write_mobile_tool_truth(
        &runtime_home,
        "op-ws-render",
        json!([tool_record(
            "tool-render-plan",
            "op-ws-render",
            "completed",
            "update_plan",
            json!(null)
        )]),
    );

    let frames = super::render_user_input_result(
        &runtime_home,
        "m-render",
        "render mobile item",
        Ok(ChatSendResponse {
            binding: binding(&runtime_home),
            answer: "rendered answer".into(),
            digest_id: "digest-render".into(),
            events_count: 1,
            response_kind: "assistant_message".into(),
            freshness: None,
            control_feedback: None,
            progress: None,
            note: None,
            routing_action: None,
        }),
        true,
    );

    assert!(frames[0].contains("\"type\":\"turn.item.completed\""));
    assert!(frames[0].contains("\"item_id\":\"tool-render-plan\""));
    assert!(frames[1].contains("\"type\":\"turn.completed\""));
    assert!(frames[2].contains("\"type\":\"turn.rendered\""));
    assert!(frames[2].contains("\"tool_execution_records\""));
    assert!(frames[2].contains("\"tool_call_id\":\"tool-render-plan\""));
}

#[test]
fn render_user_input_result_does_not_project_old_tool_records_on_failure() {
    let runtime_home = temp_runtime_home("fin-ws-mobile-render-failed");
    write_mobile_tool_truth(
        &runtime_home,
        "op-old",
        json!([tool_record(
            "tool-old",
            "op-old",
            "completed",
            "update_plan",
            json!(null)
        )]),
    );

    let frames = super::render_user_input_result(
        &runtime_home,
        "m-failed",
        "render failure",
        Err("provider failed".into()),
        true,
    );

    assert_eq!(frames.len(), 2);
    assert!(frames[0].contains("\"type\":\"turn.completed\""));
    assert!(frames[0].contains("\"status\":\"failed\""));
    assert!(frames[1].contains("\"type\":\"turn.rendered\""));
    assert!(!frames.iter().any(|frame| frame.contains("turn.item.")));
    assert!(!frames.iter().any(|frame| frame.contains("tool-old")));
}

#[test]
fn mobile_history_turns_project_runtime_turn_schema_to_mobile_render_contract() {
    let turns = mobile_history_turns(vec![json!({
        "turn_id": "turn-op-1",
        "operation_id": "op-1",
        "user_input": "PING-HISTORY",
        "assistant_visible_output": "PONG-HISTORY",
        "tool_record_refs": [
            "tools/recent_tool_records.json#tool_call_id=tool-provider-call-op-1"
        ]
    })]);

    assert_eq!(turns.len(), 1);
    assert_eq!(turns[0]["user_input"].as_str(), Some("PING-HISTORY"));
    assert_eq!(
        turns[0]["assistant_response"].as_str(),
        Some("PONG-HISTORY")
    );
    assert_eq!(
        turns[0]["tool_execution_records"]
            .as_array()
            .expect("tool records")
            .len(),
        0
    );
    assert_eq!(
        turns[0]["error_records"]
            .as_array()
            .expect("error records")
            .len(),
        0
    );
}

fn binding(runtime_home: &Path) -> DebugBinding {
    DebugBinding {
        project_id: "fin".into(),
        project_label: "fin".into(),
        runtime_home: runtime_home.display().to_string(),
        session_id: Some("session-live".into()),
        task_id: Some("task-live".into()),
        session_messages_path: None,
        recent_contexts_path: None,
        recent_digests_path: None,
    }
}

fn tool_record(
    tool_call_id: &str,
    operation_id: &str,
    status: &str,
    tool_name: &str,
    error_summary: serde_json::Value,
) -> serde_json::Value {
    json!({
        "tool_call_id": tool_call_id,
        "operation_id": operation_id,
        "trace_id":"trace-ws-tools",
        "session_id":"session-live",
        "task_id":"task-live",
        "topic_thread_id":null,
        "dispatch_id":null,
        "worker_id":"worker-system",
        "tool_name": tool_name,
        "tool_kind":"agent_tool",
        "title":"Tool Item",
        "purpose":"persist mobile render proof",
        "target_kind":"plan",
        "target_ref":"runtime/current/current_plan.json",
        "input_summary":"record progress",
        "output_summary":"plan updated",
        "status": status,
        "started_at":"2026-06-09T12:00:00+08:00",
        "ended_at":"2026-06-09T12:00:01+08:00",
        "duration_ms":1,
        "side_effects":["plan_write"],
        "artifact_refs":["runtime/current/current_plan.json"],
        "error_summary": error_summary
    })
}

fn write_mobile_tool_truth(runtime_home: &Path, operation_id: &str, records: serde_json::Value) {
    let current_dir = runtime_home.join("runtime/current");
    let tools_dir = runtime_home.join("sessions/2026/06/session-live/tools");
    fs::create_dir_all(&current_dir).expect("current dir");
    fs::create_dir_all(&tools_dir).expect("tools dir");
    fs::write(
        current_dir.join("last_run.json"),
        json!({
            "operation_id": operation_id,
            "session_recent_tool_records_path": "sessions/2026/06/session-live/tools/recent_tool_records.json"
        })
        .to_string(),
    )
    .expect("last_run");
    fs::write(
        tools_dir.join("recent_tool_records.json"),
        records.to_string(),
    )
    .expect("tool records");
}

fn temp_runtime_home(prefix: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "{}-{}",
        prefix,
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time should work")
            .as_nanos()
    ))
}
