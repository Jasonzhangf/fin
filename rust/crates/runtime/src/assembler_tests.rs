use crate::{
    ContextBaselineManager, ContextBudgetManager, ContextCompactionDecisionKind,
    ContextCompactionEngine, ModelInputAssembler,
};
use fin_contracts::{
    CurrentInputBlock, DaemonStateSummary, DigestRecord, EntityRefs, HistoryBlock,
    InputAttachmentSummary, MinimalContextView, PeerBindingSummary, PeerContextBlock,
    PeerDescriptorSummary, ProjectContextBlock, RolePromptBlock, ToolCatalogBlock,
    ToolCatalogEntry, ToolExecutionRecord,
};
use fin_provider::TokenUsage;

#[test]
fn model_input_assembler_renders_role_tools_history_and_project_scope() {
    let rendered = ModelInputAssembler::default().assemble(
        "current ask",
        &MinimalContextView {
            summary: Some("recent continuity".into()),
            continuity_tail: vec!["u1".into(), "a1".into()],
            role_prompt: Some(RolePromptBlock {
                role_id: "default".into(),
                current_prompt_summary: "summary".into(),
                behavior_rules: vec!["rule-1".into()],
                output_contract: vec![
                    "wrap the user-visible answer in <fin_user_response>...</fin_user_response>"
                        .into(),
                    "exact control feedback JSON shape example: {\"origin\":\"model_output_contract_v1\"}"
                        .into(),
                ],
                ..Default::default()
            }),
            tools: Some(ToolCatalogBlock {
                model_tools: vec![ToolCatalogEntry {
                    tool_name: "peer.list".into(),
                    kind: "agent_tool".into(),
                    summary: "list peers".into(),
                    when_to_use: vec!["need peer topology".into()],
                    when_not_to_use: vec!["topology is already known".into()],
                    input_schema_summary: "optional limit".into(),
                    output_schema_summary: "peer list".into(),
                    example_uses: vec!["list peers before choosing a route target".into()],
                    ..Default::default()
                }],
                framework_tools: vec![ToolCatalogEntry {
                    tool_name: "provider.call".into(),
                    summary: "invoke provider".into(),
                    input_schema_summary: "compiled prompt".into(),
                    ..Default::default()
                }],
                tool_selection_policy: vec!["no model tools".into()],
                ..Default::default()
            }),
            history: Some(HistoryBlock {
                recent_messages: vec!["user: hi".into(), "assistant: hello".into()],
                ..Default::default()
            }),
            project: Some(ProjectContextBlock {
                project_root: Some("/tmp/fin".into()),
                focus_summary: Some("focus".into()),
                ..Default::default()
            }),
            peer: Some(PeerContextBlock {
                topology_summary: Some("local-only placeholder".into()),
                active_peer_ids: vec!["local-worker-1".into()],
                peers: vec![PeerDescriptorSummary {
                    peer_id: "local-worker-1".into(),
                    label: "local worker".into(),
                    peer_kind: "agent".into(),
                    presence_state: "local_only".into(),
                    health_state: Some("unknown".into()),
                    capability_ids: vec!["peer.list".into()],
                    supports_session_binding: true,
                    supports_agentic_execution: true,
                }],
                binding: Some(PeerBindingSummary {
                    binding_state: Some("local_execution_only".into()),
                    ..Default::default()
                }),
                daemon: Some(DaemonStateSummary {
                    supervision_state: Some("not_attached".into()),
                    ..Default::default()
                }),
                ..Default::default()
            }),
            current_input: Some(CurrentInputBlock {
                input: "current ask".into(),
                source: "channel.qqbot".into(),
                operation_id: "op-1".into(),
                trace_id: "trace-1".into(),
                attachments: vec![InputAttachmentSummary {
                    kind: "image/png".into(),
                    name: Some("sample.png".into()),
                    url: Some("https://example.com/sample.png".into()),
                    width: Some(100),
                    height: Some(50),
                    ..Default::default()
                }],
            }),
            ..Default::default()
        },
    );

    assert!(rendered.contains("Context summary"));
    assert!(rendered.contains("Continuity tail"));
    assert!(rendered.contains("Agent prompt"));
    assert!(rendered.contains("Structured output contract"));
    assert!(rendered.contains("model_output_contract_v1"));
    assert!(rendered.contains("Model tools"));
    assert!(rendered.contains("peer.list"));
    assert!(rendered.contains("use: need peer topology"));
    assert!(rendered.contains("avoid: topology is already known"));
    assert!(rendered.contains("input: optional limit"));
    assert!(rendered.contains("output: peer list"));
    assert!(rendered.contains("example: list peers before choosing a route target"));
    assert!(rendered.contains("Framework capabilities"));
    assert!(rendered.contains("Tool selection policy"));
    assert!(rendered.contains("Current interaction ledger"));
    assert!(rendered.contains("Project scope"));
    assert!(rendered.contains("Peer scope"));
    assert!(rendered.contains("Current request"));
    assert!(rendered.contains("Request envelope"));
    assert!(rendered.contains("Input attachments"));
    assert!(rendered.contains("sample.png"));
    assert!(rendered.contains("Mandatory final answer format"));
    assert!(rendered.contains("Mandatory final answer example"));
    assert!(rendered.contains("0.98 -> 98"));
    assert!(rendered.contains("<fin_user_response>"));
    assert!(rendered.contains("<fin_control_feedback>"));
}

#[test]
fn model_input_assembler_renders_apply_patch_guidance_verbatim_in_tool_catalog() {
    let rendered = ModelInputAssembler::default().assemble(
        "patch the file",
        &MinimalContextView {
            role_prompt: Some(RolePromptBlock {
                role_id: "project".into(),
                current_prompt_summary: "summary".into(),
                output_contract: vec!["must use fin blocks".into()],
                ..Default::default()
            }),
            tools: Some(ToolCatalogBlock {
                model_tools: vec![ToolCatalogEntry {
                    tool_name: "apply_patch".into(),
                    kind: "model_tool".into(),
                    summary: "apply deterministic workspace file edits".into(),
                    when_to_use: vec![
                        "you need to modify files deterministically instead of only describing edits"
                            .into(),
                    ],
                    when_not_to_use: vec![
                        "you are still exploring and do not know the concrete change yet".into(),
                    ],
                    input_schema_summary:
                        "mode=replace: path + old_string + new_string + replace_all? (use old_string=\"\" to create a new file) ; mode=patch: patch"
                            .into(),
                    output_schema_summary: "patch receipt + modified file refs".into(),
                    example_uses: vec![
                        "replace one exact function body in src/runtime.rs".into(),
                        "apply a multi-file V4A patch for a small deterministic refactor".into(),
                    ],
                    ..Default::default()
                }],
                ..Default::default()
            }),
            ..Default::default()
        },
    );

    assert!(rendered.contains("apply_patch"));
    assert!(rendered.contains("mode=replace"));
    assert!(rendered.contains("replace_all?"));
    assert!(rendered.contains("old_string=\"\""));
    assert!(rendered.contains("mode=patch"));
    assert!(rendered.contains("patch receipt + modified file refs"));
    assert!(rendered.contains("replace one exact function body in src/runtime.rs"));
    assert!(rendered.contains("multi-file V4A patch"));
}

#[test]
fn model_input_assembler_orders_stable_prefix_before_history_and_current_tail() {
    let context = MinimalContextView {
        summary: Some("slow compacted summary".into()),
        continuity_tail: vec!["slow continuity".into()],
        role_prompt: Some(RolePromptBlock {
            role_id: "project".into(),
            current_prompt_summary: "stable role".into(),
            behavior_rules: vec!["stable behavior".into()],
            output_contract: vec!["stable output contract".into()],
            ..Default::default()
        }),
        tools: Some(ToolCatalogBlock {
            model_tools: vec![ToolCatalogEntry {
                tool_name: "stable.tool".into(),
                summary: "stable tool schema".into(),
                ..Default::default()
            }],
            ..Default::default()
        }),
        history: Some(HistoryBlock {
            recent_messages: vec!["user: older".into(), "assistant: older".into()],
            recent_tool_activity: vec!["latest tool result".into()],
            ..Default::default()
        }),
        current_input: Some(CurrentInputBlock {
            input: "latest user input".into(),
            source: "test".into(),
            operation_id: "op-cache".into(),
            trace_id: "trace-cache".into(),
            attachments: Vec::new(),
        }),
        ..Default::default()
    };
    let rendered = ModelInputAssembler::default().assemble("latest user input", &context);

    let agent = rendered.find("Agent prompt:").expect("agent prompt");
    let mandatory = rendered
        .find("Mandatory final answer format:")
        .expect("mandatory format");
    let tools = rendered.find("Model tools:").expect("tools");
    let summary = rendered.find("Context summary:").expect("summary");
    let history = rendered
        .find("Current interaction ledger:")
        .expect("history");
    let tool_tail = rendered
        .find("Current tool execution history:")
        .expect("tool history");
    let request = rendered.find("Current request:").expect("request");

    assert!(
        agent < mandatory,
        "stable role prompt must start stable prefix"
    );
    assert!(
        mandatory < tools,
        "mandatory output format belongs to stable prefix"
    );
    assert!(
        tools < summary,
        "static tool schema must precede slow context"
    );
    assert!(
        summary < history,
        "slow summary must precede append-only history"
    );
    assert!(history < tool_tail, "tool results belong near history tail");
    assert!(
        tool_tail < request,
        "latest current request must be last tail section"
    );
    assert!(rendered.trim_end().ends_with("latest user input"));
}

#[test]
fn context_assembly_plan_reports_budget_threshold_without_triggering_below_limit() {
    let context = MinimalContextView {
        role_prompt: Some(RolePromptBlock {
            role_id: "project".into(),
            current_prompt_summary: "stable".into(),
            output_contract: vec!["stable contract".into()],
            ..Default::default()
        }),
        current_input: Some(CurrentInputBlock {
            input: "small".into(),
            source: "test".into(),
            operation_id: "op-small".into(),
            trace_id: "trace-small".into(),
            attachments: Vec::new(),
        }),
        ..Default::default()
    };
    let plan = crate::ContextAssemblyPlanner {
        compact_threshold_tokens: 100_000,
    }
    .build_plan("small", &context);

    assert!(!plan.budget.should_compact);
    assert_eq!(plan.budget.trigger_reason, "below_threshold");
    assert!(
        plan.sections
            .iter()
            .all(|section| section.token_estimate > 0)
    );
}

#[test]
fn context_assembly_plan_reports_threshold_compact_when_estimate_exceeds_limit() {
    let context = MinimalContextView {
        role_prompt: Some(RolePromptBlock {
            role_id: "project".into(),
            current_prompt_summary: "stable".into(),
            output_contract: vec!["stable contract".into()],
            ..Default::default()
        }),
        history: Some(HistoryBlock {
            recent_messages: vec!["x".repeat(500)],
            ..Default::default()
        }),
        current_input: Some(CurrentInputBlock {
            input: "small".into(),
            source: "test".into(),
            operation_id: "op-large".into(),
            trace_id: "trace-large".into(),
            attachments: Vec::new(),
        }),
        ..Default::default()
    };
    let plan = crate::ContextAssemblyPlanner {
        compact_threshold_tokens: 10,
    }
    .build_plan("small", &context);

    assert!(plan.budget.should_compact);
    assert_eq!(plan.budget.trigger_reason, "estimated_prompt_tokens>=10");
}

#[test]
fn context_baseline_requires_full_once_then_diff_when_stable_prefix_unchanged() {
    let context = MinimalContextView {
        role_prompt: Some(RolePromptBlock {
            role_id: "project".into(),
            current_prompt_summary: "stable role".into(),
            output_contract: vec!["stable contract".into()],
            ..Default::default()
        }),
        tools: Some(ToolCatalogBlock {
            model_tools: vec![ToolCatalogEntry {
                tool_name: "stable.tool".into(),
                summary: "stable tool schema".into(),
                ..Default::default()
            }],
            ..Default::default()
        }),
        history: Some(HistoryBlock {
            recent_messages: vec!["user: old".into()],
            ..Default::default()
        }),
        current_input: Some(CurrentInputBlock {
            input: "first".into(),
            source: "test".into(),
            operation_id: "op-1".into(),
            trace_id: "trace-1".into(),
            attachments: Vec::new(),
        }),
        ..Default::default()
    };
    let planner = crate::ContextAssemblyPlanner {
        compact_threshold_tokens: 100_000,
    };
    let first_plan = planner.build_plan("first", &context);
    let manager = ContextBaselineManager;
    let first_diff = manager.diff(None, &first_plan, "project");
    assert!(first_diff.requires_full_reinject);
    assert_eq!(first_diff.changed_fields, vec!["missing_baseline"]);

    let baseline = manager.create("session-1", &first_plan, "project", "2026-05-23T00:00:00Z");
    let mut next_context = context.clone();
    next_context.history = Some(HistoryBlock {
        recent_messages: vec!["user: old".into(), "assistant: new append".into()],
        ..Default::default()
    });
    next_context.current_input = Some(CurrentInputBlock {
        input: "second".into(),
        source: "test".into(),
        operation_id: "op-2".into(),
        trace_id: "trace-2".into(),
        attachments: Vec::new(),
    });
    let second_plan = planner.build_plan("second", &next_context);
    let second_diff = manager.diff(Some(&baseline), &second_plan, "project");
    assert!(!second_diff.requires_full_reinject);
    assert!(second_diff.changed_fields.is_empty());
}

#[test]
fn context_budget_manager_uses_provider_usage_for_threshold_decisions() {
    let context = MinimalContextView {
        role_prompt: Some(RolePromptBlock {
            role_id: "project".into(),
            current_prompt_summary: "stable".into(),
            ..Default::default()
        }),
        current_input: Some(CurrentInputBlock {
            input: "small".into(),
            source: "test".into(),
            operation_id: "op-budget".into(),
            trace_id: "trace-budget".into(),
            attachments: Vec::new(),
        }),
        ..Default::default()
    };
    let plan = crate::ContextAssemblyPlanner {
        compact_threshold_tokens: 100_000,
    }
    .build_plan("small", &context);
    let manager = ContextBudgetManager::new(1_000);
    let low = TokenUsage {
        prompt_tokens: Some(500),
        completion_tokens: Some(20),
        total_tokens: Some(520),
        cached_tokens: Some(250),
        reasoning_tokens: Some(3),
        usage_source: "provider_anthropic".into(),
    };
    let low_decision = manager.decide(&plan, Some(&low));
    assert_eq!(
        low_decision.decision,
        ContextCompactionDecisionKind::NoCompact
    );
    assert_eq!(low_decision.evidence_strength, "strong");
    assert_eq!(low_decision.reason, "below_threshold");

    let high = TokenUsage {
        prompt_tokens: Some(1_500),
        completion_tokens: Some(20),
        total_tokens: Some(1_520),
        cached_tokens: Some(900),
        reasoning_tokens: Some(8),
        usage_source: "provider_anthropic".into(),
    };
    let high_decision = manager.decide(&plan, Some(&high));
    assert_eq!(
        high_decision.decision,
        ContextCompactionDecisionKind::PreTurnCompact
    );
    assert_eq!(high_decision.reason, "prompt_tokens>=1000");
}

#[test]
fn compact_engine_replaces_history_and_retains_drawing_artifact_refs() {
    let refs = EntityRefs {
        session_id: Some("session-draw".into()),
        task_id: Some("task-draw".into()),
        ..Default::default()
    };
    let digest = DigestRecord {
        digest_id: "digest-1".into(),
        closure_id: "closure-1".into(),
        refs: refs.clone(),
        summary: "iteration 1 produced a cat image".into(),
        continuity_tail: vec!["next edit target is iter-1".into()],
        note_refs: Vec::new(),
        artifact_candidates: vec!["images/iter-1.png".into(), "images/iter-2.png".into()],
        control_feedback: None,
        created_at: "2026-05-23T00:00:00Z".into(),
    };
    let tool = ToolExecutionRecord {
        tool_call_id: "tool-image-edit-1".into(),
        operation_id: "op-draw".into(),
        trace_id: "trace-draw".into(),
        refs,
        tool_name: "image.edit".into(),
        tool_kind: "model_tool".into(),
        title: "Edited image".into(),
        purpose: "draw iteration".into(),
        target_kind: Some("image".into()),
        target_ref: Some("images/iter-2.png".into()),
        input_summary: Some("make the cat blue".into()),
        output_summary: Some("created iter-2".into()),
        status: "completed".into(),
        started_at: "2026-05-23T00:00:00Z".into(),
        ended_at: Some("2026-05-23T00:00:01Z".into()),
        duration_ms: Some(1000),
        side_effects: Vec::new(),
        artifact_refs: vec!["images/iter-2.png".into(), "images/mask-1.png".into()],
        error_summary: None,
    };

    let record = ContextCompactionEngine.compact(crate::CompactionInput {
        session_id: "session-draw".into(),
        task_id: Some("task-draw".into()),
        trigger_reason: "prompt_tokens>=1000".into(),
        recent_messages: vec![
            "user: draw cat".into(),
            "assistant: created iter-1".into(),
            "user: make it blue".into(),
            "assistant: created iter-2".into(),
        ],
        digest_records: vec![digest],
        tool_records: vec![tool],
        retain_recent_count: 2,
        compacted_at: "2026-05-23T00:00:02Z".into(),
    });

    assert_eq!(record.replaced_message_count, 2);
    assert_eq!(
        record.retained_messages,
        vec!["user: make it blue", "assistant: created iter-2"]
    );
    assert!(record.summary.contains("iteration 1 produced a cat image"));
    assert!(
        record
            .retained_artifact_refs
            .contains(&"images/iter-1.png".into())
    );
    assert!(
        record
            .retained_artifact_refs
            .contains(&"images/iter-2.png".into())
    );
    assert!(
        record
            .retained_artifact_refs
            .contains(&"images/mask-1.png".into())
    );
    assert_eq!(record.retained_tool_refs, vec!["tool-image-edit-1"]);
}

#[test]
fn default_context_budget_does_not_compact_normal_turn() {
    let context = MinimalContextView {
        role_prompt: Some(RolePromptBlock {
            role_id: "project".into(),
            current_prompt_summary: "stable normal role".into(),
            output_contract: vec!["stable contract".into()],
            ..Default::default()
        }),
        history: Some(HistoryBlock {
            recent_messages: vec!["user: hello".into(), "assistant: hi".into()],
            ..Default::default()
        }),
        current_input: Some(CurrentInputBlock {
            input: "normal turn".into(),
            source: "test".into(),
            operation_id: "op-normal".into(),
            trace_id: "trace-normal".into(),
            attachments: Vec::new(),
        }),
        ..Default::default()
    };

    let plan = crate::ContextAssemblyPlanner::default().build_plan("normal turn", &context);
    let decision = ContextBudgetManager::default().decide(&plan, None);

    assert!(!plan.budget.should_compact);
    assert_eq!(decision.decision, ContextCompactionDecisionKind::NoCompact);
    assert_eq!(decision.evidence_strength, "weak");
}
