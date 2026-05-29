//! Red tests for prompt cache high-hit-rate guarantees.
//!
//! Invariants derived from Deepseek-reasonix ImmutablePrefix + AppendOnlyLog patterns:
//! 1. Immutable prefix (system prompt + tools) must be byte-stable across turns.
//! 2. Append-only history sections must not shrink or reorder between turns.
//! 3. Context fold must preserve immutable prefix byte-for-byte.
//! 4. ContextBudgetManager must produce graduated fold decisions (not binary).
//! 5. cached_tokens from provider must influence fold aggressiveness.

use super::*;
use fin_contracts::{MinimalContextView, RolePromptBlock, ToolCatalogBlock, ToolCatalogEntry, HistoryBlock};

fn make_plan(input: &str, context: &MinimalContextView) -> ContextAssemblyPlan {
    ContextAssemblyPlanner::default().build_plan(input, context)
}

fn make_context_with_tools_and_history(
    role_prompt: &str,
    tool_names: &[&str],
    history_lines: &[&str],
) -> MinimalContextView {
    let tools = if tool_names.is_empty() {
        None
    } else {
        Some(ToolCatalogBlock {
            model_tools: tool_names
                .iter()
                .map(|name| ToolCatalogEntry {
                    tool_name: (*name).into(),
                    kind: "function".into(),
                    summary: format!("{} does things", name),
                    purpose: format!("{} purpose", name),
                    when_to_use: vec![format!("use when you need {}", name)],
                    when_not_to_use: vec![],
                    input_schema_summary: "{}".into(),
                    output_schema_summary: "string".into(),
                    side_effects: vec![],
                    example_uses: vec![],
                })
                .collect(),
            framework_tools: vec![],
            tool_selection_policy: vec![],
            disabled_tools: vec![],
            hard_guards: vec![],
        })
    };
    let history = if history_lines.is_empty() {
        None
    } else {
        Some(HistoryBlock {
            recent_messages: history_lines.iter().map(|s| (*s).into()).collect(),
            recent_digests: vec![],
            recent_reasoning: vec![],
            recent_tool_activity: vec![],
        })
    };
    MinimalContextView {
        role_prompt: Some(RolePromptBlock {
            role_id: "system".into(),
            current_prompt_summary: role_prompt.into(),
            behavior_rules: vec!["Be helpful".into()],
            output_contract: vec![],
            ..RolePromptBlock::default()
        }),
        tools,
        history,
        ..MinimalContextView::default()
    }
}

/// Test 1: Immutable prefix hash must be stable when only history changes.
/// reasonix ImmutablePrefix ensures system+tools bytes never drift between turns.
/// fin ContextBaselineManager.stable_prefix_hash must stay identical when
/// only AppendOnlyHistory/VolatileTail sections change.
#[test]
fn immutable_prefix_hash_stable_across_turns() {
    let ctx_turn1 = make_context_with_tools_and_history(
        "You are a helpful assistant.",
        &["read_file", "write_file"],
        &["user: hello", "assistant: hi there"],
    );
    let plan1 = make_plan("what files are here?", &ctx_turn1);

    let ctx_turn2 = make_context_with_tools_and_history(
        "You are a helpful assistant.",
        &["read_file", "write_file"],
        &["user: hello", "assistant: hi there", "user: list files", "assistant: here are the files: a.rs, b.rs"],
    );
    let plan2 = make_plan("read a.rs", &ctx_turn2);

    let baseline_mgr = ContextBaselineManager::default();
    let record1 = baseline_mgr.create("sess-cache-1", &plan1, "system", "2026-05-29T00:00:00+08:00");
    let record2 = baseline_mgr.create("sess-cache-1", &plan2, "system", "2026-05-29T00:01:00+08:00");

    assert_eq!(
        record1.stable_prefix_hash, record2.stable_prefix_hash,
        "immutable prefix hash must be identical when only history changed"
    );
    assert_eq!(
        record1.tool_schema_hash, record2.tool_schema_hash,
        "tool schema hash must be identical when tools did not change"
    );
}

/// Test 2: Adding a history turn must NOT change immutable prefix hash.
/// This is the append-only invariant: history grows, prefix stays.
#[test]
fn adding_history_turn_preserves_prefix() {
    let ctx1 = make_context_with_tools_and_history(
        "System prompt",
        &["search"],
        &["turn 1"],
    );
    let ctx2 = make_context_with_tools_and_history(
        "System prompt",
        &["search"],
        &["turn 1", "turn 2", "turn 3", "turn 4", "turn 5"],
    );
    let plan1 = make_plan("query", &ctx1);
    let plan2 = make_plan("query", &ctx2);

    let immutable1: Vec<_> = plan1.sections.iter()
        .filter(|s| matches!(s.stability, ContextStabilityClass::Immutable))
        .collect();
    let immutable2: Vec<_> = plan2.sections.iter()
        .filter(|s| matches!(s.stability, ContextStabilityClass::Immutable))
        .collect();

    assert_eq!(immutable1.len(), immutable2.len(),
        "immutable section count must not change when history grows");
    for (a, b) in immutable1.iter().zip(immutable2.iter()) {
        assert_eq!(a.section_hash, b.section_hash,
            "immutable section '{}' hash must not change", a.section_id);
        assert_eq!(a.body, b.body,
            "immutable section '{}' body must be byte-identical", a.section_id);
    }
}

/// Test 3: ContextBaselineManager must detect tool schema change as requiring full reinject.
/// reasonix addTool/removeTool invalidates prefix → cache miss expected.
#[test]
fn tool_schema_change_triggers_full_reinject() {
    let ctx1 = make_context_with_tools_and_history(
        "System prompt",
        &["read_file"],
        &["turn 1"],
    );
    let ctx2 = make_context_with_tools_and_history(
        "System prompt",
        &["read_file", "write_file"],
        &["turn 1"],
    );
    let plan1 = make_plan("query", &ctx1);
    let plan2 = make_plan("query", &ctx2);

    let mgr = ContextBaselineManager::default();
    let record1 = mgr.create("sess-tool-change", &plan1, "system", "2026-05-29T00:00:00+08:00");
    let diff = mgr.diff(Some(&record1), &plan2, "system");

    assert!(diff.requires_full_reinject,
        "changing tool set must trigger full reinject");
    assert!(diff.tool_schema_hash != record1.tool_schema_hash,
        "tool schema hash must differ after adding a tool");
}

/// Test 4: ContextBudgetManager must produce graduated fold decisions.
/// reasonix has 5 threshold levels; fin currently only has binary.
#[test]
fn budget_manager_binary_decisions_work() {
    let mgr = ContextBudgetManager::default();

    // Below threshold → NoCompact
    let plan_low = ContextAssemblyPlan {
        sections: vec![],
        budget: ContextBudgetSnapshot {
            prompt_token_estimate: 50_000,
            compact_threshold_tokens: 120_000,
            should_compact: false,
            trigger_reason: "below".into(),
        },
    };
    let decision_low = mgr.decide(&plan_low, None);
    assert_eq!(decision_low.decision, ContextCompactionDecisionKind::NoCompact);

    // At threshold → PreTurnCompact (current binary behavior)
    let plan_high = ContextAssemblyPlan {
        sections: vec![],
        budget: ContextBudgetSnapshot {
            prompt_token_estimate: 125_000,
            compact_threshold_tokens: 120_000,
            should_compact: true,
            trigger_reason: "reached".into(),
        },
    };
    let decision_high = mgr.decide(&plan_high, None);
    assert_eq!(decision_high.decision, ContextCompactionDecisionKind::PreTurnCompact);

    // This test passes with current code (binary). The red test below checks for
    // the missing graduated behavior.
}

// Test 5 removed: requires new ContextBudgetDecision fields (cached_ratio, prefix_drift_detected)



/// Test 7 (RED): append-only history sections must not shrink between turns.
/// reasonix AppendOnlyLog ensures history never decreases in size.
/// fin ContextAssemblyPlan history sections must not shrink unless explicitly compacted.
#[test]
fn history_sections_monotonically_grow() {
    let ctx1 = make_context_with_tools_and_history(
        "System",
        &[],
        &["msg1", "msg2", "msg3"],
    );
    let ctx2 = make_context_with_tools_and_history(
        "System",
        &[],
        &["msg1", "msg2", "msg3", "msg4", "msg5"],
    );
    let plan1 = make_plan("q1", &ctx1);
    let plan2 = make_plan("q2", &ctx2);

    let history1_count = plan1.sections.iter()
        .filter(|s| s.stability == ContextStabilityClass::AppendOnlyHistory)
        .count();
    let history2_count = plan2.sections.iter()
        .filter(|s| s.stability == ContextStabilityClass::AppendOnlyHistory)
        .count();

    assert!(history2_count >= history1_count,
        "history sections must not shrink: turn1={history1_count} turn2={history2_count}");
}

// ==================== RED TESTS (behaviors not yet implemented) ====================

/// RED: ContextBudgetDecision must carry cached_ratio for prefix drift detection.
/// reasonix uses cache_hit_ratio to detect prefix drift and trigger re-fingerprint.
/// Current code: ContextBudgetDecision has no cached_ratio field.
/// After fix: add cached_ratio: f64 to ContextBudgetDecision.
#[test]
fn budget_decision_reports_cached_ratio() {
    let plan = ContextAssemblyPlan {
        sections: vec![],
        budget: ContextBudgetSnapshot {
            prompt_token_estimate: 100_000,
            compact_threshold_tokens: 120_000,
            should_compact: false,
            trigger_reason: "below".into(),
        },
    };
    let usage = fin_provider::TokenUsage {
        prompt_tokens: Some(100_000),
        completion_tokens: Some(100),
        total_tokens: Some(100_100),
        cached_tokens: Some(85_000),
        reasoning_tokens: None,
        usage_source: "provider_openai".into(),
    };
    let mgr = ContextBudgetManager::new(120_000);
    let decision = mgr.decide(&plan, Some(&usage));
    assert!(decision.cached_ratio > 0.8,
        "expected cached_ratio > 0.8, got {}", decision.cached_ratio);
}

/// RED: ContextBaselineManager must track prefix drift history.
/// reasonix ImmutablePrefix.verifyFingerprint() catches drift.
/// Current code: diff() compares two snapshots but does not record history.
/// After fix: add drift_history() method that returns drift events.
#[test]
fn baseline_manager_tracks_drift_history() {
    let planner = ContextAssemblyPlanner::default();
    let ctx1 = fin_contracts::MinimalContextView {
        role_prompt: Some(fin_contracts::RolePromptBlock {
            role_id: "system".into(),
            current_prompt_summary: "Version A".into(),
            ..fin_contracts::RolePromptBlock::default()
        }),
        ..fin_contracts::MinimalContextView::default()
    };
    let ctx2 = fin_contracts::MinimalContextView {
        role_prompt: Some(fin_contracts::RolePromptBlock {
            role_id: "system".into(),
            current_prompt_summary: "Version B changed".into(),
            ..fin_contracts::RolePromptBlock::default()
        }),
        ..fin_contracts::MinimalContextView::default()
    };
    let plan1 = planner.build_plan("q1", &ctx1);
    let plan2 = planner.build_plan("q2", &ctx2);
    let mgr = ContextBaselineManager::default();
    let r1 = mgr.create("sess-drift", &plan1, "system", "t1");
    let diff = mgr.diff(Some(&r1), &plan2, "system");
    assert!(diff.requires_full_reinject);
    let history = mgr.drift_history("sess-drift");
    assert_eq!(history.len(), 1, "expected 1 drift event, got {}", history.len());
}

/// RED: ContextBudgetDecision must include tail_budget for fold operations.
/// reasonix folds with tail_fraction to preserve recent context.
/// Current code: ContextBudgetDecision has no tail_budget field.
/// After fix: add tail_budget: Option<usize>.
#[test]
fn fold_decision_includes_tail_budget() {
    let plan = ContextAssemblyPlan {
        sections: vec![],
        budget: ContextBudgetSnapshot {
            prompt_token_estimate: 95_000,
            compact_threshold_tokens: 120_000,
            should_compact: true,
            trigger_reason: "reached".into(),
        },
    };
    let mgr = ContextBudgetManager::new(120_000);
    let decision = mgr.decide(&plan, None);
    assert!(decision.tail_budget.is_some(),
        "fold decision must specify tail_budget");
    let tail = decision.tail_budget.unwrap();
    assert!(tail > 0 && tail < 95_000,
        "tail_budget must be between 0 and prompt_tokens, got {tail}");
}

/// RED: ContextBudgetManager must produce graduated fold decisions.
/// reasonix has 4 threshold levels; fin currently only NoCompact / PreTurnCompact.
/// After fix: add FoldLevel enum with 4 levels, ContextBudgetDecision.fold_level field.
#[test]
fn budget_manager_produces_graduated_decisions() {
    // At 50%: should be NoFold
    // At 76%: should be NormalFold (tail_fraction=0.2)
    // At 82%: should be AggressiveFold (tail_fraction=0.1)
    // At 88%: should be ForceSummary
    let mgr = ContextBudgetManager::new(120_000);

    let plan_76 = ContextAssemblyPlan {
        sections: vec![],
        budget: ContextBudgetSnapshot {
            prompt_token_estimate: 91_200,
            compact_threshold_tokens: 120_000,
            should_compact: true,
            trigger_reason: "above".into(),
        },
    };
    let plan_88 = ContextAssemblyPlan {
        sections: vec![],
        budget: ContextBudgetSnapshot {
            prompt_token_estimate: 105_600,
            compact_threshold_tokens: 120_000,
            should_compact: true,
            trigger_reason: "above".into(),
        },
    };
    let d76 = mgr.decide(&plan_76, None);
    let d88 = mgr.decide(&plan_88, None);
    assert!(d76.fold_level != d88.fold_level,
        "76pct ({:?}) and 88pct ({:?}) should produce different fold levels", d76.fold_level, d88.fold_level);
}

/// RED: ContextCompactionEngine must preserve Immutable prefix sections.
/// reasonix compactInPlace rewrites history but never touches system prompt.
/// Current code: ContextCompactionEngine::compact() takes CompactionInput,
/// returns CompactedHistoryRecord — no awareness of ContextAssemblyPlan sections.
/// After fix: compact should accept plan sections and preserve Immutable/RarelyChanging.
#[test]
fn compaction_preserves_immutable_prefix() {
    let ctx = fin_contracts::MinimalContextView {
        role_prompt: Some(fin_contracts::RolePromptBlock {
            role_id: "system".into(),
            current_prompt_summary: "Critical system prompt".into(),
            behavior_rules: vec!["Rule 1".into()],
            ..fin_contracts::RolePromptBlock::default()
        }),
        ..fin_contracts::MinimalContextView::default()
    };
    let planner = ContextAssemblyPlanner::default();
    let plan = planner.build_plan("input", &ctx);
    let immutable_before: Vec<_> = plan.sections.iter()
        .filter(|s| s.stability == ContextStabilityClass::Immutable)
        .collect();
    assert!(!immutable_before.is_empty());

    // Compact the history portion using ContextCompactionEngine
    let engine = ContextCompactionEngine::default();
    let input = CompactionInput {
        session_id: "sess-compact-immutable".into(),
        task_id: None,
        trigger_reason: "threshold".into(),
        recent_messages: (0..20).map(|i| format!("message {i}")).collect(),
        digest_records: vec![],
        tool_records: vec![],
        retain_recent_count: 5,
        compacted_at: "t1".into(),
    };
    let compacted = engine.compact(input);

    // Re-build plan with compacted history
    let compacted_ctx = fin_contracts::MinimalContextView {
        role_prompt: ctx.role_prompt.clone(),
        tools: ctx.tools.clone(),
        history: Some(fin_contracts::HistoryBlock {
            recent_messages: compacted.retained_messages.clone(),
            ..fin_contracts::HistoryBlock::default()
        }),
        ..fin_contracts::MinimalContextView::default()
    };
    let plan_after = planner.build_plan("input", &compacted_ctx);
    let immutable_after: Vec<_> = plan_after.sections.iter()
        .filter(|s| s.stability == ContextStabilityClass::Immutable)
        .collect();

    assert_eq!(immutable_before.len(), immutable_after.len(),
        "immutable section count must survive compaction: before={}, after={}",
        immutable_before.len(), immutable_after.len());
    for (before, after) in immutable_before.iter().zip(immutable_after.iter()) {
        assert_eq!(before.body, after.body,
            "immutable section '{}' must be byte-identical after compaction", before.section_id);
    }
}

