use super::*;

use fin_config::{ConfigMapper, ProviderProtocol, UserConfig, UserProviderConfig};
use fin_contracts::{MinimalContextView, ToolExecutionRecord};
use std::{collections::BTreeMap, fs};

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
        runtime: fin_config::UserRuntimeConfig::default(),
    };
    let system = ConfigMapper::map_user_to_system(&user).expect("mapping should succeed");
    WorkerRuntime::from_system(&system, "agent-1", "worker-1", "runtime", None)
        .expect("worker runtime")
}

fn system_worker_runtime() -> WorkerRuntime {
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
        runtime: fin_config::UserRuntimeConfig::default(),
    };
    let system = ConfigMapper::map_user_to_system(&user).expect("mapping should succeed");
    WorkerRuntime::from_system(&system, "agent-system", "worker-system", "runtime", Some("system"))
        .expect("system worker runtime")
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

    assert!(
        context
            .role_prompt
            .as_ref()
            .map(|value| value
                .prompt_modules
                .iter()
                .any(|item| item.module_id == "stable_core.loaded_global_skill_index"))
            .unwrap_or(false)
    );
    assert!(
        context
            .role_prompt
            .as_ref()
            .map(|value| value
                .prompt_lineage
                .iter()
                .any(|item| item.contains("demo-skill")))
            .unwrap_or(false)
    );
}

#[test]
fn role_prompt_is_agent_first_and_hides_provider_model_identity() {
    let worker = worker_runtime();
    let context = ContextViewBuilder.build(&worker, ContextAssemblyInput::default());
    let role_prompt = context.role_prompt.expect("role prompt");

    assert!(
        role_prompt
            .prompt_modules
            .iter()
            .any(|item| item.module_id == "stable_core.request_framing")
    );
    assert!(
        role_prompt
            .prompt_lineage
            .iter()
            .any(|item| item.contains("framework-routed request"))
    );
    assert!(
        !role_prompt
            .prompt_lineage
            .iter()
            .any(|item| item.contains("provider path:"))
    );
    assert!(
        !role_prompt
            .prompt_layers
            .iter()
            .any(|item| item.layer_id == "model_overlay")
    );
}

#[test]
fn prompt_blocks_expose_wait_stop_and_apply_patch_rules() {
    let project_worker = worker_runtime();
    let project_context = ContextViewBuilder.build(&project_worker, ContextAssemblyInput::default());
    let role_prompt = project_context.role_prompt.expect("role prompt");

    assert!(
        role_prompt
            .behavior_rules
            .iter()
            .any(|item| item.contains("wait.remind") && item.contains("1 minute"))
    );
    assert!(
        role_prompt
            .behavior_rules
            .iter()
            .any(|item| item.contains("reasoning.stop"))
    );
    assert!(
        role_prompt
            .behavior_rules
            .iter()
            .any(|item| item.contains("apply_patch replace mode"))
    );
    assert!(
        role_prompt
            .output_contract
            .iter()
            .any(|item| item.contains("reasoning.stop"))
    );
    assert!(role_prompt.output_contract.iter().any(|item| {
        item.contains("apply_patch")
            && item.contains("{path, old_string, new_string}")
            && item.contains("mode=patch")
    }));
    assert!(
        role_prompt
            .prompt_layers
            .iter()
            .any(|layer| layer.layer_id == "stable_core")
    );
    assert!(
        role_prompt
            .prompt_layers
            .iter()
            .any(|layer| layer.layer_id == "role_baseline")
    );

    let system_worker = system_worker_runtime();
    let system_context = ContextViewBuilder.build(&system_worker, ContextAssemblyInput::default());
    let system_role_prompt = system_context.role_prompt.expect("system role prompt");
    assert!(system_role_prompt.prompt_modules.iter().any(|item| {
        item.module_id == "role.system.framework_state_first"
    }));
    assert!(system_role_prompt.behavior_rules.iter().any(|item| {
        item.contains("framework-owned task board")
            && item.contains("agent presence")
            && item.contains("supervision")
    }));
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
    assert!(
        apply_patch
            .when_not_to_use
            .iter()
            .any(|item| item.contains("still exploring"))
    );
    assert!(apply_patch.input_schema_summary.contains("mode=replace"));
    assert!(apply_patch.input_schema_summary.contains("mode=patch"));
    assert!(
        apply_patch
            .example_uses
            .iter()
            .any(|item| item.contains("exact function body"))
    );

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

    let write_stdin = tools
        .model_tools
        .iter()
        .find(|tool| tool.tool_name == "write_stdin")
        .expect("write_stdin tool");
    assert!(
        write_stdin
            .when_not_to_use
            .iter()
            .any(|item| item.contains("no exec replay session"))
    );

    let task_status = tools
        .model_tools
        .iter()
        .find(|tool| tool.tool_name == "project.task.status")
        .expect("project.task.status tool");
    assert!(
        task_status
            .when_not_to_use
            .iter()
            .any(|item| item.contains("no runtime_home"))
    );

    let capability_invoke = tools
        .model_tools
        .iter()
        .find(|tool| tool.tool_name == "capability.invoke")
        .expect("capability.invoke tool");
    assert!(
        capability_invoke
            .when_not_to_use
            .iter()
            .any(|item| item.contains("no capability ids"))
    );
}

#[test]
fn dynamic_tool_catalog_promotes_write_stdin_when_exec_session_exists() {
    let worker = worker_runtime();
    let runtime_home = std::env::temp_dir().join("fin-runtime-dynamic-tool-catalog");
    fs::create_dir_all(runtime_home.join("runtime/tools/exec_sessions")).expect("exec session dir");
    fs::write(
        runtime_home.join("runtime/tools/exec_sessions/demo.json"),
        b"{\"session_id\":\"demo\"}",
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

#[test]
fn system_and_project_roles_share_runtime_but_receive_different_tool_policies() {
    let system_tools = crate::tool_catalog_dynamic::build_dynamic_tool_catalog_block(
        &crate::tool_catalog_dynamic::DynamicToolCatalogInput {
            role_id: Some("system"),
            context: &MinimalContextView::default(),
            recent_tool_records: &[],
            round_index: 1,
        },
    );
    let project_tools = crate::tool_catalog_dynamic::build_dynamic_tool_catalog_block(
        &crate::tool_catalog_dynamic::DynamicToolCatalogInput {
            role_id: Some("project"),
            context: &MinimalContextView::default(),
            recent_tool_records: &[],
            round_index: 1,
        },
    );

    assert!(system_tools.tool_selection_policy.iter().any(|item| {
        item.contains("framework-owned backlog/task-board state")
            && item.contains("agent presence")
            && item.contains("supervision")
    }));
    assert!(
        project_tools
            .tool_selection_policy
            .iter()
            .any(|item| item.contains("prefer project-scoped closure, task-board execution management, worker dispatch, review, and delivery progress"))
    );

    let system_peer_list = system_tools
        .model_tools
        .iter()
        .find(|tool| tool.tool_name == "peer.list")
        .expect("system peer.list");
    assert!(
        system_peer_list
            .when_to_use
            .iter()
            .any(|item| item.contains("system role"))
    );

    let project_patch = project_tools
        .model_tools
        .iter()
        .find(|tool| tool.tool_name == "apply_patch")
        .expect("project apply_patch");
    assert!(
        project_patch
            .when_to_use
            .iter()
            .any(|item| item.contains("project role"))
    );

    let default_tools = crate::tool_catalog_dynamic::build_dynamic_tool_catalog_block(
        &crate::tool_catalog_dynamic::DynamicToolCatalogInput {
            role_id: Some("default"),
            context: &MinimalContextView::default(),
            recent_tool_records: &[],
            round_index: 1,
        },
    );
    assert!(
        default_tools
            .tool_selection_policy
            .iter()
            .any(|item| item.contains("same project role"))
    );
}
