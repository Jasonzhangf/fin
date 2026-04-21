use super::*;
use fin_contracts::ControlFeedback;

#[test]
fn run_closure_emits_expected_event_chain() {
    let mut runtime = M1Runtime::default();
    let worker = worker_runtime();
    let op = InferenceOperationBuilder
        .build(
            &worker,
            InferenceRequest {
                operation_id: "op-1".into(),
                trace_id: "trace-1".into(),
                submitted_at: "2026-04-17T00:00:00Z".into(),
                refs: EntityRefs {
                    session_id: Some("session-1".into()),
                    task_id: Some("task-1".into()),
                    ..EntityRefs::default()
                },
                input: "hello".into(),
                context: MinimalContextView::default(),
            },
        )
        .expect("build operation");

    let run = runtime
        .run_closure(op, &provider())
        .expect("closure should run");
    let kinds: Vec<_> = run.events.iter().map(|e| e.event_type.as_str()).collect();
    assert_eq!(
        kinds,
        vec![
            "operation.accepted",
            "inference.started",
            "provider.operation_accepted",
            "provider.gateway_request_sent",
            "provider.gateway_response_received",
            "provider.response_normalized",
            "provider.completed",
            "provider.round_completed",
            "model.output_round_parsed",
            "control.feedback_round_recorded",
            "tool.dispatch_round_completed",
            "model.output_parsed",
            "model.output_contract_retry_requested",
            "model.output_contract_retry_requested",
            "model.output_contract_retry_requested",
            "model.output_contract_retry_limit_reached",
            "progress.updated",
            "tool.execution_recorded",
            "control.feedback_recorded",
            "execution_note.appended",
            "reasoning.view_recorded",
            "routing.decision_recorded",
            "routing.action_derived",
            "step.ledger_recorded",
            "digest.finalized",
            "turn.recorded",
            "closure.trace_recorded",
            "operation.completed",
        ]
    );
    assert_eq!(run.prepared_request.model, "gpt-5");
    assert_eq!(run.context_snapshot.operation_id, "op-1");
    assert_eq!(run.context_snapshot.trace_id, "trace-1");
    assert!(run.provider_response.output_text.contains("hello"));
    assert!(run.assistant_response_text.contains("hello"));
    assert!(run.events.iter().all(|event| event.trace_id == "trace-1"));
    assert_eq!(run.control_feedback.origin, "runtime_heuristic");
    assert_eq!(
        run.note.control_feedback,
        Some(run.control_feedback.clone())
    );
    assert_eq!(
        run.digest.control_feedback,
        Some(run.control_feedback.clone())
    );
    assert_eq!(run.tool_records.len(), 4);
    assert!(
        run.tool_records
            .iter()
            .all(|record| record.tool_name == "provider.call")
    );
    assert_eq!(run.provider_request_records.len(), 4);
    assert_eq!(run.provider_response_records.len(), 4);
    assert_eq!(
        run.provider_request_records
            .last()
            .map(|record| record.request_id.as_str()),
        Some("provider-request-op-1-r01-a04")
    );
    assert_eq!(run.round_records.len(), 1);
    assert!(run.step_records.len() >= 5);
    assert_eq!(run.turn_record.operation_id, "op-1");
    assert_eq!(run.routing_decision.operation_id, "op-1");
    assert_eq!(run.routing_action.operation_id, "op-1");
    assert_eq!(run.reasoning_view.operation_id, "op-1");
    assert_eq!(run.closure_trace.operation_id, "op-1");
    assert!(
        run.closure_trace
            .rendered_input
            .contains("Current request:")
    );
    let provider_event = run
        .events
        .iter()
        .find(|event| event.event_type == "provider.completed")
        .expect("provider.completed event should exist");
    let payload: ProviderEventPayload =
        serde_json::from_value(provider_event.payload.clone()).expect("payload should decode");
    assert!(payload.debug.is_some());
    let control_event = run
        .events
        .iter()
        .find(|event| event.event_type == "control.feedback_recorded")
        .expect("control.feedback_recorded event should exist");
    let control_payload: ControlFeedback =
        serde_json::from_value(control_event.payload.clone()).expect("payload should decode");
    assert_eq!(control_payload, run.control_feedback);
    let turn_event = run
        .events
        .iter()
        .find(|event| event.event_type == "turn.recorded")
        .expect("turn.recorded event should exist");
    let turn_payload: fin_contracts::TurnRecord =
        serde_json::from_value(turn_event.payload.clone()).expect("turn should decode");
    assert_eq!(turn_payload.turn_id, run.turn_record.turn_id);
    let inference_started = run
        .events
        .iter()
        .find(|event| event.event_type == "inference.started")
        .expect("inference.started should exist");
    assert!(
        inference_started
            .payload
            .get("rendered_input")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .contains("Original request:\nhello")
    );
    assert!(
        run.step_records
            .iter()
            .any(|step| step.step_kind == "provider_request" && !step.event_ids.is_empty())
    );
    assert!(
        run.step_records
            .iter()
            .any(|step| step.step_kind == "model_parse" && !step.event_ids.is_empty())
    );
    assert!(
        run.step_records
            .iter()
            .any(|step| step.step_kind == "control_feedback" && !step.event_ids.is_empty())
    );
}

#[test]
fn inference_builder_carries_runtime_policy_into_operation() {
    let worker = worker_runtime();
    let operation = InferenceOperationBuilder
        .build(
            &worker,
            InferenceRequest {
                operation_id: "op-2".into(),
                trace_id: "trace-2".into(),
                submitted_at: "2026-04-17T00:00:00Z".into(),
                refs: EntityRefs::default(),
                input: "say ok".into(),
                context: MinimalContextView {
                    continuity_tail: vec!["previous".into()],
                    summary: Some("recent continuity".into()),
                    ..MinimalContextView::default()
                },
            },
        )
        .expect("operation");

    assert_eq!(operation.payload.role.role_id.as_str(), "project");
    assert_eq!(
        operation
            .payload
            .provider_path
            .primary_target()
            .provider_name,
        "openai"
    );
    assert_eq!(operation.timeout_ms, Some(60_000));
    assert_eq!(
        operation.payload.context.summary.as_deref(),
        Some("recent continuity")
    );
}

#[test]
fn runtime_policy_snapshot_builds_from_project_default_role() {
    let user = UserConfig {
        default_provider: "openai".into(),
        providers: BTreeMap::from([(
            "openai".into(),
            UserProviderConfig {
                protocol: ProviderProtocol::OpenAiCompatible,
                base_url: "https://api.example.com/v1".into(),
                model: "gpt-5".into(),
                api_key: None,
                api_key_env: Some("OPENAI_API_KEY".into()),
                user_agent: None,
                headers: BTreeMap::new(),
            },
        )]),
        runtime: fin_config::UserRuntimeConfig::default(),
    };
    let system = ConfigMapper::map_user_to_system(&user).expect("mapping should succeed");

    let snapshot =
        RuntimePolicySnapshot::from_system(&system, None).expect("snapshot should build");
    assert_eq!(snapshot.role.role_id.as_str(), "project");
    assert_eq!(snapshot.protocol_version, "fin.m1");
    assert_eq!(snapshot.provider_strategy, ProviderStrategy::Priority);
    assert_eq!(
        snapshot.provider_path.primary_target().provider_name,
        "openai"
    );

    let encoded = serde_json::to_string(&snapshot).expect("snapshot should serialize");
    let decoded: RuntimePolicySnapshot =
        serde_json::from_str(&encoded).expect("snapshot should deserialize");
    assert_eq!(decoded, snapshot);
}

#[test]
fn worker_runtime_inherits_policy_snapshot() {
    let user = UserConfig {
        default_provider: "openai".into(),
        providers: BTreeMap::from([(
            "openai".into(),
            UserProviderConfig {
                protocol: ProviderProtocol::OpenAiCompatible,
                base_url: "https://api.example.com/v1".into(),
                model: "gpt-5".into(),
                api_key: None,
                api_key_env: Some("OPENAI_API_KEY".into()),
                user_agent: None,
                headers: BTreeMap::new(),
            },
        )]),
        runtime: fin_config::UserRuntimeConfig::default(),
    };
    let system = ConfigMapper::map_user_to_system(&user).expect("mapping should succeed");
    let runtime =
        WorkerRuntime::from_system(&system, "agent-project-leader", "worker-1", "runtime", None)
            .expect("worker runtime should build");

    assert_eq!(runtime.agent_id.as_str(), "agent-project-leader");
    assert_eq!(runtime.worker_id, "worker-1");
    assert_eq!(runtime.policy.provider_path.primary_target().model, "gpt-5");
}
