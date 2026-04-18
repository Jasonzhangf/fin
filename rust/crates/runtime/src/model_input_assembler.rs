use crate::{
    context_blocks::{render_project_lines, render_role_prompt_lines},
    prompt_assembly::{exact_control_feedback_schema_example, mandatory_response_format_lines},
};
use fin_contracts::MinimalContextView;

#[derive(Debug, Clone, Default)]
pub struct ModelInputAssembler;

impl ModelInputAssembler {
    pub fn assemble(&self, input: &str, context: &MinimalContextView) -> String {
        let mut sections = Vec::new();

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
        if let Some(role_prompt) = &context.role_prompt {
            sections.push(format!(
                "Role prompt:\n{}",
                render_role_prompt_lines(role_prompt).join("\n")
            ));
            if !role_prompt.output_contract.is_empty() {
                sections.push(format!(
                    "Structured output contract:\n- {}",
                    role_prompt.output_contract.join("\n- ")
                ));
            }
        }
        if let Some(tools) = &context.tools {
            let framework_tools = tools
                .framework_tools
                .iter()
                .map(|tool| format!("{} ({})", tool.tool_name, tool.summary))
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
        if let Some(history) = &context.history {
            if !history.recent_messages.is_empty() {
                sections.push(format!(
                    "Recent history:\n- {}",
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
        if let Some(project) = &context.project {
            let lines = render_project_lines(project);
            if !lines.is_empty() {
                sections.push(format!("Project scope:\n{}", lines.join("\n")));
            }
        }
        sections.push(format!("Current user input:\n{input}"));
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
