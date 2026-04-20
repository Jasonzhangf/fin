use super::*;
use fin_config::{
    ConfigMapper, ProviderCredential, ProviderProtocol, ResolvedProviderConfig, UserConfig,
    UserProviderConfig,
};
use fin_provider::{ProviderDescriptor, StaticProviderClient};
use std::collections::BTreeMap;

fn provider() -> StaticProviderClient {
    StaticProviderClient::new(ProviderDescriptor::from_resolved(&ResolvedProviderConfig {
        name: "openai".into(),
        protocol: ProviderProtocol::OpenAiCompatible,
        base_url: "https://api.example.com/v1".into(),
        model: "gpt-5".into(),
        credential: ProviderCredential::ApiKeyEnv {
            env_var: "OPENAI_API_KEY".into(),
        },
        user_agent: None,
        headers: BTreeMap::new(),
    }))
}

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

#[path = "tests_context_render.rs"]
mod context_render;
#[path = "tests_mainline.rs"]
mod mainline;
#[path = "tests_role_runtime.rs"]
mod role_runtime;
