use super::*;

#[test]
fn runtime_retries_when_completion_feedback_fields_are_missing() {
    let mut runtime = M1Runtime::default();
    let worker = worker_runtime();
    let provider = MissingCompletionFeedbackRetryProvider::new();
    let operation = InferenceOperationBuilder
        .build(
            &worker,
            InferenceRequest {
                operation_id: "op-missing-completion-feedback".into(),
                trace_id: "trace-missing-completion-feedback".into(),
                submitted_at: "2026-04-22T13:10:00+08:00".into(),
                refs: EntityRefs {
                    session_id: Some("session-missing-completion-feedback".into()),
                    task_id: Some("task-missing-completion-feedback".into()),
                    ..EntityRefs::default()
                },
                input: "finish the task".into(),
                context: MinimalContextView::default(),
            },
        )
        .expect("operation");

    let run = runtime
        .run_closure(operation, &provider)
        .expect("closure should run");
    let requests = provider.captured_requests();
    assert_eq!(requests.len(), 2);
    assert!(requests[1].input.contains("completion_evidence is empty"));
    assert!(requests[1].input.contains("final_conclusions is empty"));
    assert!(requests[1].input.contains(
        "reasoning.stop was requested, but the control feedback still lacks a valid closure channel"
    ));
    assert_eq!(run.turn_record.status, "completed_with_evidence");
}

#[test]
fn runtime_stops_after_contract_retry_limit() {
    let mut runtime = M1Runtime::default();
    let worker = worker_runtime();
    let provider = ContractRetryLimitProvider::new();
    let operation = InferenceOperationBuilder
        .build(
            &worker,
            InferenceRequest {
                operation_id: "op-contract-retry-limit".into(),
                trace_id: "trace-contract-retry-limit".into(),
                submitted_at: "2026-04-21T21:10:00+08:00".into(),
                refs: EntityRefs {
                    session_id: Some("session-contract-retry-limit".into()),
                    task_id: Some("task-contract-retry-limit".into()),
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
    assert_eq!(requests.len(), 4);
    assert!(
        run.events
            .iter()
            .any(|event| event.event_type == "model.output_contract_retry_limit_reached")
    );
    assert_eq!(run.provider_request_records.len(), 4);
    assert_eq!(run.provider_response_records.len(), 4);
    assert_eq!(
        run.provider_request_records
            .iter()
            .map(|record| (record.round_index, record.attempt_index))
            .collect::<Vec<_>>(),
        vec![(1, 1), (1, 2), (1, 3), (1, 4)]
    );
    assert_eq!(
        run.round_records[0].request_id,
        "provider-request-op-contract-retry-limit-r01-a04"
    );
    assert!(
        !run.tool_records
            .iter()
            .any(|record| record.tool_name == "reasoning.stop" && record.status == "completed")
    );
}

#[test]
fn session_materializer_persists_retry_attempt_provider_truth() {
    let mut runtime = M1Runtime::default();
    let worker = worker_runtime();
    let provider = ContractRetryProvider::new();
    let operation = InferenceOperationBuilder
        .build(
            &worker,
            InferenceRequest {
                operation_id: "op-contract-retry-materialize".into(),
                trace_id: "trace-contract-retry-materialize".into(),
                submitted_at: "2026-04-21T21:20:00+08:00".into(),
                refs: EntityRefs {
                    session_id: Some("session-contract-retry-materialize".into()),
                    task_id: Some("task-contract-retry-materialize".into()),
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
    let runtime_home = temp_runtime_home("retry-materialize");
    fs::create_dir_all(runtime_home.join("runtime/current"))
        .expect("runtime current dir should exist");
    let receipt = SessionMaterializer
        .persist(
            &runtime_home,
            &run,
            &fin_config::RuntimeRetentionConfig::default(),
        )
        .expect("materialization should succeed");

    let current_requests: Vec<ProviderRequestRecord> = serde_json::from_str(
        &fs::read_to_string(runtime_home.join("runtime/current/current_provider_requests.json"))
            .expect("current requests should exist"),
    )
    .expect("current requests should decode");
    let current_responses: Vec<ProviderResponseRecord> = serde_json::from_str(
        &fs::read_to_string(runtime_home.join("runtime/current/current_provider_responses.json"))
            .expect("current responses should exist"),
    )
    .expect("current responses should decode");
    let session_requests: Vec<ProviderRequestRecord> = serde_json::from_str(
        &fs::read_to_string(
            receipt
                .session_dir
                .join("provider/recent_provider_requests.json"),
        )
        .expect("session requests should exist"),
    )
    .expect("session requests should decode");
    let session_responses: Vec<ProviderResponseRecord> = serde_json::from_str(
        &fs::read_to_string(
            receipt
                .session_dir
                .join("provider/recent_provider_responses.json"),
        )
        .expect("session responses should exist"),
    )
    .expect("session responses should decode");

    for records in [&current_requests, &session_requests] {
        assert_eq!(
            records
                .iter()
                .map(|record| (record.round_index, record.attempt_index))
                .collect::<Vec<_>>(),
            vec![(1, 1), (1, 2)]
        );
    }
    for records in [&current_responses, &session_responses] {
        assert_eq!(
            records
                .iter()
                .map(|record| (record.round_index, record.attempt_index))
                .collect::<Vec<_>>(),
            vec![(1, 1), (1, 2)]
        );
    }

    fs::remove_dir_all(&runtime_home).expect("temp runtime home should be cleaned");
}

#[test]
fn hidden_framework_turn_does_not_write_session_progress_control_or_notes() {
    let mut runtime = M1Runtime::default();
    let worker = worker_runtime();
    let provider = ContractRetryProvider::new();
    let mut operation = InferenceOperationBuilder
        .build(
            &worker,
            InferenceRequest {
                operation_id: "op-hidden-startup-self-check".into(),
                trace_id: "trace-hidden-startup-self-check".into(),
                submitted_at: "2026-04-26T13:40:00+08:00".into(),
                refs: EntityRefs {
                    session_id: Some("session-hidden-startup-self-check".into()),
                    task_id: Some("task-hidden-startup-self-check".into()),
                    ..EntityRefs::default()
                },
                input: "startup self-check".into(),
                context: MinimalContextView::default(),
            },
        )
        .expect("operation");
    operation.source = "framework.startup.self_check".into();
    let run = runtime
        .run_closure(operation, &provider)
        .expect("closure should run");

    let runtime_home = temp_runtime_home("hidden-no-session-history");
    fs::create_dir_all(runtime_home.join("runtime/current"))
        .expect("runtime current dir should exist");
    let receipt = SessionMaterializer
        .persist(
            &runtime_home,
            &run,
            &fin_config::RuntimeRetentionConfig::default(),
        )
        .expect("materialization should succeed");

    assert!(
        !receipt.session_dir.join("events/stream.jsonl").exists(),
        "hidden turn must not append session event stream"
    );
    assert!(
        !receipt.session_dir.join("progress/latest.json").exists(),
        "hidden turn must not write session progress latest"
    );
    assert!(
        !receipt.session_dir.join("control/latest.json").exists(),
        "hidden turn must not write session control latest"
    );
    assert!(
        !receipt.session_dir.join("notes/latest.json").exists(),
        "hidden turn must not write session notes latest"
    );
    assert!(
        runtime_home
            .join("runtime/current/current_control_feedback.json")
            .exists(),
        "hidden turn still needs runtime current control feedback truth"
    );

    fs::remove_dir_all(&runtime_home).expect("temp runtime home should be cleaned");
}
