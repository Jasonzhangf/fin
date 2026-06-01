use super::*;

fn make_compact_input(messages: Vec<&str>, retain: usize) -> CompactionInput {
    CompactionInput {
        session_id: "s-1".into(),
        task_id: None,
        trigger_reason: "budget".into(),
        recent_messages: messages.into_iter().map(String::from).collect(),
        digest_records: vec![],
        tool_records: vec![],
        retain_recent_count: retain,
        compacted_at: "2026-06-01T00:00:00Z".into(),
    }
}

#[test]
fn compact_empty_input_yields_empty_retained() {
    let engine = ContextCompactionEngine;
    let input = make_compact_input(vec![], 2);
    let result = engine.compact(input);
    assert!(result.retained_messages.is_empty());
    assert!(result.summary.is_empty());
    assert_eq!(result.replaced_message_count, 0);
}

#[test]
fn compact_retain_count_zero_defaults_to_one() {
    let engine = ContextCompactionEngine;
    let input = make_compact_input(vec!["a", "b"], 0);
    let result = engine.compact(input);
    assert_eq!(result.retained_messages.len(), 1);
    assert_eq!(result.retained_messages[0], "b");
    assert_eq!(result.replaced_message_count, 1);
}

#[test]
fn compact_retains_recent_and_compacts_old() {
    let engine = ContextCompactionEngine;
    let input = make_compact_input(vec!["old1", "old2", "recent"], 1);
    let result = engine.compact(input);
    assert_eq!(result.retained_messages, vec!["recent"]);
    assert_eq!(result.replaced_message_count, 2);
    assert!(result.summary.contains("message: old1"));
    assert!(result.summary.contains("message: old2"));
}

#[test]
fn compact_extracts_digest_summary_and_continuity() {
    use fin_contracts::{DigestRecord, EntityRefs};
    let engine = ContextCompactionEngine;
    let digest = DigestRecord {
        digest_id: "d-1".into(),
        closure_id: "c-1".into(),
        refs: EntityRefs::default(),
        summary: "fixed bug".into(),
        continuity_tail: vec!["check auth".into()],
        note_refs: vec![],
        artifact_candidates: vec![],
        control_feedback: None,
        created_at: "t".into(),
    };
    let mut input = make_compact_input(vec!["msg1"], 0);
    input.digest_records = vec![digest];
    let result = engine.compact(input);
    assert!(result.summary.contains("digest: fixed bug"));
    assert!(result.summary.contains("continuity: check auth"));
}

#[test]
fn compact_deduplicates_tool_artifact_refs() {
    use fin_contracts::{EntityRefs, ToolExecutionRecord};
    let engine = ContextCompactionEngine;
    let tool = ToolExecutionRecord {
        tool_call_id: "tc-1".into(),
        operation_id: "op-1".into(),
        trace_id: "t-1".into(),
        refs: EntityRefs::default(),
        tool_name: "apply_patch".into(),
        tool_kind: "editor".into(),
        title: "apply patch".into(),
        purpose: "fix code".into(),
        target_kind: None,
        target_ref: None,
        input_summary: Some("fix".into()),
        output_summary: None,
        status: "success".into(),
        started_at: "t".into(),
        ended_at: None,
        duration_ms: None,
        side_effects: vec![],
        artifact_refs: vec!["file:///a.rs".into(), "file:///a.rs".into()],
        error_summary: None,
    };
    let mut input = make_compact_input(vec!["msg1"], 0);
    input.tool_records = vec![tool];
    let result = engine.compact(input);
    assert_eq!(result.retained_artifact_refs.len(), 1);
    assert_eq!(result.retained_tool_refs, vec!["tc-1"]);
}
