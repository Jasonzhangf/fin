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
fn contract_retry_preserves_non_empty_assistant_when_last_attempt_is_empty_native_tool() {
    // Reproduces the real QQ failure with op-system-entry-0123 round 6:
    // attempts 1-3 have non-empty assistant + reasoning.stop but invalid
    // control_feedback (missing fields), while attempt 4 is a native
    // tool_use with empty output_text. Before the fix, the last empty
    // attempt overwrote the user-visible answer, causing session_materializer
    // to skip the assistant message and QQ to receive no reply.
    let mut runtime = M1Runtime::default();
    let worker = worker_runtime();
    let provider = EmptyAssistantLastAttemptRetryProvider::new();
    let operation = InferenceOperationBuilder
        .build(
            &worker,
            InferenceRequest {
                operation_id: "op-empty-assistant-last-attempt".into(),
                trace_id: "trace-empty-assistant-last-attempt".into(),
                submitted_at: "2026-05-11T18:30:00+08:00".into(),
                refs: EntityRefs {
                    session_id: Some("session-empty-assistant-last-attempt".into()),
                    task_id: Some("task-empty-assistant-last-attempt".into()),
                    ..EntityRefs::default()
                },
                input: "这个任务已经完成了吗？".into(),
                context: MinimalContextView::default(),
            },
        )
        .expect("operation");

    let run = runtime
        .run_closure(operation, &provider)
        .expect("closure should run");

    // The key assertion: assistant_response_text must be non-empty,
    // carrying forward the last valid user-visible answer from attempts 1-3.
    assert!(
        !run.assistant_response_text.trim().is_empty(),
        "assistant_response_text must not be empty; it should carry forward the last non-empty user-visible answer"
    );
    // In the real QQ failure (op-system-entry-0123), the closure was finalized
    // with empty text because round 6's final contract-retry attempt had empty
    // output_text and there was no subsequent followup. After the fix, if a
    // followup round runs (like in this test), it produces a new answer which
    // supersedes the retry answer — but the critical thing is that we never end
    // up with empty assistant text. The fix ensures the contract-retry
    // final_round preserves a non-empty answer that the followup can build on.
    assert!(
        run.assistant_response_text.contains("任务已审查通过") || run.assistant_response_text.contains("推理链可观测性任务"),
        "assistant_response_text should contain a meaningful answer, got: {}",
        run.assistant_response_text
    );

    // Control_feedback should carry the non-empty origin from earlier attempts
    assert_eq!(
        run.control_feedback.origin, "model_output_contract_v1",
        "control_feedback origin should be from the model, not runtime_observation_only"
    );

    // The turn record must have non-empty visible output
    assert!(
        !run.turn_record.assistant_visible_output.as_ref().map_or(true, |v| v.trim().is_empty()),
        "turn_record.assistant_visible_output must be non-empty"
    );

    // Round 1 has 4 contract-retry attempts, round 2 has 1 followup = 5 total
    assert_eq!(run.provider_request_records.len(), 5);
    assert_eq!(run.provider_response_records.len(), 5);

    // Contract-retry exited cleanly (attempt 4 passed validation because
    // empty assistant + non-stop tool_calls is not a validation error).
    // The key: retry succeeded after 3 retries.
    assert!(
        run.events
            .iter()
            .any(|event| event.event_type == "model.output_contract_retry_requested"),
        "must emit at least one contract_retry_requested event"
    );

    // Tool dispatch truth stays on the last attempt (attempt 4's native tool call)
    assert!(
        run.tool_records
            .iter()
            .any(|record| record.tool_name == "project.task.status"),
        "tool_records should include the native tool call from attempt 4"
    );

    // Verify session_materializer will persist the assistant message
    let runtime_home = temp_runtime_home("empty-assistant-regression");
    fs::create_dir_all(runtime_home.join("runtime/current"))
        .expect("runtime current dir should exist");
    let receipt = SessionMaterializer
        .persist(
            &runtime_home,
            &run,
            &fin_config::RuntimeRetentionConfig::default(),
        )
        .expect("materialization should succeed");

    let messages_path = runtime_home.join(&receipt.session_messages_path);
    let messages: Vec<SessionMessageRecord> = serde_json::from_str(
        &fs::read_to_string(&messages_path)
            .unwrap_or_else(|_| panic!("messages.json should exist at {}", messages_path.display())),
    )
    .expect("messages should decode");

    let assistant_messages: Vec<_> = messages
        .iter()
        .filter(|m| m.role == "assistant")
        .collect();
    assert!(
        !assistant_messages.is_empty(),
        "session_materializer must persist an assistant message when assistant_response_text is non-empty"
    );
    assert!(
        !assistant_messages[0].content.trim().is_empty(),
        "assistant message content must be non-empty"
    );

    // last_run.json must carry the non-empty answer
    let last_run_path = runtime_home.join("runtime/current/last_run.json");
    let last_run: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(&last_run_path)
            .expect("last_run.json should exist"),
    )
    .expect("last_run should decode");
    let answer = last_run["answer"].as_str().unwrap_or("");
    assert!(!answer.is_empty(), "last_run answer must be non-empty");

    fs::remove_dir_all(&runtime_home).expect("temp runtime home should be cleaned");
}

