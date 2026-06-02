use super::*;

#[test]
fn runtime_policy_supports_system_and_project_roles_from_same_system_config() {
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

    let system_runtime = WorkerRuntime::from_system(
        &system,
        "agent-system",
        "worker-sys",
        "runtime",
        Some("system"),
    )
    .expect("system runtime");
    let project_runtime = WorkerRuntime::from_system(
        &system,
        "agent-project",
        "worker-project",
        "runtime",
        Some("project"),
    )
    .expect("project runtime");

    assert_eq!(system_runtime.policy.role.role_id.as_str(), "system");
    assert_eq!(project_runtime.policy.role.role_id.as_str(), "project");
    assert_eq!(
        system_runtime
            .policy
            .provider_path
            .primary_target()
            .provider_name,
        project_runtime
            .policy
            .provider_path
            .primary_target()
            .provider_name
    );
    assert_eq!(
        system_runtime.policy.provider_path.primary_target().model,
        project_runtime.policy.provider_path.primary_target().model
    );
}

#[test]
fn project_agent_can_materialize_multiple_worker_runtimes_without_new_roles() {
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

    let worker_a = WorkerRuntime::from_system(
        &system,
        "agent-project",
        "worker-a",
        "runtime",
        Some("project"),
    )
    .expect("worker a");
    let worker_b = WorkerRuntime::from_system(
        &system,
        "agent-project",
        "worker-b",
        "runtime",
        Some("project"),
    )
    .expect("worker b");

    assert_eq!(worker_a.agent_id.as_str(), "agent-project");
    assert_eq!(worker_b.agent_id.as_str(), "agent-project");
    assert_eq!(worker_a.policy.role.role_id.as_str(), "project");
    assert_eq!(worker_b.policy.role.role_id.as_str(), "project");
    assert_ne!(worker_a.worker_id, worker_b.worker_id);
    assert_eq!(
        worker_a.policy.provider_path.primary_target().provider_name,
        worker_b.policy.provider_path.primary_target().provider_name
    );

    let context_a = ContextViewBuilder.build(
        &worker_a,
        ContextAssemblyInput {
            operation_id: "op-worker-a".into(),
            trace_id: "trace-worker-a".into(),
            input: "task slice a".into(),
            source: "runtime".into(),
            ..ContextAssemblyInput::default()
        },
    );
    let context_b = ContextViewBuilder.build(
        &worker_b,
        ContextAssemblyInput {
            operation_id: "op-worker-b".into(),
            trace_id: "trace-worker-b".into(),
            input: "task slice b".into(),
            source: "runtime".into(),
            ..ContextAssemblyInput::default()
        },
    );

    assert_eq!(
        context_a
            .role_prompt
            .as_ref()
            .map(|value| value.role_id.as_str()),
        Some("project")
    );
    assert_eq!(
        context_b
            .role_prompt
            .as_ref()
            .map(|value| value.role_id.as_str()),
        Some("project")
    );
    assert_eq!(
        context_a
            .peer
            .as_ref()
            .map(|value| value.active_peer_ids.as_slice()),
        Some(&["local-worker-a".to_string()][..])
    );
    assert_eq!(
        context_b
            .peer
            .as_ref()
            .map(|value| value.active_peer_ids.as_slice()),
        Some(&["local-worker-b".to_string()][..])
    );
}

#[test]
fn removed_worker_role_is_rejected_as_prompt_role() {
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

    let err = WorkerRuntime::from_system(
        &system,
        "agent-project",
        "worker-bad-role",
        "runtime",
        Some("worker"),
    )
    .expect_err("worker role should no longer resolve as a prompt role");

    assert!(
        err.to_string()
            .contains("runtime policy role 'worker' is missing")
    );
}

#[test]
fn legacy_default_role_alias_resolves_to_project() {
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
    let mut system = ConfigMapper::map_user_to_system(&user).expect("mapping should succeed");
    system.policy.default_role = "default".into();

    let runtime =
        WorkerRuntime::from_system(&system, "agent-project", "worker-project", "runtime", None)
            .expect("default alias should resolve");
    let explicit = WorkerRuntime::from_system(
        &system,
        "agent-project",
        "worker-project-2",
        "runtime",
        Some("default"),
    )
    .expect("explicit default alias should resolve");

    assert_eq!(runtime.policy.role.role_id.as_str(), "project");
    assert_eq!(explicit.policy.role.role_id.as_str(), "project");
}
