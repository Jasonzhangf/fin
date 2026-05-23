use crate::{
    context_block_render::{render_peer_lines, render_project_lines, render_role_prompt_lines},
    prompt_assembly::{exact_control_feedback_schema_example, mandatory_response_format_lines},
};
use fin_contracts::{InputAttachmentSummary, MinimalContextView, ToolCatalogEntry};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContextStabilityClass {
    Immutable = 0,
    RarelyChanging = 1,
    SlowlyChanging = 2,
    AppendOnlyHistory = 3,
    VolatileTail = 4,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextAssemblySection {
    pub section_id: String,
    pub stability: ContextStabilityClass,
    pub title: String,
    pub body: String,
    pub section_hash: String,
    pub token_estimate: usize,
    pub source_artifact_refs: Vec<String>,
    pub included_reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextBudgetSnapshot {
    pub prompt_token_estimate: usize,
    pub compact_threshold_tokens: usize,
    pub should_compact: bool,
    pub trigger_reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextAssemblyPlan {
    pub sections: Vec<ContextAssemblySection>,
    pub budget: ContextBudgetSnapshot,
}

#[derive(Debug, Clone)]
pub struct ContextAssemblyPlanner {
    pub compact_threshold_tokens: usize,
}

impl Default for ContextAssemblyPlanner {
    fn default() -> Self {
        Self {
            compact_threshold_tokens: 120_000,
        }
    }
}

impl ContextAssemblyPlanner {
    pub fn build_plan(&self, input: &str, context: &MinimalContextView) -> ContextAssemblyPlan {
        let mut sections = Vec::new();
        push_role_prompt_sections(&mut sections, context);
        push_tool_catalog_sections(&mut sections, context);
        push_rare_context_sections(&mut sections, context);
        push_slow_context_sections(&mut sections, context);
        push_history_sections(&mut sections, context);
        push_current_input_sections(&mut sections, input, context);
        sections.sort_by_key(|section| section.stability);
        let prompt_token_estimate = sections.iter().map(|section| section.token_estimate).sum();
        let compact_threshold_tokens = self.compact_threshold_tokens.max(1);
        let should_compact = prompt_token_estimate >= compact_threshold_tokens;
        let trigger_reason = if should_compact {
            format!("estimated_prompt_tokens>={compact_threshold_tokens}")
        } else {
            "below_threshold".into()
        };
        ContextAssemblyPlan {
            sections,
            budget: ContextBudgetSnapshot {
                prompt_token_estimate,
                compact_threshold_tokens,
                should_compact,
                trigger_reason,
            },
        }
    }
}

fn push_role_prompt_sections(
    sections: &mut Vec<ContextAssemblySection>,
    context: &MinimalContextView,
) {
    if let Some(role_prompt) = &context.role_prompt {
        push_section(
            sections,
            "immutable.agent_prompt",
            ContextStabilityClass::Immutable,
            "Agent prompt",
            render_role_prompt_lines(role_prompt).join("\n"),
        );
        if !role_prompt.output_contract.is_empty() {
            push_section(
                sections,
                "immutable.structured_output_contract",
                ContextStabilityClass::Immutable,
                "Structured output contract",
                format!("- {}", role_prompt.output_contract.join("\n- ")),
            );
        }
        push_section(
            sections,
            "immutable.mandatory_final_answer_format",
            ContextStabilityClass::Immutable,
            "Mandatory final answer format",
            mandatory_response_format_lines().join("\n"),
        );
        push_section(
            sections,
            "immutable.mandatory_final_answer_example",
            ContextStabilityClass::Immutable,
            "Mandatory final answer example",
            exact_control_feedback_schema_example().to_string(),
        );
    }
}

fn push_tool_catalog_sections(
    sections: &mut Vec<ContextAssemblySection>,
    context: &MinimalContextView,
) {
    let Some(tools) = &context.tools else { return };
    let model_tools = tools
        .model_tools
        .iter()
        .map(render_tool_entry)
        .collect::<Vec<_>>();
    if !model_tools.is_empty() {
        push_section(
            sections,
            "immutable.model_tools",
            ContextStabilityClass::Immutable,
            "Model tools",
            format!("- {}", model_tools.join("\n- ")),
        );
    }
    let framework_tools = tools
        .framework_tools
        .iter()
        .map(render_tool_entry)
        .collect::<Vec<_>>();
    if !framework_tools.is_empty() {
        push_section(
            sections,
            "immutable.framework_capabilities",
            ContextStabilityClass::Immutable,
            "Framework capabilities",
            format!("- {}", framework_tools.join("\n- ")),
        );
    }
    if !tools.disabled_tools.is_empty() {
        push_section(
            sections,
            "immutable.disabled_tool_paths",
            ContextStabilityClass::Immutable,
            "Disabled tool paths",
            format!("- {}", tools.disabled_tools.join("\n- ")),
        );
    }
    if !tools.tool_selection_policy.is_empty() {
        push_section(
            sections,
            "immutable.tool_selection_policy",
            ContextStabilityClass::Immutable,
            "Tool selection policy",
            format!("- {}", tools.tool_selection_policy.join("\n- ")),
        );
    }
}

fn push_rare_context_sections(
    sections: &mut Vec<ContextAssemblySection>,
    context: &MinimalContextView,
) {
    if let Some(project) = &context.project {
        let lines = render_project_lines(project);
        if !lines.is_empty() {
            push_section(
                sections,
                "rare.project_scope",
                ContextStabilityClass::RarelyChanging,
                "Project scope",
                lines.join("\n"),
            );
        }
    }
    if let Some(peer) = &context.peer {
        let lines = render_peer_lines(peer);
        if !lines.is_empty() {
            push_section(
                sections,
                "rare.peer_scope",
                ContextStabilityClass::RarelyChanging,
                "Peer scope",
                lines.join("\n"),
            );
        }
    }
}

fn push_slow_context_sections(
    sections: &mut Vec<ContextAssemblySection>,
    context: &MinimalContextView,
) {
    if let Some(summary) = context
        .summary
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    {
        push_section(
            sections,
            "slow.context_summary",
            ContextStabilityClass::SlowlyChanging,
            "Context summary",
            summary.to_string(),
        );
    }
    if !context.continuity_tail.is_empty() {
        push_section(
            sections,
            "slow.continuity_tail",
            ContextStabilityClass::SlowlyChanging,
            "Continuity tail",
            format!("- {}", context.continuity_tail.join("\n- ")),
        );
    }
}

fn push_history_sections(sections: &mut Vec<ContextAssemblySection>, context: &MinimalContextView) {
    let Some(history) = &context.history else {
        return;
    };
    if !history.recent_messages.is_empty() {
        push_section(
            sections,
            "history.current_interaction_ledger",
            ContextStabilityClass::AppendOnlyHistory,
            "Current interaction ledger",
            format!("- {}", history.recent_messages.join("\n- ")),
        );
    }
    if !history.recent_reasoning.is_empty() {
        push_section(
            sections,
            "history.current_reasoning_history",
            ContextStabilityClass::AppendOnlyHistory,
            "Current reasoning history",
            format!("- {}", history.recent_reasoning.join("\n- ")),
        );
    }
    if !history.recent_tool_activity.is_empty() {
        push_section(
            sections,
            "history.current_tool_execution_history",
            ContextStabilityClass::AppendOnlyHistory,
            "Current tool execution history",
            format!("- {}", history.recent_tool_activity.join("\n- ")),
        );
    }
}

fn push_current_input_sections(
    sections: &mut Vec<ContextAssemblySection>,
    input: &str,
    context: &MinimalContextView,
) {
    if let Some(current_input) = &context.current_input {
        push_section(
            sections,
            "tail.request_envelope",
            ContextStabilityClass::VolatileTail,
            "Request envelope",
            format!(
                "source={}\noperation_id={}\ntrace_id={}",
                current_input.source, current_input.operation_id, current_input.trace_id
            ),
        );
        let lines = render_attachments(current_input.attachments.as_slice());
        if !lines.is_empty() {
            push_section(
                sections,
                "tail.input_attachments",
                ContextStabilityClass::VolatileTail,
                "Input attachments",
                format!("- {}", lines.join("\n- ")),
            );
        }
    }
    push_section(
        sections,
        "tail.current_request",
        ContextStabilityClass::VolatileTail,
        "Current request",
        input.to_string(),
    );
}

fn push_section(
    sections: &mut Vec<ContextAssemblySection>,
    section_id: &str,
    stability: ContextStabilityClass,
    title: &str,
    body: String,
) {
    if body.trim().is_empty() {
        return;
    }
    sections.push(ContextAssemblySection {
        section_id: section_id.into(),
        stability,
        title: title.into(),
        section_hash: stable_hash(&body),
        token_estimate: estimate_tokens(&body),
        source_artifact_refs: source_artifact_refs(section_id, &body),
        included_reason: format!("non_empty_{section_id}"),
        body,
    });
}

fn stable_hash(value: &str) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut hasher = DefaultHasher::new();
    value.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

fn source_artifact_refs(section_id: &str, body: &str) -> Vec<String> {
    if !section_id.contains("artifact") && !body.contains("images/") && !body.contains("artifact") {
        return Vec::new();
    }
    body.split_whitespace()
        .map(|part| part.trim_matches(|ch: char| matches!(ch, ',' | ';' | ')' | '(')))
        .filter(|part| part.contains("images/") || part.contains("artifact"))
        .map(str::to_string)
        .collect()
}

fn estimate_tokens(value: &str) -> usize {
    (value.chars().count() / 4).max(1)
}

fn render_attachments(items: &[InputAttachmentSummary]) -> Vec<String> {
    items
        .iter()
        .map(|item| {
            let mut parts = vec![item.name.clone().unwrap_or_else(|| "attachment".into())];
            if !item.kind.trim().is_empty() {
                parts.push(format!("kind={}", item.kind));
            }
            if let Some(size_bytes) = item.size_bytes {
                parts.push(format!("size={}B", size_bytes));
            }
            if let (Some(width), Some(height)) = (item.width, item.height) {
                parts.push(format!("dimensions={}x{}", width, height));
            }
            if let Some(url) = &item.url {
                parts.push(format!("url={url}"));
            }
            if let Some(path) = &item.local_path {
                parts.push(format!("path={path}"));
            }
            parts.join(" | ")
        })
        .collect()
}

fn render_tool_entry(tool: &ToolCatalogEntry) -> String {
    let mut lines = vec![format!("{} — {}", tool.tool_name, tool.summary)];
    if !tool.when_to_use.is_empty() {
        lines.push(format!("use: {}", tool.when_to_use.join(" | ")));
    }
    if !tool.when_not_to_use.is_empty() {
        lines.push(format!("avoid: {}", tool.when_not_to_use.join(" | ")));
    }
    if !tool.input_schema_summary.trim().is_empty() {
        lines.push(format!("input: {}", tool.input_schema_summary));
    }
    if !tool.output_schema_summary.trim().is_empty() {
        lines.push(format!("output: {}", tool.output_schema_summary));
    }
    for example in &tool.example_uses {
        lines.push(format!("example: {example}"));
    }
    lines.join("; ")
}
