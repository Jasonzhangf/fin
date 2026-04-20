use crate::{ContextAssemblyInput, ContextViewBuilder, WorkerRuntime};
use fin_config::{ConfigMapper, ProviderProtocol, UserConfig, UserProviderConfig};
use fin_contracts::{DigestRecord, EntityRefs, InputAttachmentSummary};
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
            attachment_summaries: vec![InputAttachmentSummary {
                attachment_id: Some("att-1".into()),
                kind: "image/png".into(),
                name: Some("demo.png".into()),
                url: Some("https://example.com/demo.png".into()),
                width: Some(128),
                height: Some(64),
                ..InputAttachmentSummary::default()
            }],
        },
    );

    assert_eq!(
        context
            .control
            .as_ref()
            .and_then(|v| v.session_id.as_deref()),
        Some("session-rich")
    );
    assert_eq!(
        context.role_prompt.as_ref().map(|v| v.role_id.as_str()),
        Some("project")
    );
    assert_eq!(
        context.role_prompt.as_ref().map(|v| v.prompt_history.len()),
        Some(1)
    );
    assert!(
        context
            .role_prompt
            .as_ref()
            .map(|v| v.prompt_lineage.iter().any(|i| i.contains("stable core")))
            .unwrap_or(false)
    );
    assert!(
        context
            .role_prompt
            .as_ref()
            .map(|v| v
                .prompt_modules
                .iter()
                .any(|i| i.module_id == "stable_core.framework_truth_rules"))
            .unwrap_or(false)
    );
    assert!(
        context
            .role_prompt
            .as_ref()
            .map(|v| v
                .prompt_modules
                .iter()
                .any(|i| i.module_id == "role.project.identity"))
            .unwrap_or(false)
    );
    assert!(
        context
            .role_prompt
            .as_ref()
            .map(|v| v
                .prompt_modules
                .iter()
                .any(|i| i.module_id == "stable_core.request_framing"))
            .unwrap_or(false)
    );
    assert!(
        context
            .role_prompt
            .as_ref()
            .map(|v| v.prompt_layers.iter().any(|i| i.layer_id == "stable_core"))
            .unwrap_or(false)
    );
    assert!(
        context
            .role_prompt
            .as_ref()
            .map(|v| v
                .prompt_lineage
                .iter()
                .any(|i| i.contains("framework-routed request")))
            .unwrap_or(false)
    );
    assert!(
        context
            .role_prompt
            .as_ref()
            .map(|v| v.output_contract.len() >= 6)
            .unwrap_or(false)
    );
    assert!(
        context
            .role_prompt
            .as_ref()
            .map(|v| v
                .output_contract
                .iter()
                .any(|i| i.contains("project scope") || i.contains("verify step")))
            .unwrap_or(false)
    );
    assert!(
        context
            .role_prompt
            .as_ref()
            .map(|v| v
                .output_contract
                .iter()
                .any(|i| i.contains("model_output_contract_v1")))
            .unwrap_or(false)
    );
    assert!(
        context
            .role_prompt
            .as_ref()
            .map(|v| v
                .output_contract
                .iter()
                .any(|i| i.contains("do not emit extra control-feedback keys")))
            .unwrap_or(false)
    );
    assert!(
        context
            .role_prompt
            .as_ref()
            .map(|v| v
                .output_contract
                .iter()
                .any(|i| i.contains("two top-level blocks")))
            .unwrap_or(false)
    );
    assert!(
        context
            .role_prompt
            .as_ref()
            .map(|v| v.output_contract.iter().any(|i| i.contains("0.98/1.0")))
            .unwrap_or(false)
    );
    assert_eq!(
        context.tools.as_ref().map(|v| v.model_tools.len()),
        Some(24)
    );
    assert_eq!(
        context.tools.as_ref().map(|v| v.framework_tools.len()),
        Some(6)
    );
    assert_eq!(
        context
            .tools
            .as_ref()
            .map(|v| v.tool_selection_policy.len()),
        Some(10)
    );
    assert_eq!(
        context.tools.as_ref().map(|v| v.disabled_tools.len()),
        Some(3)
    );
    assert!(
        context
            .tools
            .as_ref()
            .map(|v| v
                .model_tools
                .iter()
                .any(|tool| tool.tool_name == "update_plan"))
            .unwrap_or(false)
    );
    assert!(
        context
            .tools
            .as_ref()
            .map(|v| v
                .model_tools
                .iter()
                .any(|tool| tool.tool_name == "session.list"))
            .unwrap_or(false)
    );
    assert!(
        context
            .tools
            .as_ref()
            .map(|v| v
                .model_tools
                .iter()
                .any(|tool| tool.tool_name == "apply_patch"))
            .unwrap_or(false)
    );
    assert!(
        context
            .tools
            .as_ref()
            .map(|v| v
                .model_tools
                .iter()
                .any(|tool| tool.tool_name == "view_image"))
            .unwrap_or(false)
    );
    assert!(
        context
            .tools
            .as_ref()
            .map(|v| v
                .model_tools
                .iter()
                .any(|tool| tool.tool_name == "context_history.rebuild"))
            .unwrap_or(false)
    );
    assert!(
        context
            .tools
            .as_ref()
            .map(|v| v
                .model_tools
                .iter()
                .any(|tool| tool.tool_name == "project.task.status"))
            .unwrap_or(false)
    );
    assert!(
        context
            .tools
            .as_ref()
            .map(|v| v
                .model_tools
                .iter()
                .any(|tool| tool.tool_name == "project.task.list"))
            .unwrap_or(false)
    );
    assert!(
        context
            .tools
            .as_ref()
            .map(|v| v
                .model_tools
                .iter()
                .any(|tool| tool.tool_name == "project.task.create"))
            .unwrap_or(false)
    );
    assert!(
        context
            .tools
            .as_ref()
            .map(|v| v
                .model_tools
                .iter()
                .any(|tool| tool.tool_name == "project.task.claim"))
            .unwrap_or(false)
    );
    assert!(
        context
            .tools
            .as_ref()
            .map(|v| v
                .model_tools
                .iter()
                .any(|tool| tool.tool_name == "project.task.submit"))
            .unwrap_or(false)
    );
    assert!(
        context
            .tools
            .as_ref()
            .map(|v| v
                .model_tools
                .iter()
                .any(|tool| tool.tool_name == "project.task.review"))
            .unwrap_or(false)
    );
    assert!(
        context
            .tools
            .as_ref()
            .map(|v| v
                .model_tools
                .iter()
                .any(|tool| tool.tool_name == "agent.presence.list"))
            .unwrap_or(false)
    );
    assert!(
        context
            .tools
            .as_ref()
            .map(|v| v
                .model_tools
                .iter()
                .any(|tool| tool.tool_name == "project.supervision.list"))
            .unwrap_or(false)
    );
    assert!(
        context
            .tools
            .as_ref()
            .map(|v| v
                .model_tools
                .iter()
                .any(|tool| tool.tool_name == "peer.list"))
            .unwrap_or(false)
    );
    assert!(
        context
            .tools
            .as_ref()
            .map(|v| v
                .model_tools
                .iter()
                .any(|tool| tool.tool_name == "wait.remind"))
            .unwrap_or(false)
    );
    assert!(
        context
            .tools
            .as_ref()
            .map(|v| v
                .model_tools
                .iter()
                .any(|tool| tool.tool_name == "reasoning.stop"))
            .unwrap_or(false)
    );
    assert!(
        context
            .tools
            .as_ref()
            .and_then(|v| v.framework_tools.first())
            .map(|t| !t.purpose.is_empty() && !t.input_schema_summary.is_empty())
            .unwrap_or(false)
    );
    assert_eq!(
        context.history.as_ref().map(|v| v.recent_messages.len()),
        Some(2)
    );
    assert_eq!(
        context
            .knowledge
            .as_ref()
            .map(|v| v.artifact_candidates.len()),
        Some(1)
    );
    assert_eq!(
        context.project.as_ref().and_then(|v| v.cwd.as_deref()),
        Some(cwd.as_str())
    );
    assert_eq!(
        context
            .project
            .as_ref()
            .map(|v| v.relative_selected_paths.clone()),
        Some(vec!["src".to_string(), "docs".to_string()])
    );
    assert_eq!(
        context
            .project
            .as_ref()
            .and_then(|v| v.primary_project.as_ref())
            .map(|v| v.label.as_str()),
        Some("fin")
    );
    assert_eq!(
        context.project.as_ref().map(|v| v.active_projects.len()),
        Some(1)
    );
    assert_eq!(context.project.as_ref().map(|v| v.projects.len()), Some(1));
    assert!(
        context
            .project
            .as_ref()
            .and_then(|v| v.focus_summary.as_deref())
            .unwrap_or_default()
            .contains("src")
    );
    assert_eq!(
        context.current_input.as_ref().map(|v| v.input.as_str()),
        Some("current input")
    );
    assert_eq!(
        context.current_input.as_ref().map(|v| v.attachments.len()),
        Some(1)
    );
    assert_eq!(
        context.peer.as_ref().map(|v| v.active_peer_ids.len()),
        Some(1)
    );
    assert!(
        context
            .peer
            .as_ref()
            .and_then(|v| v.topology_summary.as_deref())
            .unwrap_or_default()
            .contains("local-only M1 placeholder")
    );

    let encoded = serde_json::to_value(&context).expect("context should serialize");
    assert!(encoded.get("control").is_some());
    assert!(encoded.get("role_prompt").is_some());
    assert!(encoded.get("tools").is_some());
    assert!(encoded.get("history").is_some());
    assert!(encoded.get("knowledge").is_some());
    assert!(encoded.get("project").is_some());
    assert!(encoded.get("peer").is_some());
    assert!(encoded.get("current_input").is_some());
}

#[test]
fn context_view_builder_loads_ensured_local_worker_peers_from_runtime_state() {
    let worker = worker_runtime();
    let runtime_home = std::env::temp_dir().join("fin-context-peer-state-test");
    let state_dir = runtime_home.join("runtime/peers/state");
    fs::create_dir_all(&state_dir).expect("state dir");
    fs::write(
        state_dir.join("local-worker-b.json"),
        r#"{
  "peer_id":"local-worker-b",
  "peer_kind":"project_worker",
  "lifecycle_state":"online",
  "last_heartbeat_at":"2026-04-20T16:00:00+08:00",
  "reconnect_backoff_ms":0
}"#,
    )
    .expect("peer state");

    let context = ContextViewBuilder.build(
        &worker,
        ContextAssemblyInput {
            runtime_home: Some(runtime_home.display().to_string()),
            ..ContextAssemblyInput::default()
        },
    );

    let peer = context.peer.expect("peer block");
    assert_eq!(peer.active_peer_ids.len(), 2);
    assert!(
        peer.active_peer_ids
            .iter()
            .any(|value| value == "local-worker-1")
    );
    assert!(
        peer.active_peer_ids
            .iter()
            .any(|value| value == "local-worker-b")
    );
    assert!(
        peer.topology_summary
            .as_deref()
            .unwrap_or_default()
            .contains("ensured peer")
    );
    assert_eq!(
        peer.daemon
            .as_ref()
            .and_then(|value| value.supervision_state.as_deref()),
        Some("local_peer_state_visible")
    );
    assert!(peer.peers.iter().any(|item| {
        item.peer_id == "local-worker-b"
            && item.peer_kind == "project_worker"
            && item.presence_state == "online"
    }));
}
