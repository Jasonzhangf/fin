use super::*;

#[test]
fn context_view_builder_exposes_loaded_global_skills() {
    let worker = worker_runtime();
    let home = std::env::temp_dir().join("fin-runtime-global-skills-test");
    std::fs::create_dir_all(home.join("skills/sample-skill")).expect("skill dir should create");
    std::fs::write(
        home.join("skills/sample-skill/SKILL.md"),
        "---
name: sample-skill
description: Sample skill for prompt loading.
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
                .any(|item| item.contains("sample-skill")))
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

#[test]
fn system_prompt_forces_cross_cwd_dispatch_and_names_control_tools() {
    let system_worker = system_worker_runtime();
    let system_context = ContextViewBuilder.build(&system_worker, ContextAssemblyInput::default());
    let role_prompt = system_context.role_prompt.expect("system role prompt");

    assert!(role_prompt.prompt_modules.iter().any(|item| {
        item.module_id == "role.system.cross_cwd_dispatch"
            && item.summary.contains("different project root / cwd")
            && item.summary.contains("project agent")
    }));
    assert!(role_prompt.behavior_rules.iter().any(|item| {
        item.contains("different project root / cwd")
            && item.contains("project agent")
            && item.contains("do not perform substantial cross-cwd execution")
    }));
    assert!(role_prompt.behavior_rules.iter().any(|item| {
        item.contains("peer.list")
            && item.contains("peer.describe")
            && item.contains("agent.presence.list")
            && item.contains("project.supervision.list")
    }));
    assert!(role_prompt.behavior_rules.iter().any(|item| {
        item.contains("project.task.list")
            && item.contains("agent.assign")
            && item.contains("explicit result refs")
    }));
}

#[test]
fn project_prompt_requires_bounded_execution_and_review_truth() {
    let project_worker = worker_runtime();
    let project_context =
        ContextViewBuilder.build(&project_worker, ContextAssemblyInput::default());
    let role_prompt = project_context.role_prompt.expect("project role prompt");

    assert!(role_prompt.prompt_modules.iter().any(|item| {
        item.module_id == "role.project.worker_execution_modes"
            && item.summary.contains("execution")
            && item.summary.contains("review")
            && item.summary.contains("handoff")
    }));
    assert!(role_prompt.behavior_rules.iter().any(|item| {
        item.contains("project.task.list")
            && item.contains("project.task.claim")
            && item.contains("project.task.submit")
            && item.contains("project.task.review")
    }));
    assert!(role_prompt.behavior_rules.iter().any(|item| {
        item.contains("same project") && item.contains("agent.assign") && item.contains("bounded")
    }));
    assert!(role_prompt.behavior_rules.iter().any(|item| {
        item.contains("subagents")
            && item.contains("progress")
            && item.contains("review")
            && item.contains("closure")
    }));
}

#[test]
fn prompt_rules_enforce_conversational_continuity_and_supervised_progress() {
    let system_context =
        ContextViewBuilder.build(&system_worker_runtime(), ContextAssemblyInput::default());
    let system_role_prompt = system_context.role_prompt.expect("system role prompt");
    assert!(system_role_prompt.behavior_rules.iter().any(|item| {
        item.contains("ongoing agent conversation") && item.contains("topic shift")
    }));
    assert!(
        system_role_prompt
            .behavior_rules
            .iter()
            .any(|item| { item.contains("do not silently stop after the first tool call") })
    );
    assert!(system_role_prompt.behavior_rules.iter().any(|item| {
        item.contains("keep the user-facing conversation alive")
            && item.contains("completion review")
            && item.contains("recovery")
    }));
    assert!(system_role_prompt.behavior_rules.iter().any(|item| {
        item.contains("continuous timeline thread")
            && item.contains("isolated question-answer block")
    }));
    assert!(system_role_prompt.behavior_rules.iter().any(|item| {
        item.contains("append-only continuation updates")
            && item.contains("final result")
            && item.contains("closed")
    }));

    let project_context =
        ContextViewBuilder.build(&worker_runtime(), ContextAssemblyInput::default());
    let project_role_prompt = project_context.role_prompt.expect("project role prompt");
    assert!(project_role_prompt.behavior_rules.iter().any(|item| {
        item.contains("new forward timeline updates") && item.contains("rewriting prior history")
    }));
    assert!(project_role_prompt.behavior_rules.iter().any(|item| {
        item.contains("delegated session thread") && item.contains("unrelated fresh conversation")
    }));
    assert!(system_role_prompt.behavior_rules.iter().any(|item| {
        item.contains("before the first framework tool call")
            && item.contains("state the routing/execution intent")
            && item.contains("what result will be returned")
    }));
    assert!(system_role_prompt.behavior_rules.iter().any(|item| {
        item.contains("after each framework tool result")
            && item.contains("decide the next action explicitly")
            && item.contains("do not stop at raw tool output")
    }));
    assert!(system_role_prompt.behavior_rules.iter().any(|item| {
        item.contains("managed execution recipe")
            && item.contains("inspect framework truth")
            && item.contains("choose one tool")
            && item.contains("review the returned artifact")
    }));
    assert!(system_role_prompt.behavior_rules.iter().any(|item| {
        item.contains("peer.list")
            && item.contains("peer.describe")
            && item.contains("daemon.ensure_peer")
            && item.contains("agent.assign")
    }));
    assert!(project_role_prompt.behavior_rules.iter().any(|item| {
        item.contains("tool calls as steps inside one managed execution loop")
            && item.contains("inspect")
            && item.contains("decide")
            && item.contains("act")
            && item.contains("verify")
    }));
    assert!(project_role_prompt.behavior_rules.iter().any(|item| {
        item.contains("managed project execution recipe")
            && item.contains("claim or inspect task truth")
            && item.contains("bounded next step")
            && item.contains("submit/review/continue")
    }));
}
