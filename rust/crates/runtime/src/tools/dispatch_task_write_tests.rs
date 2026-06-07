use super::dispatch;
use super::dispatch::execute_model_tools;
use super::test_helpers::{context_with_runtime_home, temp_runtime_home};
use crate::model::parser::ModelToolCall;
use fin_contracts::{EntityRefs, MinimalContextView, ProjectContextBlock};
use serde_json::json;
use std::{
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

fn refs(worker_id: &str) -> EntityRefs {
    EntityRefs {
        session_id: Some("session-task-write".into()),
        task_id: Some("task-owner-loop".into()),
        worker_id: Some(worker_id.into()),
        ..EntityRefs::default()
    }
}

fn ensure_session_dir(runtime_home: &Path) -> PathBuf {
    let session_dir = runtime_home.join("sessions/2026/04/session-task-write");
    fs::create_dir_all(&session_dir).expect("session dir");
    session_dir
}

#[test]
fn project_task_write_tools_create_claim_submit_and_review() {
    let runtime_home = temp_runtime_home("task-write");
    ensure_session_dir(&runtime_home);
    let context = context_with_runtime_home(&runtime_home);

    let create = execute_model_tools(
        "op-task-create",
        "trace-task-create",
        &refs("worker-owner"),
        "2026-04-20T12:10:00+08:00",
        &context,
        1,
        &[ModelToolCall {
            tool_name: "project.task.create".into(),
            tool_call_id: None,
            arguments: json!({
                "task_id": "task-owner-loop",
                "title": "Owner loop follow-up",
                "summary": "dispatch managed work",
            }),
        }],
    );
    assert!(
        create
            .events
            .iter()
            .any(|(event_type, _)| event_type == "project.task.created")
    );

    let claim = execute_model_tools(
        "op-task-claim",
        "trace-task-claim",
        &refs("worker-exec"),
        "2026-04-20T12:10:01+08:00",
        &context,
        1,
        &[ModelToolCall {
            tool_name: "project.task.claim".into(),
            tool_call_id: None,
            arguments: json!({ "task_id": "task-owner-loop" }),
        }],
    );
    let claim_record = claim
        .tool_records
        .iter()
        .find(|item| item.tool_name == "project.task.claim")
        .expect("claim record");
    assert_eq!(claim_record.status, "completed");

    let submit = execute_model_tools(
        "op-task-submit",
        "trace-task-submit",
        &refs("worker-exec"),
        "2026-04-20T12:10:02+08:00",
        &context,
        1,
        &[ModelToolCall {
            tool_name: "project.task.submit".into(),
            tool_call_id: None,
            arguments: json!({
                "task_id": "task-owner-loop",
                "result_summary": "patched runtime and ran smoke",
                "artifact_refs": ["sessions/2026/04/session-task-write/tests/smoke.log"],
            }),
        }],
    );
    let submit_record = submit
        .tool_records
        .iter()
        .find(|item| item.tool_name == "project.task.submit")
        .expect("submit record");
    assert_eq!(submit_record.status, "completed");
    assert!(
        submit_record
            .output_summary
            .as_deref()
            .unwrap_or_default()
            .contains("submitted for review")
    );

    let review = execute_model_tools(
        "op-task-review",
        "trace-task-review",
        &refs("worker-owner"),
        "2026-04-20T12:10:03+08:00",
        &context,
        1,
        &[ModelToolCall {
            tool_name: "project.task.review".into(),
            tool_call_id: None,
            arguments: json!({
                "task_id": "task-owner-loop",
                "decision": "approve",
                "review_summary": "verified evidence and approved",
            }),
        }],
    );
    let review_record = review
        .tool_records
        .iter()
        .find(|item| item.tool_name == "project.task.review")
        .expect("review record");
    assert_eq!(review_record.status, "completed");

    let task_text = fs::read_to_string(
        runtime_home
            .join("sessions/2026/04/session-task-write/tasks/registry/task-owner-loop.json"),
    )
    .expect("task file");
    assert!(task_text.contains("\"status\": \"done\""));
    assert!(task_text.contains("\"latest_review_decision\": \"approve\""));
}

#[test]
fn project_task_review_rejects_non_owner_and_submit_rejects_wrong_claimer() {
    let runtime_home = temp_runtime_home("task-write-guards");
    ensure_session_dir(&runtime_home);
    let context = context_with_runtime_home(&runtime_home);

    let _ = execute_model_tools(
        "op-task-create-guard",
        "trace-task-create-guard",
        &refs("worker-owner"),
        "2026-04-20T12:11:00+08:00",
        &context,
        1,
        &[ModelToolCall {
            tool_name: "project.task.create".into(),
            tool_call_id: None,
            arguments: json!({
                "task_id": "task-owner-loop",
                "title": "Guarded task",
            }),
        }],
    );

    let _ = execute_model_tools(
        "op-task-claim-guard",
        "trace-task-claim-guard",
        &refs("worker-exec"),
        "2026-04-20T12:11:01+08:00",
        &context,
        1,
        &[ModelToolCall {
            tool_name: "project.task.claim".into(),
            tool_call_id: None,
            arguments: json!({ "task_id": "task-owner-loop" }),
        }],
    );

    let bad_submit = execute_model_tools(
        "op-task-submit-guard",
        "trace-task-submit-guard",
        &refs("worker-other"),
        "2026-04-20T12:11:02+08:00",
        &context,
        1,
        &[ModelToolCall {
            tool_name: "project.task.submit".into(),
            tool_call_id: None,
            arguments: json!({
                "task_id": "task-owner-loop",
                "result_summary": "intruding submit",
            }),
        }],
    );
    assert!(
        bad_submit
            .tool_records
            .iter()
            .any(|item| item.tool_name == "project.task.submit" && item.status == "failed")
    );

    let bad_review = execute_model_tools(
        "op-task-review-guard",
        "trace-task-review-guard",
        &refs("worker-exec"),
        "2026-04-20T12:11:03+08:00",
        &context,
        1,
        &[ModelToolCall {
            tool_name: "project.task.review".into(),
            tool_call_id: None,
            arguments: json!({
                "task_id": "task-owner-loop",
                "decision": "approve",
            }),
        }],
    );
    assert!(
        bad_review
            .tool_records
            .iter()
            .any(|item| item.tool_name == "project.task.review" && item.status == "failed")
    );
}

#[test]
fn system_owner_can_assign_worker_and_close_managed_task_loop() {
    let runtime_home = temp_runtime_home("task-collab-loop");
    ensure_session_dir(&runtime_home);
    let context = context_with_runtime_home(&runtime_home);

    let create = execute_model_tools(
        "op-collab-create",
        "trace-collab-create",
        &refs("worker-system"),
        "2026-04-20T12:20:00+08:00",
        &context,
        1,
        &[ModelToolCall {
            tool_name: "project.task.create".into(),
            tool_call_id: None,
            arguments: json!({
                "task_id": "task-collab-loop",
                "title": "Close startup control summary loop",
                "summary": "have one worker implement and return evidence",
                "review_owner_worker_id": "worker-system",
            }),
        }],
    );
    assert!(
        create
            .events
            .iter()
            .any(|(event_type, _)| event_type == "project.task.created")
    );

    let assign = execute_model_tools(
        "op-collab-assign",
        "trace-collab-assign",
        &refs("worker-system"),
        "2026-04-20T12:20:01+08:00",
        &context,
        1,
        &[ModelToolCall {
            tool_name: "agent.assign".into(),
            tool_call_id: None,
            arguments: json!({
                "target_worker_id": "worker-builder",
                "task_summary": "implement startup summary persistence and report receipts",
            }),
        }],
    );
    assert!(
        assign
            .events
            .iter()
            .any(|(event_type, _)| event_type == "agent.assignment_requested")
    );
    let pending_assignments =
        fs::read_to_string(runtime_home.join("runtime/assignments/pending.json"))
            .expect("assignment queue");
    assert!(pending_assignments.contains("\"target_worker_id\": \"worker-builder\""));
    assert!(pending_assignments.contains("\"owner_worker_id\": \"worker-system\""));

    let claim = execute_model_tools(
        "op-collab-claim",
        "trace-collab-claim",
        &refs("worker-builder"),
        "2026-04-20T12:20:02+08:00",
        &context,
        1,
        &[ModelToolCall {
            tool_name: "project.task.claim".into(),
            tool_call_id: None,
            arguments: json!({ "task_id": "task-collab-loop" }),
        }],
    );
    assert!(
        claim
            .tool_records
            .iter()
            .any(|item| item.tool_name == "project.task.claim" && item.status == "completed")
    );

    let submit = execute_model_tools(
        "op-collab-submit",
        "trace-collab-submit",
        &refs("worker-builder"),
        "2026-04-20T12:20:03+08:00",
        &context,
        1,
        &[ModelToolCall {
            tool_name: "project.task.submit".into(),
            tool_call_id: None,
            arguments: json!({
                "task_id": "task-collab-loop",
                "result_summary": "persisted startup summary and verified receipts",
                "artifact_refs": ["runtime/current/current_startup_control_summary.json"],
            }),
        }],
    );
    assert!(
        submit
            .tool_records
            .iter()
            .any(|item| item.tool_name == "project.task.submit" && item.status == "completed")
    );

    let review = execute_model_tools(
        "op-collab-review",
        "trace-collab-review",
        &refs("worker-system"),
        "2026-04-20T12:20:04+08:00",
        &context,
        1,
        &[ModelToolCall {
            tool_name: "project.task.review".into(),
            tool_call_id: None,
            arguments: json!({
                "task_id": "task-collab-loop",
                "decision": "approve",
                "review_summary": "system owner verified worker evidence and approved delivery",
            }),
        }],
    );
    assert!(
        review
            .events
            .iter()
            .any(|(event_type, _)| event_type == "project.task.review_completed")
    );

    let task_text = fs::read_to_string(
        runtime_home
            .join("sessions/2026/04/session-task-write/tasks/registry/task-collab-loop.json"),
    )
    .expect("task file");
    assert!(task_text.contains("\"status\": \"done\""));
    assert!(task_text.contains("\"claimed_by_worker_id\": \"worker-builder\""));
    assert!(task_text.contains("\"submitted_by_worker_id\": \"worker-builder\""));
    assert!(task_text.contains("\"reviewed_by_worker_id\": \"worker-system\""));
    assert!(task_text.contains("\"latest_review_decision\": \"approve\""));
}
