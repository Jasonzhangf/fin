use crate::tool_catalog_dynamic::{DynamicToolCatalogInput, build_dynamic_tool_catalog_block};
use fin_contracts::{
    CurrentInputBlock, KnowledgeArtifactBlock, MinimalContextView, ToolExecutionRecord,
};
use std::collections::BTreeSet;

const ROUND_RECENT_MESSAGES_WINDOW: usize = 8;
const ROUND_RECENT_TOOL_WINDOW: usize = 6;
const ROUND_ARTIFACT_WINDOW: usize = 12;
const ROUND_CONTINUITY_WINDOW: usize = 10;

pub(super) struct DynamicRoundContextInput<'a> {
    pub(super) role_id: &'a str,
    pub(super) base_context: &'a MinimalContextView,
    pub(super) operation_id: &'a str,
    pub(super) trace_id: &'a str,
    pub(super) round_index: u32,
    pub(super) current_input: &'a str,
    pub(super) previous_assistant_response: Option<&'a str>,
    pub(super) recent_tool_records: &'a [ToolExecutionRecord],
}

pub(super) fn build_round_context(input: DynamicRoundContextInput<'_>) -> MinimalContextView {
    let mut context = input.base_context.clone();
    let previous_current_input = context.current_input.clone();
    let mut history = context.history.take().unwrap_or_default();
    if let Some(previous_assistant_response) = input
        .previous_assistant_response
        .filter(|value| input.round_index > 1 && !value.trim().is_empty())
    {
        bounded_push(
            &mut history.recent_messages,
            format!(
                "assistant(previous round): {}",
                short_text(previous_assistant_response, 240)
            ),
            ROUND_RECENT_MESSAGES_WINDOW,
        );
    }
    let tool_lines = render_recent_tool_activity(input.recent_tool_records);
    if !tool_lines.is_empty() {
        history.recent_tool_activity = tool_lines;
    }
    context.history = Some(history);

    let mut knowledge = context.knowledge.take().unwrap_or_default();
    merge_recent_artifacts(&mut knowledge, input.recent_tool_records);
    context.knowledge = Some(knowledge);

    if input.round_index > 1 {
        let tool_count = input
            .recent_tool_records
            .iter()
            .filter(|record| record.tool_name != "provider.call")
            .count();
        context.summary = Some(match context.summary.take() {
            Some(previous) if !previous.trim().is_empty() => format!(
                "{previous} | auto-tool follow-up round {} with {} executed tool result(s)",
                input.round_index, tool_count
            ),
            _ => format!(
                "auto-tool follow-up round {} with {} executed tool result(s)",
                input.round_index, tool_count
            ),
        });
        if let Some(previous_assistant_response) = input.previous_assistant_response {
            bounded_push(
                &mut context.continuity_tail,
                short_text(previous_assistant_response, 160),
                ROUND_CONTINUITY_WINDOW,
            );
        }
    }

    context.current_input = Some(CurrentInputBlock {
        input: input.current_input.to_string(),
        source: previous_current_input
            .as_ref()
            .filter(|_| input.round_index == 1)
            .map(|current| current.source.clone())
            .unwrap_or_else(|| "runtime.auto_tool_followup".into()),
        operation_id: input.operation_id.to_string(),
        trace_id: input.trace_id.to_string(),
        attachments: previous_current_input
            .filter(|_| input.round_index == 1)
            .map(|current| current.attachments)
            .unwrap_or_default(),
    });

    let tool_context = context.clone();
    context.tools = Some(build_dynamic_tool_catalog_block(&DynamicToolCatalogInput {
        role_id: Some(input.role_id),
        context: &tool_context,
        recent_tool_records: input.recent_tool_records,
        round_index: input.round_index,
    }));
    context
}

fn render_recent_tool_activity(records: &[ToolExecutionRecord]) -> Vec<String> {
    records
        .iter()
        .filter(|record| record.tool_name != "provider.call")
        .rev()
        .take(ROUND_RECENT_TOOL_WINDOW)
        .map(|record| {
            let outcome = record
                .output_summary
                .as_deref()
                .or(record.error_summary.as_deref())
                .unwrap_or(record.status.as_str());
            match record.target_ref.as_deref() {
                Some(target) if !target.trim().is_empty() => format!(
                    "{} {} -> {} ({})",
                    record.tool_name,
                    record.status,
                    target,
                    short_text(outcome, 160)
                ),
                _ => format!(
                    "{} {} -> {}",
                    record.tool_name,
                    record.status,
                    short_text(outcome, 180)
                ),
            }
        })
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect()
}

fn merge_recent_artifacts(knowledge: &mut KnowledgeArtifactBlock, records: &[ToolExecutionRecord]) {
    let mut merged = knowledge
        .artifact_candidates
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    for artifact in records
        .iter()
        .filter(|record| record.tool_name != "provider.call")
        .flat_map(|record| record.artifact_refs.iter().cloned())
    {
        merged.insert(artifact);
    }
    knowledge.artifact_candidates = merged
        .into_iter()
        .rev()
        .take(ROUND_ARTIFACT_WINDOW)
        .collect();
    knowledge.artifact_candidates.reverse();
}

fn bounded_push(items: &mut Vec<String>, value: String, window: usize) {
    if value.trim().is_empty() {
        return;
    }
    items.push(value);
    if items.len() > window {
        let overflow = items.len() - window;
        items.drain(0..overflow);
    }
}

fn short_text(value: &str, limit: usize) -> String {
    let trimmed = value.trim();
    if trimmed.chars().count() <= limit {
        trimmed.to_string()
    } else {
        let mut output = trimmed.chars().take(limit).collect::<String>();
        output.push('…');
        output
    }
}
