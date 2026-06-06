//! Prompt domain: role prompt assembly and prompt test fixtures.
//! Owning layer: runtime.
pub mod assembly;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod basics;
#[cfg(test)]
mod catalog;
#[cfg(test)]
mod role_policy;

// Shared test helpers + imports — visible to all `#[path]` sub-modules via `use super::*`.
#[cfg(test)]
pub(crate) use fin_config::{ConfigMapper, ProviderProtocol, UserConfig, UserProviderConfig};
#[cfg(test)]
pub(crate) use fin_contracts::{MinimalContextView, ToolExecutionRecord};
#[cfg(test)]
pub(crate) use crate::{
    context::view::{ContextAssemblyInput, ContextViewBuilder},
    WorkerRuntime,
};
#[cfg(test)]
use std::collections::BTreeMap;

#[cfg(test)]
pub(crate) fn worker_runtime() -> WorkerRuntime {
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

#[cfg(test)]
pub(crate) fn system_worker_runtime() -> WorkerRuntime {
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
    WorkerRuntime::from_system(
        &system,
        "agent-system",
        "worker-system",
        "runtime",
        Some("system"),
    )
    .expect("system worker runtime")
}
