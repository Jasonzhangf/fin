use crate::{
    CliError, agent_presence::ensure_entry_agent_presence, channel_peer::ensure_builtin_qqbot_peer,
    channel_peer_qqbot_bridge::BuiltinQqbotBridge, startup_wakeup::refresh_startup_control_plane,
    time::local_timestamp_now, web_debug::CliDebugActionHandler,
};
use fin_config::SystemConfig;
use fin_contracts::InputAttachmentSummary;
use fin_debug_server::{ChatSendRequest, serve_debug_mvp_with_handler};
use std::path::{Path, PathBuf};

pub(crate) fn serve_web_debug(
    user_toml_path: &Path,
    user_toml: String,
    system: SystemConfig,
    runtime_home: PathBuf,
    bind_addr: &str,
) -> Result<(), CliError> {
    let now = local_timestamp_now();
    let _ = refresh_startup_control_plane(&runtime_home, &system, &now)?;
    let _ = ensure_entry_agent_presence(&system, &runtime_home, &now)?;
    ensure_builtin_qqbot_peer(&runtime_home)?;
    let handler = CliDebugActionHandler::new(user_toml.clone(), system)?;
    let _qqbot_bridge =
        BuiltinQqbotBridge::start(&runtime_home, handler.clone(), Some(user_toml_path))?;
    serve_debug_mvp_with_handler(&runtime_home, bind_addr, &handler)?;
    Ok(())
}

pub(crate) fn build_channel_peer_request(
    message: String,
    attachments: Vec<InputAttachmentSummary>,
) -> ChatSendRequest {
    ChatSendRequest {
        message,
        input_kind: Some("channel_ingress".into()),
        attachments,
    }
}
