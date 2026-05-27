use crate::config::map_system_config;
use crate::runtime_home::ensure_runtime_home_layout;
use crate::web_debug::CliDebugActionHandler;
use fin_config::SystemConfig;
use fin_provider::{ProviderDescriptor, StructuredStaticProviderClient};
use std::path::PathBuf;
use std::{
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

static TEMP_RUNTIME_COUNTER: AtomicU64 = AtomicU64::new(1);

pub(super) fn sample_user_toml() -> String {
    r#"
default_provider = "openai"

[providers.openai]
protocol = "open-ai-compatible"
base_url = "https://api.example.com/v1"
model = "gpt-5"
api_key_env = "OPENAI_API_KEY"
"#
    .into()
}

pub(super) fn temp_runtime_home() -> PathBuf {
    let seq = TEMP_RUNTIME_COUNTER.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!(
        "fin-status-probe-{}-{seq}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time should work")
            .as_nanos()
    ))
}

pub(super) fn static_provider(system: &SystemConfig) -> StructuredStaticProviderClient {
    StructuredStaticProviderClient::new(ProviderDescriptor::from_resolved(
        system.default_provider_config().expect("default provider"),
    ))
}

pub(super) fn build_handler(home: &std::path::Path) -> CliDebugActionHandler {
    ensure_runtime_home_layout(home).expect("runtime home should init");
    let system = map_system_config(&sample_user_toml()).expect("system config");
    CliDebugActionHandler::new(sample_user_toml(), system).expect("handler should build")
}
