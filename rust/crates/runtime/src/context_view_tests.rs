use crate::{ContextAssemblyInput, ContextViewBuilder, WorkerRuntime};
use fin_config::{
    ConfigMapper, ProviderProtocol, UserConfig, UserProviderConfig,
};
use fin_contracts::{DigestRecord, EntityRefs};
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
fn context_view_builder_populates_rich_blocks() {
    let worker = worker_runtime();
    let cwd = std::env::current_dir()
        .expect("cwd should exist")
        .display()
        .to_string();
    let context = ContextViewBuilder.build(
        &worker,
        ContextAssemblyInput {
            operation_id: "op-rich".into(),
            trace_id: "trace-rich".into(),
            refs: EntityRefs {
                session_id: Some("session-rich".into()),
                task_id: Some("task-rich".into()),
                ..EntityRefs::default()
            },
            input: "current input".into(),
            source: "cli.user".into(),
            recent_messages: vec!["user: hello".into(), "assistant: hi".into()],
            recent_digests: vec![DigestRecord {
                digest_id: "digest-1".into(),
                closure_id: "closure-1".into(),
                refs: EntityRefs {
                    session_id: Some("session-rich".into()),
                    task_id: Some("task-rich".into()),
                    ..EntityRefs::default()
                },
                summary: "digest summary".into(),
                continuity_tail: vec!["hello".into(), "hi".into()],
                note_refs: vec!["note-1".into()],
                artifact_candidates: vec!["artifact-1".into()],
                control_feedback: None,
                created_at: "2026-04-17T00:00:00Z".into(),
            }],
            recent_reasoning_views: Vec::new(),
            recent_tool_records: Vec::new(),
            project_label: Some("fin".into()),
            runtime_home: Some("/tmp/fin".into()),
            cwd: Some(cwd.clone()),
            selected_paths: vec!["src".into(), "docs".into()],
        },
    );

    assert_eq!(context.control.as_ref().and_then(|v| v.session_id.as_deref()), Some("session-rich"));
    assert_eq!(context.role_prompt.as_ref().map(|v| v.role_id.as_str()), Some("default"));
    assert_eq!(context.role_prompt.as_ref().map(|v| v.prompt_history.len()), Some(1));
    assert!(context.role_prompt.as_ref().map(|v| v.prompt_lineage.iter().any(|i| i.contains("stable core"))).unwrap_or(false));
    assert!(context.role_prompt.as_ref().map(|v| v.prompt_modules.iter().any(|i| i.module_id == "stable_core.framework_truth_rules")).unwrap_or(false));
    assert!(context.role_prompt.as_ref().map(|v| v.prompt_modules.iter().any(|i| i.module_id == "role.project.purpose")).unwrap_or(false));
    assert!(context.role_prompt.as_ref().map(|v| v.prompt_modules.iter().any(|i| i.module_id == "overlay.gpt_codex.tool_persistence")).unwrap_or(false));
    assert!(context.role_prompt.as_ref().map(|v| v.prompt_layers.iter().any(|i| i.layer_id == "stable_core")).unwrap_or(false));
    assert!(context.role_prompt.as_ref().map(|v| v.prompt_layers.iter().any(|i| i.layer_id == "model_overlay")).unwrap_or(false));
    assert!(context.role_prompt.as_ref().map(|v| v.output_contract.len() >= 6).unwrap_or(false));
    assert!(context.role_prompt.as_ref().map(|v| v.output_contract.iter().any(|i| i.contains("project scope") || i.contains("verify step"))).unwrap_or(false));
    assert!(context.role_prompt.as_ref().map(|v| v.output_contract.iter().any(|i| i.contains("model_output_contract_v1"))).unwrap_or(false));
    assert!(context.role_prompt.as_ref().map(|v| v.output_contract.iter().any(|i| i.contains("do not emit extra control-feedback keys"))).unwrap_or(false));
    assert!(context.role_prompt.as_ref().map(|v| v.output_contract.iter().any(|i| i.contains("two top-level blocks"))).unwrap_or(false));
    assert!(context.role_prompt.as_ref().map(|v| v.output_contract.iter().any(|i| i.contains("0.98/1.0"))).unwrap_or(false));
    assert_eq!(context.tools.as_ref().map(|v| v.framework_tools.len()), Some(6));
    assert_eq!(context.tools.as_ref().map(|v| v.tool_selection_policy.len()), Some(3));
    assert_eq!(context.tools.as_ref().map(|v| v.disabled_tools.len()), Some(3));
    assert!(context.tools.as_ref().and_then(|v| v.framework_tools.first()).map(|t| !t.purpose.is_empty() && !t.input_schema_summary.is_empty()).unwrap_or(false));
    assert_eq!(context.history.as_ref().map(|v| v.recent_messages.len()), Some(2));
    assert_eq!(context.knowledge.as_ref().map(|v| v.artifact_candidates.len()), Some(1));
    assert_eq!(context.project.as_ref().and_then(|v| v.cwd.as_deref()), Some(cwd.as_str()));
    assert_eq!(context.project.as_ref().map(|v| v.relative_selected_paths.clone()), Some(vec!["src".to_string(), "docs".to_string()]));
    assert_eq!(context.project.as_ref().and_then(|v| v.primary_project.as_ref()).map(|v| v.label.as_str()), Some("fin"));
    assert_eq!(context.project.as_ref().map(|v| v.active_projects.len()), Some(1));
    assert_eq!(context.project.as_ref().map(|v| v.projects.len()), Some(1));
    assert!(context.project.as_ref().and_then(|v| v.focus_summary.as_deref()).unwrap_or_default().contains("src"));
    assert_eq!(context.current_input.as_ref().map(|v| v.input.as_str()), Some("current input"));

    let encoded = serde_json::to_value(&context).expect("context should serialize");
    assert!(encoded.get("control").is_some());
    assert!(encoded.get("role_prompt").is_some());
    assert!(encoded.get("tools").is_some());
    assert!(encoded.get("history").is_some());
    assert!(encoded.get("knowledge").is_some());
    assert!(encoded.get("project").is_some());
    assert!(encoded.get("current_input").is_some());
}
