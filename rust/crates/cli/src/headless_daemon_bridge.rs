use crate::{
    CliError,
    channel_peer::{ensure_builtin_qqbot_peer, record_builtin_qqbot_runtime_event},
    channel_peer_connectivity::resolve_qqbot_credentials,
    channel_peer_qqbot_bridge::BuiltinQqbotBridge,
    web_debug::CliDebugActionHandler,
};
use serde_json::json;
use std::path::Path;

pub(crate) fn maybe_start_builtin_qqbot_bridge(
    runtime_home: &Path,
    handler: &CliDebugActionHandler,
) -> Result<Option<BuiltinQqbotBridge>, CliError> {
    let user_toml_path = runtime_home.join("config/user.toml");
    let _ = ensure_builtin_qqbot_peer(runtime_home)?;
    if resolve_qqbot_credentials(Some(user_toml_path.as_path())).is_none() {
        let _ = record_builtin_qqbot_runtime_event(
            runtime_home,
            "channel.peer.bridge_start_skipped",
            Some("runtime_ready"),
            Some("auth_required"),
            None,
            json!({
                "reason": "missing_credentials",
                "user_toml_path": user_toml_path.display().to_string(),
            }),
        )?;
        return Ok(None);
    }
    BuiltinQqbotBridge::start(
        runtime_home,
        handler.clone(),
        Some(user_toml_path.as_path()),
    )
    .map(Some)
}
