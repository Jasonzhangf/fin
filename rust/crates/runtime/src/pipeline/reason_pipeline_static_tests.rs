#[test]
fn reasoning_pipeline_uses_unique_node_type_names_and_no_round_execution_alias() {
    let pipeline = include_str!("reason_pipeline.rs");
    for node in [
        "ReasonReq01Seed",
        "ReasonReq02ContextPlan",
        "ReasonReq03BudgetedContext",
        "ReasonReq04RenderedInput",
        "ReasonReq05ProviderCall",
        "ReasonResp06ModelOutput",
        "ReasonResp07ParsedContract",
        "ReasonResp08RuntimeDecision",
        "ReasonResp09Closure",
    ] {
        let struct_decl = format!("struct {node}");
        assert!(
            pipeline.contains(&struct_decl),
            "missing unique reasoning pipeline node {node}"
        );
    }

    let rounds = include_str!("../closure/rounds.rs");
    assert!(
        !rounds.contains("struct RoundExecution"),
        "generic RoundExecution must not remain as reasoning-chain truth"
    );
    assert!(
        rounds.contains("ReasonReq01Seed")
            && rounds.contains("ReasonResp09ClosureBuilder.build(runtime_decision)"),
        "execute_round must connect ReasonReq01Seed through ReasonResp09Closure"
    );
}

#[test]
fn reasoning_pipeline_builders_only_convert_adjacent_nodes() {
    let pipeline = include_str!("reason_pipeline.rs");
    let adjacency = [
        (
            "ReasonReq02ContextPlanBuilder",
            "ReasonReq01Seed",
            "ReasonReq02ContextPlan",
        ),
        (
            "ReasonReq03BudgetedContextBuilder",
            "ReasonReq02ContextPlan",
            "ReasonReq03BudgetedContext",
        ),
        (
            "ReasonReq04RenderedInputBuilder",
            "ReasonReq03BudgetedContext",
            "ReasonReq04RenderedInput",
        ),
        (
            "ReasonReq05ProviderCallBuilder",
            "ReasonReq04RenderedInput",
            "ReasonReq05ProviderCall",
        ),
        (
            "ReasonResp06ModelOutputParser",
            "ReasonReq05ProviderCall",
            "ReasonResp06ModelOutput",
        ),
        (
            "ReasonResp07ParsedContractParser",
            "ReasonResp06ModelOutput",
            "ReasonResp07ParsedContract",
        ),
        (
            "ReasonResp08RuntimeDecisionBuilder",
            "ReasonResp07ParsedContract",
            "ReasonResp08RuntimeDecision",
        ),
        (
            "ReasonResp09ClosureBuilder",
            "ReasonResp08RuntimeDecision",
            "ReasonResp09Closure",
        ),
    ];
    for (owner, input, output) in adjacency {
        assert!(
            pipeline.contains(owner),
            "missing owning builder/parser {owner}"
        );
        assert!(
            pipeline.contains(&format!("{owner} {{")) || pipeline.contains(&format!("{owner};")),
            "{owner} declaration missing"
        );
        assert!(
            pipeline.contains(&format!("{input}) -> {output}"))
                || pipeline.contains(&format!("{input},"))
                    && pipeline.contains(&format!("{output} {{")),
            "{owner} must expose adjacent {input} -> {output} conversion"
        );
    }
}
