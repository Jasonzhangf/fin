use fin_contracts::{DigestRecord, ToolExecutionRecord};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompactionInput {
    pub session_id: String,
    pub task_id: Option<String>,
    pub trigger_reason: String,
    pub recent_messages: Vec<String>,
    pub digest_records: Vec<DigestRecord>,
    pub tool_records: Vec<ToolExecutionRecord>,
    pub retain_recent_count: usize,
    pub compacted_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompactedHistoryRecord {
    pub session_id: String,
    pub task_id: Option<String>,
    pub compacted_at: String,
    pub trigger_reason: String,
    pub summary: String,
    pub retained_messages: Vec<String>,
    pub retained_artifact_refs: Vec<String>,
    pub retained_tool_refs: Vec<String>,
    pub replaced_message_count: usize,
}

#[derive(Debug, Clone, Default)]
pub struct ContextCompactionEngine;

impl ContextCompactionEngine {
    pub fn compact(&self, input: CompactionInput) -> CompactedHistoryRecord {
        let retain_recent_count = input.retain_recent_count.max(1);
        let total_messages = input.recent_messages.len();
        let split_at = total_messages.saturating_sub(retain_recent_count);
        let retained_messages = input.recent_messages[split_at..].to_vec();
        let replaced_message_count = split_at;
        let mut summary_lines = Vec::new();
        for message in input.recent_messages.iter().take(split_at) {
            summary_lines.push(format!("message: {message}"));
        }
        for digest in &input.digest_records {
            if !digest.summary.trim().is_empty() {
                summary_lines.push(format!("digest: {}", digest.summary));
            }
            for tail in &digest.continuity_tail {
                summary_lines.push(format!("continuity: {tail}"));
            }
        }
        let retained_artifact_refs = input
            .digest_records
            .iter()
            .flat_map(|digest| digest.artifact_candidates.iter().cloned())
            .chain(
                input
                    .tool_records
                    .iter()
                    .flat_map(|tool| tool.artifact_refs.iter().cloned()),
            )
            .filter(|value| !value.trim().is_empty())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        let retained_tool_refs = input
            .tool_records
            .iter()
            .map(|tool| tool.tool_call_id.clone())
            .filter(|value| !value.trim().is_empty())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        CompactedHistoryRecord {
            session_id: input.session_id,
            task_id: input.task_id,
            compacted_at: input.compacted_at,
            trigger_reason: input.trigger_reason,
            summary: summary_lines.join("\n"),
            retained_messages,
            retained_artifact_refs,
            retained_tool_refs,
            replaced_message_count,
        }
    }
}
