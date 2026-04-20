use crate::{
    context_blocks::{render_peer_lines, render_project_lines, render_role_prompt_lines},
    prompt_assembly::{exact_control_feedback_schema_example, mandatory_response_format_lines},
};
use fin_contracts::{InputAttachmentSummary, MinimalContextView, ToolCatalogEntry};

#[derive(Debug, Clone, Default)]
pub struct ModelInputAssembler;

impl ModelInputAssembler {
    pub fn assemble(&self, input: &str, context: &MinimalContextView) -> String {
        let mut sections = Vec::new();

        if let Some(role_prompt) = &context.role_prompt {
            sections.push(format!(
                "Agent prompt:\n{}",
                render_role_prompt_lines(role_prompt).join("\n")
            ));
            if !role_prompt.output_contract.is_empty() {
                sections.push(format!(
                    "Structured output contract:\n- {}",
                    role_prompt.output_contract.join("\n- ")
                ));
            }
        }
        if let Some(summary) = context
            .summary
            .as_deref()
            .filter(|value| !value.trim().is_empty())
        {
            sections.push(format!("Context summary:\n{summary}"));
        }
        if !context.continuity_tail.is_empty() {
            sections.push(format!(
                "Continuity tail:\n- {}",
                context.continuity_tail.join("\n- ")
            ));
        }
        if let Some(tools) = &context.tools {
            let model_tools = tools
                .model_tools
                .iter()
                .map(render_tool_entry)
                .collect::<Vec<_>>();
            if !model_tools.is_empty() {
                sections.push(format!("Model tools:\n- {}", model_tools.join("\n- ")));
            }
            let framework_tools = tools
                .framework_tools
                .iter()
                .map(render_tool_entry)
                .collect::<Vec<_>>();
            if !framework_tools.is_empty() {
                sections.push(format!(
                    "Framework capabilities:\n- {}",
                    framework_tools.join("\n- ")
                ));
            }
            if !tools.disabled_tools.is_empty() {
                sections.push(format!(
                    "Disabled tool paths:\n- {}",
                    tools.disabled_tools.join("\n- ")
                ));
            }
            if !tools.tool_selection_policy.is_empty() {
                sections.push(format!(
                    "Tool selection policy:\n- {}",
                    tools.tool_selection_policy.join("\n- ")
                ));
            }
        }
        if let Some(project) = &context.project {
            let lines = render_project_lines(project);
            if !lines.is_empty() {
                sections.push(format!("Project scope:\n{}", lines.join("\n")));
            }
        }
        if let Some(peer) = &context.peer {
            let lines = render_peer_lines(peer);
            if !lines.is_empty() {
                sections.push(format!("Peer scope:\n{}", lines.join("\n")));
            }
        }
        if let Some(history) = &context.history {
            if !history.recent_messages.is_empty() {
                sections.push(format!(
                    "Recent interaction ledger:\n- {}",
                    history.recent_messages.join("\n- ")
                ));
            }
            if !history.recent_reasoning.is_empty() {
                sections.push(format!(
                    "Recent reasoning summaries:\n- {}",
                    history.recent_reasoning.join("\n- ")
                ));
            }
            if !history.recent_tool_activity.is_empty() {
                sections.push(format!(
                    "Recent tool activity:\n- {}",
                    history.recent_tool_activity.join("\n- ")
                ));
            }
        }
        sections.push(format!("Current request:\n{input}"));
        if let Some(current_input) = &context.current_input {
            sections.push(format!(
                "Request envelope:\nsource={}\noperation_id={}\ntrace_id={}",
                current_input.source, current_input.operation_id, current_input.trace_id,
            ));
            let lines = render_attachments(current_input.attachments.as_slice());
            if !lines.is_empty() {
                sections.push(format!("Input attachments:\n- {}", lines.join("\n- ")));
            }
        }
        if context.role_prompt.is_some() {
            sections.push(format!(
                "Mandatory final answer format:\n- {}",
                mandatory_response_format_lines().join("\n- ")
            ));
            sections.push(format!(
                "Mandatory final answer example:\n<fin_user_response>\nYOUR_DIRECT_ANSWER\n</fin_user_response>\n<fin_control_feedback>{}</fin_control_feedback>",
                exact_control_feedback_schema_example()
            ));
        }

        sections.join("\n\n")
    }
}

fn render_attachments(items: &[InputAttachmentSummary]) -> Vec<String> {
    items
        .iter()
        .map(|item| {
            let mut parts = Vec::new();
            let label = item
                .name
                .as_deref()
                .or(item.content_type.as_deref())
                .or(item.url.as_deref())
                .unwrap_or("attachment");
            parts.push(label.to_string());
            if let Some(kind) = (!item.kind.trim().is_empty()).then_some(item.kind.as_str()) {
                parts.push(format!("kind={kind}"));
            }
            if let Some(size_bytes) = item.size_bytes {
                parts.push(format!("size={}B", size_bytes));
            }
            if let (Some(width), Some(height)) = (item.width, item.height) {
                parts.push(format!("dimensions={}x{}", width, height));
            }
            if let Some(url) = item.url.as_deref() {
                parts.push(format!("url={url}"));
            }
            if let Some(path) = item.local_path.as_deref() {
                parts.push(format!("path={path}"));
            }
            parts.join(" · ")
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
    if !tool.example_uses.is_empty() {
        lines.push(format!("example: {}", tool.example_uses.join(" | ")));
    }
    lines.join("\n  ")
}
