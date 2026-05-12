use super::*;

#[test]
fn context_tool_catalog_exposes_rich_apply_patch_and_query_tool_metadata() {
    let worker = worker_runtime();
    let context = ContextViewBuilder.build(&worker, ContextAssemblyInput::default());
    let tools = context.tools.expect("tool catalog");

    let apply_patch = tools
        .model_tools
        .iter()
        .find(|tool| tool.tool_name == "apply_patch")
        .expect("apply_patch tool");
    assert!(
        apply_patch
            .when_not_to_use
            .iter()
            .any(|item| item.contains("still exploring"))
    );
    assert!(apply_patch.input_schema_summary.contains("mode=replace"));
    assert!(apply_patch.input_schema_summary.contains("mode=patch"));
    assert!(apply_patch.input_schema_summary.contains("old_string=\"\""));
    assert!(
        apply_patch
            .example_uses
            .iter()
            .any(|item| item.contains("exact function body"))
    );
    assert!(
        apply_patch
            .example_uses
            .iter()
            .any(|item| item.contains("create a new file"))
    );

    for tool_name in [
        "view_image",
        "context_history.rebuild",
        "project.task.status",
        "project.task.list",
        "project.task.create",
        "project.task.claim",
        "project.task.submit",
        "project.task.review",
        "agent.presence.list",
        "project.supervision.list",
    ] {
        let tool = tools
            .model_tools
            .iter()
            .find(|item| item.tool_name == tool_name)
            .unwrap_or_else(|| panic!("missing tool {tool_name}"));
        assert!(
            !tool.when_to_use.is_empty(),
            "tool {} should expose when_to_use guidance",
            tool_name
        );
        assert!(
            !tool.output_schema_summary.trim().is_empty(),
            "tool {} should expose output schema summary",
            tool_name
        );
        assert!(
            !tool.example_uses.is_empty(),
            "tool {} should expose example uses",
            tool_name
        );
    }
}

#[test]
fn dynamic_tool_catalog_marks_runtime_and_peer_dependent_tools_when_context_is_missing() {
    let tools = crate::tool_catalog_dynamic::build_dynamic_tool_catalog_block(
        &crate::tool_catalog_dynamic::DynamicToolCatalogInput {
            role_id: None,
            context: &MinimalContextView::default(),
            recent_tool_records: &Vec::<ToolExecutionRecord>::new(),
            round_index: 1,
        },
    );

    for (tool_name, expected) in [
        ("write_stdin", "no exec replay session"),
        ("project.task.status", "no runtime_home"),
        ("project.task.create", "no runtime_home"),
        ("agent.presence.list", "no runtime_home"),
        ("project.supervision.list", "no runtime_home"),
        ("capability.invoke", "no capability ids"),
    ] {
        let tool = tools
            .model_tools
            .iter()
            .find(|tool| tool.tool_name == tool_name)
            .unwrap_or_else(|| panic!("missing tool {tool_name}"));
        assert!(
            tool.when_not_to_use
                .iter()
                .any(|item| item.contains(expected)),
            "missing when_not_to_use hint for {tool_name}"
        );
    }
}

#[test]
fn dynamic_tool_catalog_promotes_write_stdin_when_exec_session_exists() {
    let worker = worker_runtime();
    let runtime_home = std::env::temp_dir().join("fin-runtime-dynamic-tool-catalog");
    fs::create_dir_all(runtime_home.join("runtime/tools/exec_sessions")).expect("exec session dir");
    fs::write(
        runtime_home.join("runtime/tools/exec_sessions/sample.json"),
        br#"{"session_id":"sample"}"#,
    )
    .expect("exec session seed");

    let context = ContextViewBuilder.build(
        &worker,
        ContextAssemblyInput {
            runtime_home: Some(runtime_home.display().to_string()),
            ..ContextAssemblyInput::default()
        },
    );
    let tools = context.tools.expect("tool catalog");
    let write_stdin = tools
        .model_tools
        .iter()
        .find(|tool| tool.tool_name == "write_stdin")
        .expect("write_stdin tool");

    assert!(
        write_stdin
            .when_to_use
            .iter()
            .any(|item| item.contains("exec replay session is available"))
    );
    assert!(
        tools
            .tool_selection_policy
            .iter()
            .any(|item| item.contains("write_stdin is actionable"))
    );
}
