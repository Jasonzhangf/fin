use super::*;

#[test]
fn runtime_closure_uses_structured_user_response_for_session_visible_output() {
    let mut runtime = M1Runtime::default();
    let worker = worker_runtime();
    let operation = InferenceOperationBuilder
        .build(
            &worker,
            InferenceRequest {
                operation_id: "op-structured".into(),
                trace_id: "trace-structured".into(),
                submitted_at: "2026-04-18T10:00:00+08:00".into(),
                refs: EntityRefs {
                    task_id: Some("task-structured".into()),
                    ..EntityRefs::default()
                },
                input: "continue".into(),
                context: MinimalContextView::default(),
            },
        )
        .expect("operation");

    let run = runtime
        .run_closure(operation, &StructuredProvider::new())
        .expect("closure");
    assert_eq!(run.assistant_response_text, "structured answer");
    assert_eq!(
        run.note.summary,
        "provider openai returned: structured answer"
    );
    assert_eq!(run.digest.continuity_tail[1], "structured answer");
    assert!(
        run.events
            .iter()
            .any(|event| event.event_type == "model.output_parsed")
    );
}

#[test]
fn runtime_closure_records_wait_reminder_tool_and_event() {
    let mut runtime = M1Runtime::default();
    let worker = worker_runtime();
    let operation = InferenceOperationBuilder
        .build(
            &worker,
            InferenceRequest {
                operation_id: "op-wait-tool".into(),
                trace_id: "trace-wait-tool".into(),
                submitted_at: "2026-04-18T10:10:00+08:00".into(),
                refs: EntityRefs {
                    session_id: Some("session-wait".into()),
                    task_id: Some("task-wait".into()),
                    ..EntityRefs::default()
                },
                input: "等两分钟再看日志".into(),
                context: MinimalContextView::default(),
            },
        )
        .expect("operation");

    let run = runtime
        .run_closure(operation, &WaitToolProvider::new())
        .expect("closure");
    assert_eq!(run.assistant_response_text, "先等待日志完成。");
    assert!(
        run.tool_records
            .iter()
            .any(|record| record.tool_name == "wait.remind" && record.status == "completed")
    );
    assert!(
        run.events
            .iter()
            .any(|event| event.event_type == "system.reminder_scheduled")
    );
    assert_eq!(run.provider_request_records.len(), 1);
    assert_eq!(run.provider_response_records.len(), 1);
    let operation_completed = run
        .events
        .iter()
        .find(|event| event.event_type == "operation.completed")
        .expect("operation.completed");
    assert_eq!(
        operation_completed
            .payload
            .get("status")
            .and_then(serde_json::Value::as_str),
        Some("waiting_external")
    );
    assert_eq!(
        operation_completed
            .payload
            .get("stop_source")
            .and_then(serde_json::Value::as_str),
        Some("wait.remind")
    );
    assert!(
        !run.events
            .iter()
            .any(|event| event.event_type == "reasoning.auto_tool_roundtrip_completed")
    );
}

#[test]
fn runtime_closure_uses_reasoning_stop_as_stop_signal() {
    let mut runtime = M1Runtime::default();
    let worker = worker_runtime();
    let operation = InferenceOperationBuilder
        .build(
            &worker,
            InferenceRequest {
                operation_id: "op-stop-tool".into(),
                trace_id: "trace-stop-tool".into(),
                submitted_at: "2026-04-18T10:20:00+08:00".into(),
                refs: EntityRefs {
                    session_id: Some("session-stop".into()),
                    task_id: Some("task-stop".into()),
                    ..EntityRefs::default()
                },
                input: "收口当前任务".into(),
                context: MinimalContextView::default(),
            },
        )
        .expect("operation");

    let run = runtime
        .run_closure(operation, &ReasoningStopProvider::new())
        .expect("closure");
    assert!(
        run.tool_records
            .iter()
            .any(|record| record.tool_name == "reasoning.stop" && record.status == "completed")
    );
    assert!(
        run.events
            .iter()
            .any(|event| event.event_type == "reasoning.stopped")
    );
    let operation_completed = run
        .events
        .iter()
        .find(|event| event.event_type == "operation.completed")
        .expect("operation.completed");
    assert_eq!(
        operation_completed
            .payload
            .get("status")
            .and_then(serde_json::Value::as_str),
        Some("completed_with_evidence")
    );
    assert_eq!(
        operation_completed
            .payload
            .get("stop_source")
            .and_then(serde_json::Value::as_str),
        Some("completed_with_evidence")
    );
    assert_eq!(run.turn_record.status, "completed_with_evidence");
    assert!(
        !run.events
            .iter()
            .any(|event| event.event_type == "control.exit_gate_rejected")
    );
}

#[test]
fn runtime_closure_completes_with_evidence_via_control_feedback() {
    let mut runtime = M1Runtime::default();
    let worker = worker_runtime();
    let operation = InferenceOperationBuilder
        .build(
            &worker,
            InferenceRequest {
                operation_id: "op-completed-with-evidence".into(),
                trace_id: "trace-completed-with-evidence".into(),
                submitted_at: "2026-04-22T10:30:00+08:00".into(),
                refs: EntityRefs {
                    session_id: Some("session-complete".into()),
                    task_id: Some("task-complete".into()),
                    ..EntityRefs::default()
                },
                input: "给我最终完成结论".into(),
                context: MinimalContextView::default(),
            },
        )
        .expect("operation");

    let run = runtime
        .run_closure(operation, &CompletedWithEvidenceProvider::new())
        .expect("closure");
    let operation_completed = run
        .events
        .iter()
        .find(|event| event.event_type == "operation.completed")
        .expect("operation.completed");
    assert_eq!(
        operation_completed
            .payload
            .get("status")
            .and_then(serde_json::Value::as_str),
        Some("completed_with_evidence")
    );
    assert_eq!(
        operation_completed
            .payload
            .get("stop_source")
            .and_then(serde_json::Value::as_str),
        Some("completed_with_evidence")
    );
    assert_eq!(run.turn_record.status, "completed_with_evidence");
    assert_eq!(run.progress.next_step.as_deref(), Some("render_projection"));
}

#[test]
fn runtime_closure_accepts_simple_chat_exit_channel() {
    let mut runtime = M1Runtime::default();
    let worker = worker_runtime();
    let operation = InferenceOperationBuilder
        .build(
            &worker,
            InferenceRequest {
                operation_id: "op-simple-chat".into(),
                trace_id: "trace-simple-chat".into(),
                submitted_at: "2026-04-22T10:40:00+08:00".into(),
                refs: EntityRefs {
                    session_id: Some("session-simple-chat".into()),
                    ..EntityRefs::default()
                },
                input: "你好".into(),
                context: MinimalContextView::default(),
            },
        )
        .expect("operation");

    let run = runtime
        .run_closure(operation, &SimpleChatProvider::new())
        .expect("closure");
    let operation_completed = run
        .events
        .iter()
        .find(|event| event.event_type == "operation.completed")
        .expect("operation.completed");
    assert_eq!(
        operation_completed
            .payload
            .get("status")
            .and_then(serde_json::Value::as_str),
        Some("simple_chat_done")
    );
    assert_eq!(run.turn_record.status, "simple_chat_done");
}

#[test]
fn runtime_closure_accepts_blocked_user_action_exit_channel() {
    let mut runtime = M1Runtime::default();
    let worker = worker_runtime();
    let operation = InferenceOperationBuilder
        .build(
            &worker,
            InferenceRequest {
                operation_id: "op-blocked-user-action".into(),
                trace_id: "trace-blocked-user-action".into(),
                submitted_at: "2026-04-22T10:50:00+08:00".into(),
                refs: EntityRefs {
                    session_id: Some("session-blocked".into()),
                    task_id: Some("task-blocked".into()),
                    ..EntityRefs::default()
                },
                input: "继续部署".into(),
                context: MinimalContextView::default(),
            },
        )
        .expect("operation");

    let run = runtime
        .run_closure(operation, &BlockedUserActionProvider::new())
        .expect("closure");
    let operation_completed = run
        .events
        .iter()
        .find(|event| event.event_type == "operation.completed")
        .expect("operation.completed");
    assert_eq!(
        operation_completed
            .payload
            .get("status")
            .and_then(serde_json::Value::as_str),
        Some("blocked_requires_user_action")
    );
    assert_eq!(run.turn_record.status, "blocked_requires_user_action");
}
