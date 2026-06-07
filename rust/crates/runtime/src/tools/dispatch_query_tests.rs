use super::dispatch;
use super::dispatch::execute_model_tools;
use super::test_helpers::{
    context_with_runtime_home, context_with_runtime_home_and_cwd, refs, temp_runtime_home,
};
use crate::model::parser::ModelToolCall;
use serde_json::json;
use std::{
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

#[test]
fn view_image_uses_current_attachment_metadata() {
    let runtime_home = temp_runtime_home("view-image");
    fs::create_dir_all(&runtime_home).expect("runtime_home");
    let workspace = runtime_home.join("workspace");
    fs::create_dir_all(&workspace).expect("workspace");
    let image_path = workspace.join("shot.png");
    fs::write(&image_path, b"fakepng").expect("image seed");
    let mut context = context_with_runtime_home_and_cwd(&runtime_home, &workspace);
    context.current_input = Some(fin_contracts::CurrentInputBlock {
        input: "inspect image".into(),
        source: "test".into(),
        operation_id: "op-image".into(),
        trace_id: "trace-image".into(),
        attachments: vec![fin_contracts::InputAttachmentSummary {
            kind: "image/png".into(),
            content_type: Some("image/png".into()),
            name: Some("shot.png".into()),
            local_path: Some(image_path.display().to_string()),
            size_bytes: Some(7),
            width: Some(64),
            height: Some(32),
            ..Default::default()
        }],
    });

    let result = execute_model_tools(
        "op-view-image",
        "trace-view-image",
        &refs(),
        "2026-04-20T12:00:02+08:00",
        &context,
        1,
        &[ModelToolCall {
            tool_name: "view_image".into(),
            tool_call_id: None,
            arguments: json!({}),
        }],
    );
    let record = result
        .tool_records
        .iter()
        .find(|item| item.tool_name == "view_image")
        .expect("view_image record");
    assert_eq!(record.status, "completed");
    assert!(
        record
            .output_summary
            .as_deref()
            .unwrap_or_default()
            .contains("64x32")
    );
}

#[test]
fn context_history_rebuild_refreshes_rebuild_index() {
    let runtime_home = temp_runtime_home("context-rebuild");
    fs::create_dir_all(runtime_home.join("runtime/current")).expect("runtime current");
    let session_dir = runtime_home.join("sessions/2026/04/session-tool-dispatch");
    fs::create_dir_all(session_dir.join("context")).expect("session context dir");
    fs::create_dir_all(session_dir.join("digests")).expect("digests");
    fs::create_dir_all(session_dir.join("reasoning")).expect("reasoning");
    fs::create_dir_all(session_dir.join("tools")).expect("tools");
    fs::write(
        runtime_home.join("runtime/current/current_context.json"),
        serde_json::to_vec_pretty(&json!({
            "operation_id": "op-current-context",
            "trace_id": "trace-current-context",
            "session_id": "session-tool-dispatch",
            "task_id": "task-tool-dispatch"
        }))
        .expect("json"),
    )
    .expect("current_context");
    fs::write(
        session_dir.join("digests/recent.json"),
        b"[{\"digest_id\":\"d1\"}]",
    )
    .expect("digests");
    fs::write(
        session_dir.join("reasoning/recent.json"),
        b"[{\"reasoning_id\":\"r1\"}]",
    )
    .expect("reasoning");
    fs::write(
        session_dir.join("tools/recent.json"),
        b"[{\"tool_call_id\":\"t1\"}]",
    )
    .expect("tools");
    let context = context_with_runtime_home(&runtime_home);

    let result = execute_model_tools(
        "op-context-rebuild",
        "trace-context-rebuild",
        &refs(),
        "2026-04-20T12:00:03+08:00",
        &context,
        1,
        &[ModelToolCall {
            tool_name: "context_history.rebuild".into(),
            tool_call_id: None,
            arguments: json!({ "reason": "test_rebuild" }),
        }],
    );
    let record = result
        .tool_records
        .iter()
        .find(|item| item.tool_name == "context_history.rebuild")
        .expect("context_history.rebuild record");
    assert_eq!(record.status, "completed");
    let rebuild_index: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(runtime_home.join("runtime/current/current_rebuild_index.json"))
            .expect("rebuild index"),
    )
    .expect("rebuild index json");
    assert_eq!(rebuild_index["reason"].as_str(), Some("test_rebuild"));
}

#[test]
fn project_task_tools_list_and_status_known_tasks() {
    let runtime_home = temp_runtime_home("project-task-tools");
    let session_dir = runtime_home.join("sessions/2026/04/session-tool-dispatch");
    fs::create_dir_all(session_dir.join("tasks/routing")).expect("routing");
    fs::create_dir_all(session_dir.join("control")).expect("control");
    fs::create_dir_all(session_dir.join("tasks/plan")).expect("plan");
    fs::create_dir_all(session_dir.join("conversation")).expect("conversation");
    fs::write(
        session_dir.join("tasks/routing/latest_action.json"),
        br#"{"task_id":"task-tool-dispatch","action_kind":"continue_current_task"}"#,
    )
    .expect("routing latest");
    fs::write(
        session_dir.join("control/execution_state.json"),
        br#"{"state_id":"state-1","task_id":"task-tool-dispatch","status":"running","pending_input_count":2,"accepts_user_input":true,"updated_at":"2026-04-20T12:00:04+08:00"}"#,
    )
    .expect("execution state");
    fs::write(
        session_dir.join("tasks/plan/latest.json"),
        br#"{"steps":[{"step":"inspect","status":"completed"},{"step":"patch","status":"in_progress"}]}"#,
    )
    .expect("plan latest");
    fs::write(
        session_dir.join("conversation/messages.json"),
        br#"[{"message_id":"user-1","task_id":"task-tool-dispatch"}]"#,
    )
    .expect("messages");
    let context = context_with_runtime_home(&runtime_home);

    let listed = execute_model_tools(
        "op-project-task-list",
        "trace-project-task-list",
        &refs(),
        "2026-04-20T12:00:04+08:00",
        &context,
        1,
        &[ModelToolCall {
            tool_name: "project.task.list".into(),
            tool_call_id: None,
            arguments: json!({ "limit": 5 }),
        }],
    );
    assert!(
        listed
            .tool_records
            .iter()
            .any(|item| item.tool_name == "project.task.list" && item.status == "completed")
    );

    let status = execute_model_tools(
        "op-project-task-status",
        "trace-project-task-status",
        &refs(),
        "2026-04-20T12:00:05+08:00",
        &context,
        1,
        &[ModelToolCall {
            tool_name: "project.task.status".into(),
            tool_call_id: None,
            arguments: json!({ "task_id": "task-tool-dispatch" }),
        }],
    );
    let record = status
        .tool_records
        .iter()
        .find(|item| item.tool_name == "project.task.status")
        .expect("project.task.status record");
    assert_eq!(record.status, "completed");
    assert!(
        record
            .output_summary
            .as_deref()
            .unwrap_or_default()
            .contains("status=running")
    );
}

#[test]
fn control_query_tools_list_presence_and_supervision_truth() {
    let runtime_home = temp_runtime_home("control-query-tools");
    fs::create_dir_all(runtime_home.join("runtime/current")).expect("runtime current");
    fs::write(
        runtime_home.join("runtime/current/current_agent_presence_registry.json"),
        br#"{
  "agents":[
    {"agent_id":"mbp.system","device_name":"mbp","agent_name":"system","status":"busy"},
    {"agent_id":"mbp.fin","device_name":"mbp","agent_name":"fin","status":"idle"},
    {"agent_id":"mbp.infra","device_name":"mbp","agent_name":"infra","status":"waiting"}
  ]
}"#,
    )
    .expect("presence registry");
    fs::write(
        runtime_home.join("runtime/current/current_project_supervision.json"),
        br#"{
  "ready_count":0,
  "resume_ready_count":1,
  "busy_count":1,
  "waiting_count":1,
  "recover_needed_count":1,
  "projects":[
    {"project_id":"fin","desired_action":"monitor_running_task"},
    {"project_id":"infra","desired_action":"resume_project_task"},
    {"project_id":"archive","desired_action":"recover_project_agent"}
  ]
}"#,
    )
    .expect("project supervision");
    let context = context_with_runtime_home(&runtime_home);

    let presence = execute_model_tools(
        "op-agent-presence",
        "trace-agent-presence",
        &refs(),
        "2026-04-20T12:00:06+08:00",
        &context,
        1,
        &[ModelToolCall {
            tool_name: "agent.presence.list".into(),
            tool_call_id: None,
            arguments: json!({ "limit": 5 }),
        }],
    );
    let presence_record = presence
        .tool_records
        .iter()
        .find(|item| item.tool_name == "agent.presence.list")
        .expect("agent.presence.list record");
    assert_eq!(presence_record.status, "completed");
    assert!(
        presence_record
            .output_summary
            .as_deref()
            .unwrap_or_default()
            .contains("agents=3")
    );
    assert!(
        presence
            .events
            .iter()
            .any(|(event_type, _)| event_type == "agent.presence_list_completed")
    );

    let supervision = execute_model_tools(
        "op-project-supervision",
        "trace-project-supervision",
        &refs(),
        "2026-04-20T12:00:07+08:00",
        &context,
        1,
        &[ModelToolCall {
            tool_name: "project.supervision.list".into(),
            tool_call_id: None,
            arguments: json!({ "desired_action": "resume_project_task" }),
        }],
    );
    let supervision_record = supervision
        .tool_records
        .iter()
        .find(|item| item.tool_name == "project.supervision.list")
        .expect("project.supervision.list record");
    assert_eq!(supervision_record.status, "completed");
    assert!(
        supervision_record
            .output_summary
            .as_deref()
            .unwrap_or_default()
            .contains("resume_ready=1")
    );
    assert!(
        supervision_record
            .output_summary
            .as_deref()
            .unwrap_or_default()
            .contains("infra:resume_project_task")
    );
    assert!(
        supervision
            .events
            .iter()
            .any(|(event_type, _)| event_type == "project.supervision_list_completed")
    );
}
