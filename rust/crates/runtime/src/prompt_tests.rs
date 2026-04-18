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
fn gpt_model_receives_gpt_codex_overlay_modules() {
    let worker = worker_runtime();
    let context = ContextViewBuilder.build(&worker, ContextAssemblyInput::default());

    assert!(
        context
            .role_prompt
            .as_ref()
            .map(|value| value
                .prompt_modules
                .iter()
                .any(|item| item.module_id == "overlay.gpt_codex.tool_persistence"))
            .unwrap_or(false)
    );
}
