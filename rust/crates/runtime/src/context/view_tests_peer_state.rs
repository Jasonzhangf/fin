use super::*;
use std::fs;

pub(crate) use fin_config::{ConfigMapper, ProviderProtocol, UserConfig, UserProviderConfig};
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
        runtime: fin_config::UserRuntimeConfig::default(),
    };
    let system = ConfigMapper::map_user_to_system(&user).expect("mapping should succeed");
    WorkerRuntime::from_system(&system, "agent-1", "worker-1", "runtime", None)
        .expect("worker runtime")
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
