use crate::{ContextAssemblyInput, ContextViewBuilder, WorkerRuntime};
use fin_config::{ConfigMapper, ProviderProtocol, UserConfig, UserProviderConfig};
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

#[path = "context_view_tests_peer_state.rs"]
mod peer_state;
#[path = "context_view_tests_rich_blocks.rs"]
mod rich_blocks;
