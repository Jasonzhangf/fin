use super::*;

use fin_config::{ConfigMapper, ProviderProtocol, UserConfig, UserProviderConfig};
use std::collections::BTreeMap;

fn worker_runtime() -> WorkerRuntime {
    let user = UserConfig {
        default_provider: "openai".into(),
        providers: BTreeMap::from([(
            "openai".into(),
            UserProviderConfig {
                protocol: ProviderProtocol::OpenAiCompatible,
                base_url: "https://api.example.com/v1".into(),
                model: "gpt-5".into(),
                api_key: None,
                api_key_env: Some("OPENAI_API_KEY".into()),
                user_agent: None,
                headers: BTreeMap::new(),
            },
        )]),
    };
    let system = ConfigMapper::map_user_to_system(&user).expect("mapping should succeed");
    WorkerRuntime::from_system(&system, "agent-1", "worker-1", "runtime", None)
        .expect("worker runtime")
}

#[test]
fn context_view_builder_exposes_loaded_global_skills() {
    let worker = worker_runtime();
    let home = std::env::temp_dir().join("fin-runtime-global-skills-test");
    std::fs::create_dir_all(home.join("skills/demo-skill")).expect("skill dir should create");
    std::fs::write(
        home.join("skills/demo-skill/SKILL.md"),
        "---
name: demo-skill
description: Demo skill for prompt loading.
---
",
    )
    .expect("skill file should write");

    let context = ContextViewBuilder.build(
        &worker,
        ContextAssemblyInput {
            runtime_home: Some(home.display().to_string()),
            ..ContextAssemblyInput::default()
        },
    );

    assert!(context
        .role_prompt
        .as_ref()
        .map(|value| value
            .prompt_modules
            .iter()
            .any(|item| item.module_id == "stable_core.loaded_global_skill_index"))
        .unwrap_or(false));
    assert!(context
        .role_prompt
        .as_ref()
        .map(|value| value
            .prompt_lineage
            .iter()
            .any(|item| item.contains("demo-skill")))
        .unwrap_or(false));
}

#[test]
fn gpt_model_receives_gpt_codex_overlay_modules() {
    let worker = worker_runtime();
    let context = ContextViewBuilder.build(&worker, ContextAssemblyInput::default());

    assert!(context
        .role_prompt
        .as_ref()
        .map(|value| value
            .prompt_modules
            .iter()
            .any(|item| item.module_id == "overlay.gpt_codex.tool_persistence"))
        .unwrap_or(false));
}

#[test]
fn prompt_blocks_expose_wait_stop_and_apply_patch_rules() {
    let worker = worker_runtime();
    let context = ContextViewBuilder.build(&worker, ContextAssemblyInput::default());
    let role_prompt = context.role_prompt.expect("role prompt");

    assert!(role_prompt
        .behavior_rules
        .iter()
        .any(|item| item.contains("wait.remind") && item.contains("1 minute")));
    assert!(role_prompt
        .behavior_rules
        .iter()
        .any(|item| item.contains("reasoning.stop")));
    assert!(role_prompt
        .behavior_rules
        .iter()
        .any(|item| item.contains("apply_patch replace mode")));
    assert!(role_prompt
        .output_contract
        .iter()
        .any(|item| item.contains("reasoning.stop")));
    assert!(role_prompt.output_contract.iter().any(|item| {
        item.contains("apply_patch")
            && item.contains("{path, old_string, new_string}")
            && item.contains("mode=patch")
    }));
    assert!(role_prompt
        .prompt_layers
        .iter()
        .any(|layer| layer.layer_id == "stable_core"));
    assert!(role_prompt
        .prompt_layers
        .iter()
        .any(|layer| layer.layer_id == "role_baseline"));
}

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
    assert!(apply_patch
        .when_not_to_use
        .iter()
        .any(|item| item.contains("still exploring")));
    assert!(apply_patch.input_schema_summary.contains("mode=replace"));
    assert!(apply_patch.input_schema_summary.contains("mode=patch"));
    assert!(apply_patch
        .example_uses
        .iter()
        .any(|item| item.contains("exact function body")));

    for tool_name in [
        "view_image",
        "context_history.rebuild",
        "project.task.status",
        "project.task.list",
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
