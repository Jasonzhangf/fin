use crate::{
    WorkerRuntime,
    context_blocks::{build_peer_block, build_project_block},
    prompt_assembly::build_role_prompt_block,
    tool_catalog_dynamic::{DynamicToolCatalogInput, build_dynamic_tool_catalog_block},
};
use fin_contracts::{
    ContextControlBlock, CurrentInputBlock, DigestRecord, EntityRefs, HistoryBlock,
    InputAttachmentSummary, KnowledgeArtifactBlock, MinimalContextView, ReasoningViewRecord,
    ToolExecutionRecord,
};
use std::collections::BTreeSet;

const RECENT_HISTORY_WINDOW: usize = 6;
const RECENT_DIGEST_WINDOW: usize = 3;
const RECENT_REASONING_WINDOW: usize = 3;
const RECENT_TOOL_WINDOW: usize = 4;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ContextAssemblyInput {
    pub operation_id: String,
    pub trace_id: String,
    pub refs: EntityRefs,
    pub input: String,
    pub source: String,
    pub recent_messages: Vec<String>,
    pub recent_digests: Vec<DigestRecord>,
    pub recent_reasoning_views: Vec<ReasoningViewRecord>,
    pub recent_tool_records: Vec<ToolExecutionRecord>,
    pub project_label: Option<String>,
    pub runtime_home: Option<String>,
    pub cwd: Option<String>,
    pub selected_paths: Vec<String>,
    pub attachment_summaries: Vec<InputAttachmentSummary>,
}

#[derive(Debug, Clone, Default)]
pub struct ContextViewBuilder;

impl ContextViewBuilder {
    pub fn build(&self, worker: &WorkerRuntime, input: ContextAssemblyInput) -> MinimalContextView {
        let recent_digests = input
            .recent_digests
            .iter()
            .rev()
            .take(RECENT_DIGEST_WINDOW)
            .cloned()
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect::<Vec<_>>();
        let recent_messages = input
            .recent_messages
            .iter()
            .rev()
            .take(RECENT_HISTORY_WINDOW)
            .cloned()
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect::<Vec<_>>();
        let recent_reasoning = input
            .recent_reasoning_views
            .iter()
            .rev()
            .take(RECENT_REASONING_WINDOW)
            .map(|item| item.summary.clone())
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect::<Vec<_>>();
        let recent_tool_activity = input
            .recent_tool_records
            .iter()
            .rev()
            .take(RECENT_TOOL_WINDOW)
            .map(|item| {
                format!(
                    "{} {} -> {}",
                    item.tool_name,
                    item.status,
                    item.target_ref.as_deref().unwrap_or("unknown target")
                )
            })
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect::<Vec<_>>();

        let mut continuity_tail = Vec::new();
        for digest in &recent_digests {
            continuity_tail.extend(digest.continuity_tail.iter().cloned());
        }
        let summary = recent_digests.last().map(|digest| digest.summary.clone());
        let digest_summaries = recent_digests
            .iter()
            .map(|digest| digest.summary.clone())
            .collect::<Vec<_>>();
        let artifact_candidates = recent_digests
            .iter()
            .flat_map(|digest| digest.artifact_candidates.iter().cloned())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();

        let mut context = MinimalContextView {
            continuity_tail,
            summary,
            control: Some(ContextControlBlock {
                session_id: input.refs.session_id.clone(),
                task_id: input.refs.task_id.clone(),
                topic_thread_id: input.refs.topic_thread_id.clone(),
                dispatch_id: input.refs.dispatch_id.clone(),
                worker_id: Some(worker.worker_id.clone()),
                operation_id: Some(input.operation_id.clone()),
                trace_id: Some(input.trace_id.clone()),
                protocol_version: Some(worker.policy.protocol_version.clone()),
                provider_strategy: Some(worker.policy.provider_strategy),
                stream: Some(worker.policy.stream),
            }),
            role_prompt: Some(build_role_prompt_block(worker, &recent_digests, &input)),
            tools: None,
            history: Some(HistoryBlock {
                recent_messages,
                recent_digests: digest_summaries.clone(),
                recent_reasoning,
                recent_tool_activity,
            }),
            knowledge: Some(KnowledgeArtifactBlock {
                digest_summaries,
                artifact_candidates,
            }),
            project: Some(build_project_block(worker, &input)),
            peer: Some(build_peer_block(worker, &input)),
            current_input: Some(CurrentInputBlock {
                input: input.input,
                source: input.source,
                operation_id: input.operation_id,
                trace_id: input.trace_id,
                attachments: input.attachment_summaries,
            }),
        };
        let tool_context = context.clone();
        context.tools = Some(build_dynamic_tool_catalog_block(&DynamicToolCatalogInput {
            role_id: Some(worker.policy.role.role_id.as_str()),
            context: &tool_context,
            recent_tool_records: &input.recent_tool_records,
            round_index: 1,
        }));
        context
    }
}
