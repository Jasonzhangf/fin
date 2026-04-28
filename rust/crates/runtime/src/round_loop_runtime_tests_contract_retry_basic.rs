use super::*;

#[test]
fn runtime_retries_invalid_output_contract_and_recovers() {
    let mut runtime = M1Runtime::default();
    let worker = worker_runtime();
    let provider = ContractRetryProvider::new();
    let operation = InferenceOperationBuilder
        .build(
            &worker,
            InferenceRequest {
                operation_id: "op-contract-retry".into(),
                trace_id: "trace-contract-retry".into(),
                submitted_at: "2026-04-21T21:00:00+08:00".into(),
                refs: EntityRefs {
                    session_id: Some("session-contract-retry".into()),
                    task_id: Some("task-contract-retry".into()),
                    ..EntityRefs::default()
                },
                input: "finish the turn".into(),
                context: MinimalContextView::default(),
            },
        )
        .expect("operation");

    let run = runtime
        .run_closure(operation, &provider)
        .expect("closure should run");
    let requests = provider.captured_requests();
    assert_eq!(requests.len(), 2);
    assert!(requests[1].input.contains(
        "The previous output is close, but a few required fin contract pieces are still missing"
    ));
    assert!(
        requests[1]
            .input
            .contains("detected <fin_tool_calls> but it is not executable")
    );
    assert!(
        run.events
            .iter()
            .any(|event| event.event_type == "model.output_contract_retry_requested")
    );
    assert!(
        run.events
            .iter()
            .any(|event| event.event_type == "model.output_contract_retry_succeeded")
    );
    assert_eq!(run.provider_request_records.len(), 2);
    assert_eq!(run.provider_response_records.len(), 2);
    assert_eq!(
        run.provider_request_records
            .iter()
            .map(|record| (
                record.round_index,
                record.attempt_index,
                record.request_id.as_str()
            ))
            .collect::<Vec<_>>(),
        vec![
            (1, 1, "provider-request-op-contract-retry-r01-a01"),
            (1, 2, "provider-request-op-contract-retry-r01-a02"),
        ]
    );
    assert_eq!(
        run.provider_response_records
            .iter()
            .map(|record| (
                record.round_index,
                record.attempt_index,
                record.response_record_id.as_str()
            ))
            .collect::<Vec<_>>(),
        vec![
            (1, 1, "provider-response-op-contract-retry-r01-a01"),
            (1, 2, "provider-response-op-contract-retry-r01-a02"),
        ]
    );
    assert_eq!(run.round_records.len(), 1);
    assert_eq!(
        run.round_records[0].request_id,
        "provider-request-op-contract-retry-r01-a02"
    );
    assert!(run.step_records.iter().any(|step| {
        step.step_kind == "model_parse"
            && step.summary.contains("round 1 attempt 1")
            && step.summary.contains("accepted=false")
    }));
    assert!(run.step_records.iter().any(|step| {
        step.step_kind == "model_parse"
            && step.summary.contains("round 1 attempt 2")
            && step.summary.contains("accepted=true")
    }));
    assert!(
        run.tool_records
            .iter()
            .any(|record| record.tool_name == "reasoning.stop" && record.status == "completed")
    );
}

#[test]
fn runtime_retries_when_control_feedback_block_is_missing() {
    let mut runtime = M1Runtime::default();
    let worker = worker_runtime();
    let provider = MissingControlFeedbackRetryProvider::new();
    let operation = InferenceOperationBuilder
        .build(
            &worker,
            InferenceRequest {
                operation_id: "op-missing-control-feedback-retry".into(),
                trace_id: "trace-missing-control-feedback-retry".into(),
                submitted_at: "2026-04-22T13:00:00+08:00".into(),
                refs: EntityRefs {
                    session_id: Some("session-missing-control-feedback-retry".into()),
                    ..EntityRefs::default()
                },
                input: "reply politely".into(),
                context: MinimalContextView::default(),
            },
        )
        .expect("operation");

    let run = runtime
        .run_closure(operation, &provider)
        .expect("closure should run");
    let requests = provider.captured_requests();
    assert_eq!(requests.len(), 2);
    assert!(
        requests[1]
            .input
            .contains("missing <fin_control_feedback> JSON block")
    );
    assert!(requests[1].input.contains("The previous output is close"));
    assert!(requests[1].input.contains("Be gentle, precise"));
    assert_eq!(run.control_feedback.origin, "model_output_contract_v1");
}

#[test]
fn runtime_allows_intermediate_tool_round_without_control_feedback() {
    let mut runtime = M1Runtime::default();
    let worker = worker_runtime();
    let provider = IntermediateToolRoundWithoutControlFeedbackProvider::new();
    let operation = InferenceOperationBuilder
        .build(
            &worker,
            InferenceRequest {
                operation_id: "op-intermediate-tool-no-control".into(),
                trace_id: "trace-intermediate-tool-no-control".into(),
                submitted_at: "2026-04-22T22:10:00+08:00".into(),
                refs: EntityRefs {
                    session_id: Some("session-intermediate-tool-no-control".into()),
                    ..EntityRefs::default()
                },
                input: "开始复杂 managed 任务".into(),
                context: MinimalContextView::default(),
            },
        )
        .expect("operation");

    let run = runtime
        .run_closure(operation, &provider)
        .expect("closure should run");
    let requests = provider.captured_requests();
    assert_eq!(requests.len(), 2);
    assert!(requests[1].input.contains("Continue the same turn."));
    assert!(!requests[1].input.contains(
        "The previous output is close, but a few required fin contract pieces are still missing."
    ));
    assert!(
        requests[1]
            .prior_tool_calls
            .iter()
            .any(|call| call.name == "project.task.list")
    );
    assert!(
        requests[1]
            .prior_tool_calls
            .iter()
            .any(|call| call.name == "agent.presence.list")
    );
    assert_eq!(run.turn_record.status, "completed_with_evidence");
}
