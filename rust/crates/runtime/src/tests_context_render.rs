use super::*;

#[test]
fn run_closure_renders_context_into_provider_input() {
    let mut runtime = M1Runtime::default();
    let worker = worker_runtime();
    let op = InferenceOperationBuilder
        .build(
            &worker,
            InferenceRequest {
                operation_id: "op-context".into(),
                trace_id: "trace-context".into(),
                submitted_at: "2026-04-17T00:00:00Z".into(),
                refs: EntityRefs::default(),
                input: "answer current turn".into(),
                context: MinimalContextView {
                    continuity_tail: vec!["first".into(), "second".into()],
                    summary: Some("carry previous state".into()),
                    role_prompt: Some(fin_contracts::RolePromptBlock {
                        role_id: "default".into(),
                        output_contract: vec![
                            r#"exact control feedback JSON shape example: {"origin":"model_output_contract_v1"}"#.into(),
                            "do not emit extra control-feedback keys outside the fin whitelist".into(),
                        ],
                        ..Default::default()
                    }),
                    ..MinimalContextView::default()
                },
            },
        )
        .expect("build operation");

    let run = runtime
        .run_closure(op, &provider())
        .expect("closure should run");
    for fragment in [
        "Context summary:",
        "carry previous state",
        "Continuity tail:",
        "Structured output contract:",
        "model_output_contract_v1",
        "do not emit extra control-feedback keys",
        "Current request:
answer current turn",
    ] {
        assert!(
            run.prepared_request.rendered_input.contains(fragment),
            "missing context fragment {fragment}"
        );
    }
}
