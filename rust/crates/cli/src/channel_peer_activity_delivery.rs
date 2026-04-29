use crate::{CliError, channel_peer::active_builtin_qqbot_session, time::local_timestamp_now};
use fin_contracts::ActivityCardsSnapshot;
use fin_runtime::{build_activity_cards, build_activity_cards_for_session};
use serde::{Deserialize, Serialize};
use std::path::Path;

#[path = "channel_peer_activity_delivery_render.rs"]
mod channel_peer_activity_delivery_render;
#[path = "channel_peer_activity_delivery_store.rs"]
mod channel_peer_activity_delivery_store;

use channel_peer_activity_delivery_render::{render_compact_text, should_emit_snapshot};
use channel_peer_activity_delivery_store::{load_state, persist_state, signature_for_snapshot};

const MAX_ACTIVE_SOURCES: usize = 3;
const SYSTEM_SOURCE_ID: &str = "system-agent";
const PENDING_INBOUND_NOTICE: &str = "已收到，正在处理";

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct QqbotActivityDeliveryState {
    #[serde(default)]
    pub(crate) session_id: Option<String>,
    #[serde(default)]
    pub(crate) target: Option<String>,
    #[serde(default)]
    pub(crate) last_user_signature: Option<String>,
    #[serde(default)]
    pub(crate) last_delivered_text: Option<String>,
    #[serde(default)]
    pub(crate) last_delivery_reason: Option<String>,
    #[serde(default)]
    pub(crate) last_delivery_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PreparedActivityDelivery {
    pub(crate) target: String,
    pub(crate) text: String,
    pub(crate) signature: String,
    pub(crate) reason: String,
}

pub(crate) fn bind_target(
    runtime_home: &Path,
    session_id: &str,
    target: &str,
) -> Result<QqbotActivityDeliveryState, CliError> {
    let mut state = load_state(runtime_home)?;
    state.session_id = Some(session_id.to_string());
    state.target = Some(target.to_string());
    persist_state(runtime_home, &state)?;
    Ok(state)
}

pub(crate) fn clear_target(runtime_home: &Path) -> Result<(), CliError> {
    let mut state = load_state(runtime_home)?;
    state.target = None;
    state.session_id = None;
    persist_state(runtime_home, &state)
}

pub(crate) fn prepare_periodic_delivery(
    runtime_home: &Path,
) -> Result<Option<PreparedActivityDelivery>, CliError> {
    let state = load_state(runtime_home)?;
    let Some(target) = state.target.clone() else {
        return Ok(None);
    };
    let Some(session_id) = state.session_id.clone() else {
        return Ok(None);
    };
    let Some(active_session_id) = active_builtin_qqbot_session(runtime_home)? else {
        clear_target(runtime_home)?;
        return Ok(None);
    };
    if active_session_id != session_id {
        clear_target(runtime_home)?;
        return Ok(None);
    }
    let snapshot = build_activity_cards(runtime_home)?;
    if snapshot
        .session_id
        .as_deref()
        .is_some_and(|current| current != session_id)
    {
        clear_target(runtime_home)?;
        return Ok(None);
    }
    if !should_emit_snapshot(&snapshot) {
        return Ok(None);
    }
    let signature = signature_for_snapshot(&snapshot);
    let rendered = render_compact_text(&snapshot);
    if rendered.trim().is_empty() {
        return Ok(None);
    }
    let changed = state.last_user_signature.as_deref() != Some(signature.as_str())
        || state.last_delivered_text.as_deref() != Some(rendered.as_str());
    if !changed {
        return Ok(None);
    }
    Ok(Some(PreparedActivityDelivery {
        target,
        text: rendered,
        signature,
        reason: "diff".into(),
    }))
}

pub(crate) fn current_delivery_signature_if_deliverable(
    runtime_home: &Path,
) -> Result<Option<String>, CliError> {
    let snapshot = build_activity_cards(runtime_home)?;
    if !should_emit_snapshot(&snapshot) {
        return Ok(None);
    }
    Ok(Some(signature_for_snapshot(&snapshot)))
}

#[allow(dead_code)]
pub(crate) fn prepare_embedded_reply(
    runtime_home: &Path,
    answer: &str,
) -> Result<Option<(String, String)>, CliError> {
    let snapshot = build_activity_cards(runtime_home)?;
    let rendered = render_compact_text(&snapshot);
    if rendered.trim().is_empty() {
        return Ok(None);
    }
    Ok(Some((
        format!(
            "{}

{}",
            answer.trim_end(),
            rendered
        ),
        signature_for_snapshot(&snapshot),
    )))
}

pub(crate) fn mark_delivered(
    runtime_home: &Path,
    signature: &str,
    text: &str,
    reason: &str,
) -> Result<QqbotActivityDeliveryState, CliError> {
    let mut state = load_state(runtime_home)?;
    state.last_user_signature = Some(signature.to_string());
    state.last_delivered_text = Some(text.to_string());
    state.last_delivery_reason = Some(reason.to_string());
    state.last_delivery_at = Some(local_timestamp_now());
    persist_state(runtime_home, &state)?;
    Ok(state)
}

#[allow(dead_code)]
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
struct PeerRegistry {
    #[serde(default)]
    peers: Vec<PeerRegistryEntry>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
struct PeerRegistryEntry {
    peer_id: String,
    peer_kind: String,
    presence_state: String,
    #[serde(default)]
    lifecycle_state: String,
    #[serde(default)]
    runtime_state: Option<String>,
    #[serde(default)]
    connectivity_state: Option<String>,
    #[serde(default)]
    binding_state: Option<String>,
    updated_at: String,
    #[serde(default)]
    pairing_required: Option<bool>,
    #[serde(default)]
    session_valid: Option<bool>,
    #[serde(default)]
    session_id: Option<String>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
struct ChannelConversationRegistry {
    #[serde(default)]
    conversations: Vec<ChannelConversationRecord>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
struct ChannelConversationRecord {
    #[serde(default)]
    session_id: Option<String>,
    #[serde(default)]
    last_inbound_message_id: Option<String>,
    #[serde(default)]
    last_inbound_at: Option<String>,
    #[serde(default)]
    last_delivered_message_id: Option<String>,
    #[serde(default)]
    last_delivery_at: Option<String>,
    #[serde(default)]
    status: String,
}

#[cfg(test)]
#[path = "channel_peer_activity_delivery_tests.rs"]
mod tests;
