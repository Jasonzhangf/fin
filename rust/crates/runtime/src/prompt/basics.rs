use super::*;

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
    let project_context =
        ContextViewBuilder.build(&project_worker, ContextAssemblyInput::default());
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
    assert!(
        system_role_prompt
            .prompt_modules
            .iter()
            .any(|item| item.module_id == "role.system.framework_state_first")
    );
    assert!(system_role_prompt.behavior_rules.iter().any(|item| {
        item.contains("framework-owned task board")
            && item.contains("agent presence")
            && item.contains("supervision")
    }));
}
