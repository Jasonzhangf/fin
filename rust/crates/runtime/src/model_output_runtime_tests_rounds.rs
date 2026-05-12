use super::*;

#[test]
fn runtime_closure_records_round_level_events_for_multi_round_tool_loop() {
    let mut runtime = M1Runtime::default();
    let worker = worker_runtime();
    let operation = InferenceOperationBuilder
        .build(
            &worker,
            InferenceRequest {
                operation_id: "op-two-round".into(),
                trace_id: "trace-two-round".into(),
                submitted_at: "2026-04-19T12:00:00+08:00".into(),
                refs: EntityRefs {
                    session_id: Some("session-two-round".into()),
                    task_id: Some("task-two-round".into()),
                    ..EntityRefs::default()
                },
                input: "先检查 peer 再结束".into(),
                context: MinimalContextView::default(),
            },
        )
        .expect("operation");

    let run = runtime
        .run_closure(operation, &TwoRoundProvider::new())
        .expect("closure");
    assert_eq!(run.provider_request_records.len(), 2);
    assert_eq!(run.provider_response_records.len(), 2);
    assert_eq!(
        run.events
            .iter()
            .filter(|event| event.event_type == "provider.round_completed")
            .count(),
        2
    );
    assert_eq!(
        run.events
            .iter()
            .filter(|event| event.event_type == "model.output_round_parsed")
            .count(),
        2
    );
    assert_eq!(
        run.events
            .iter()
            .filter(|event| event.event_type == "control.feedback_round_recorded")
            .count(),
        2
    );
    assert_eq!(
        run.events
            .iter()
            .filter(|event| event.event_type == "tool.dispatch_round_completed")
            .count(),
        2
    );
    assert!(
        run.step_records
            .iter()
            .filter(|step| matches!(
                step.step_kind.as_str(),
                "provider_request" | "model_parse" | "control_feedback" | "tool_dispatch"
            ))
            .all(|step| !step.event_ids.is_empty())
    );
    assert!(
        run.step_records
            .windows(2)
            .all(|pair| pair[0].step_index < pair[1].step_index)
    );
    let model_tool_ids = run
        .tool_records
        .iter()
        .filter(|record| record.tool_name != "provider.call")
        .map(|record| record.tool_call_id.as_str())
        .collect::<Vec<_>>();
    let unique_model_tool_ids = model_tool_ids
        .iter()
        .copied()
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(unique_model_tool_ids.len(), model_tool_ids.len());
    assert!(model_tool_ids.iter().any(|id| id.contains("-r01-00")));
    assert!(model_tool_ids.iter().any(|id| id.contains("-r02-00")));
}
