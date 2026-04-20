use crate::{model_output::ModelToolCall, tool_dispatch::execute_model_tools};
use fin_contracts::{EntityRefs, MinimalContextView, ProjectContextBlock};
use serde_json::json;
use std::{
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

fn temp_runtime_home(prefix: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "fin-runtime-{prefix}-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos()
    ))
}

fn context_with_runtime_home(runtime_home: &Path) -> MinimalContextView {
    MinimalContextView {
        project: Some(ProjectContextBlock {
            runtime_home: Some(runtime_home.display().to_string()),
            ..ProjectContextBlock::default()
        }),
        ..MinimalContextView::default()
    }
}

fn context_with_runtime_home_and_cwd(runtime_home: &Path, cwd: &Path) -> MinimalContextView {
    MinimalContextView {
        project: Some(ProjectContextBlock {
            runtime_home: Some(runtime_home.display().to_string()),
            cwd: Some(cwd.display().to_string()),
            project_root: Some(cwd.display().to_string()),
            ..ProjectContextBlock::default()
        }),
        ..MinimalContextView::default()
    }
}

fn refs() -> EntityRefs {
    EntityRefs {
        session_id: Some("session-tool-dispatch".into()),
        task_id: Some("task-tool-dispatch".into()),
        worker_id: Some("worker-tool-dispatch".into()),
        ..EntityRefs::default()
    }
}

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
