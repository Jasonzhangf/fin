use crate::ModelInputAssembler;
use fin_contracts::{
    HistoryBlock, MinimalContextView, ProjectContextBlock, RolePromptBlock, ToolCatalogBlock,
    ToolCatalogEntry,
};

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
                framework_tools: vec![ToolCatalogEntry {
                    tool_name: "provider.call".into(),
                    summary: "invoke provider".into(),
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
            ..Default::default()
        },
    );

    assert!(rendered.contains("Context summary"));
    assert!(rendered.contains("Continuity tail"));
    assert!(rendered.contains("Role prompt"));
    assert!(rendered.contains("Structured output contract"));
    assert!(rendered.contains("model_output_contract_v1"));
    assert!(rendered.contains("Framework capabilities"));
    assert!(rendered.contains("Tool selection policy"));
    assert!(rendered.contains("Recent history"));
    assert!(rendered.contains("Project scope"));
    assert!(rendered.contains("Current user input"));
    assert!(rendered.contains("Mandatory final answer format"));
    assert!(rendered.contains("Mandatory final answer example"));
    assert!(rendered.contains("0.98 -> 98"));
    assert!(rendered.contains("<fin_user_response>"));
    assert!(rendered.contains("<fin_control_feedback>"));
}
