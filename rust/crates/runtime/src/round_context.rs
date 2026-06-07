use crate::tools::catalog_dynamic::{DynamicToolCatalogInput, build_dynamic_catalog_block};
use crate::tools::history_render::render_current_tool_execution_history;
use fin_contracts::{
    CurrentInputBlock, KnowledgeArtifactBlock, MinimalContextView, ToolExecutionRecord,
};
use std::collections::BTreeSet;

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
        history.recent_messages.push(format!(
            "assistant(previous round): {}",
            previous_assistant_response.trim()
        ));
    }
    history.recent_tool_activity =
        render_current_tool_execution_history(&context, input.recent_tool_records);
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
            context
                .continuity_tail
                .push(previous_assistant_response.trim().to_string());
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
    context.tools = Some(build_dynamic_catalog_block(&DynamicToolCatalogInput {
        role_id: Some(input.role_id),
        context: &tool_context,
        recent_tool_records: input.recent_tool_records,
        round_index: input.round_index,
    }));
    context
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
    knowledge.artifact_candidates = merged.into_iter().collect();
}
