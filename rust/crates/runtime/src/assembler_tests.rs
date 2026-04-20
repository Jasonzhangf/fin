use crate::ModelInputAssembler;
use fin_contracts::{
    CurrentInputBlock, DaemonStateSummary, HistoryBlock, InputAttachmentSummary,
    MinimalContextView, PeerBindingSummary, PeerContextBlock, PeerDescriptorSummary,
    ProjectContextBlock, RolePromptBlock, ToolCatalogBlock, ToolCatalogEntry,
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
                model_tools: vec![ToolCatalogEntry {
                    tool_name: "peer.list".into(),
                    kind: "agent_tool".into(),
                    summary: "list peers".into(),
                    when_to_use: vec!["need peer topology".into()],
                    when_not_to_use: vec!["topology is already known".into()],
                    input_schema_summary: "optional limit".into(),
                    output_schema_summary: "peer list".into(),
                    example_uses: vec!["list peers before choosing a route target".into()],
                    ..Default::default()
                }],
                framework_tools: vec![ToolCatalogEntry {
                    tool_name: "provider.call".into(),
                    summary: "invoke provider".into(),
                    input_schema_summary: "compiled prompt".into(),
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
            peer: Some(PeerContextBlock {
                topology_summary: Some("local-only placeholder".into()),
                active_peer_ids: vec!["local-worker-1".into()],
                peers: vec![PeerDescriptorSummary {
                    peer_id: "local-worker-1".into(),
                    label: "local worker".into(),
                    peer_kind: "agent".into(),
                    presence_state: "local_only".into(),
                    health_state: Some("unknown".into()),
                    capability_ids: vec!["peer.list".into()],
                    supports_session_binding: true,
                    supports_agentic_execution: true,
                }],
                binding: Some(PeerBindingSummary {
                    binding_state: Some("local_execution_only".into()),
                    ..Default::default()
                }),
                daemon: Some(DaemonStateSummary {
                    supervision_state: Some("not_attached".into()),
                    ..Default::default()
                }),
                ..Default::default()
            }),
            current_input: Some(CurrentInputBlock {
                input: "current ask".into(),
                source: "channel.qqbot".into(),
                operation_id: "op-1".into(),
                trace_id: "trace-1".into(),
                attachments: vec![InputAttachmentSummary {
                    kind: "image/png".into(),
                    name: Some("demo.png".into()),
                    url: Some("https://example.com/demo.png".into()),
                    width: Some(100),
                    height: Some(50),
                    ..Default::default()
                }],
            }),
            ..Default::default()
        },
    );

    assert!(rendered.contains("Context summary"));
    assert!(rendered.contains("Continuity tail"));
    assert!(rendered.contains("Role prompt"));
    assert!(rendered.contains("Structured output contract"));
    assert!(rendered.contains("model_output_contract_v1"));
    assert!(rendered.contains("Model tools"));
    assert!(rendered.contains("peer.list"));
    assert!(rendered.contains("use: need peer topology"));
    assert!(rendered.contains("avoid: topology is already known"));
    assert!(rendered.contains("input: optional limit"));
    assert!(rendered.contains("output: peer list"));
    assert!(rendered.contains("example: list peers before choosing a route target"));
    assert!(rendered.contains("Framework capabilities"));
    assert!(rendered.contains("Tool selection policy"));
    assert!(rendered.contains("Recent history"));
    assert!(rendered.contains("Project scope"));
    assert!(rendered.contains("Peer scope"));
    assert!(rendered.contains("Current user input"));
    assert!(rendered.contains("Input attachments"));
    assert!(rendered.contains("demo.png"));
    assert!(rendered.contains("Mandatory final answer format"));
    assert!(rendered.contains("Mandatory final answer example"));
    assert!(rendered.contains("0.98 -> 98"));
    assert!(rendered.contains("<fin_user_response>"));
    assert!(rendered.contains("<fin_control_feedback>"));
}

#[test]
fn model_input_assembler_renders_apply_patch_guidance_verbatim_in_tool_catalog() {
    let rendered = ModelInputAssembler::default().assemble(
        "patch the file",
        &MinimalContextView {
            role_prompt: Some(RolePromptBlock {
                role_id: "project".into(),
                current_prompt_summary: "summary".into(),
                output_contract: vec!["must use fin blocks".into()],
                ..Default::default()
            }),
            tools: Some(ToolCatalogBlock {
                model_tools: vec![ToolCatalogEntry {
                    tool_name: "apply_patch".into(),
                    kind: "model_tool".into(),
                    summary: "apply deterministic workspace file edits".into(),
                    when_to_use: vec![
                        "you need to modify files deterministically instead of only describing edits"
                            .into(),
                    ],
                    when_not_to_use: vec![
                        "you are still exploring and do not know the concrete change yet".into(),
                    ],
                    input_schema_summary:
                        "mode=replace: path + old_string + new_string + replace_all? ; mode=patch: patch"
                            .into(),
                    output_schema_summary: "patch receipt + modified file refs".into(),
                    example_uses: vec![
                        "replace one exact function body in src/runtime.rs".into(),
                        "apply a multi-file V4A patch for a small deterministic refactor".into(),
                    ],
                    ..Default::default()
                }],
                ..Default::default()
            }),
            ..Default::default()
        },
    );

    assert!(rendered.contains("apply_patch"));
    assert!(rendered.contains("mode=replace"));
    assert!(rendered.contains("replace_all?"));
    assert!(rendered.contains("mode=patch"));
    assert!(rendered.contains("patch receipt + modified file refs"));
    assert!(rendered.contains("replace one exact function body in src/runtime.rs"));
    assert!(rendered.contains("multi-file V4A patch"));
}
