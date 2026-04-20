use crate::{
    CliError,
    channel_peer_activity_delivery::clear_target,
    channel_peer_connectivity::{apply_probe_failure, apply_probe_success, probe_qqbot_upstream},
    channel_peer_store::{append_peer_event, persist_state, read_json},
    time::local_timestamp_now,
};
#[path = "channel_peer_state.rs"]
mod channel_peer_state;
use channel_peer_state::{
    add_minutes, release_binding_state, repair_state_defaults, session_is_expired,
    sync_compat_fields,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{fs, path::Path};

const QQBOT_PEER_ID: &str = "peer-channel-gateway-qqbot-local";
const QQBOT_PEER_KIND: &str = "channel_gateway.qqbot";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct GatewayPeerState {
    pub(crate) peer_id: String,
    pub(crate) peer_kind: String,
    #[serde(default)]
    pub(crate) runtime_state: String,
    #[serde(default)]
    pub(crate) connectivity_state: String,
    #[serde(default)]
    pub(crate) binding_state: String,
    pub(crate) lifecycle_state: String,
    pub(crate) pairing_required: bool,
    pub(crate) session_valid: bool,
    pub(crate) started_at: String,
    pub(crate) updated_at: String,
    #[serde(default)]
    pub(crate) paired_at: Option<String>,
    #[serde(default)]
    pub(crate) session_id: Option<String>,
    #[serde(default)]
    pub(crate) session_expires_at: Option<String>,
    #[serde(default)]
    pub(crate) session_ttl_minutes: Option<u64>,
    #[serde(default)]
    pub(crate) last_heartbeat_at: Option<String>,
    #[serde(default)]
    pub(crate) connectivity_checked_at: Option<String>,
    #[serde(default)]
    pub(crate) upstream_authenticated_at: Option<String>,
    #[serde(default)]
    pub(crate) upstream_expires_at: Option<String>,
    #[serde(default)]
    pub(crate) credential_source: Option<String>,
    #[serde(default)]
    pub(crate) last_connectivity_error: Option<String>,
    #[serde(default)]
    pub(crate) reconnect_count: u64,
    #[serde(default = "default_next_event_sequence")]
    pub(crate) next_event_sequence: u64,
}

fn default_next_event_sequence() -> u64 {
    1
}

pub(crate) fn ensure_builtin_qqbot_peer(runtime_home: &Path) -> Result<GatewayPeerState, CliError> {
    let now = local_timestamp_now();
    let peer_dir = runtime_home.join("runtime/peers/qqbot");
    fs::create_dir_all(&peer_dir).map_err(|source| CliError::WriteFile {
        path: peer_dir.display().to_string(),
        source,
    })?;
    let state_path = peer_dir.join("state.json");
    let loaded = read_json::<GatewayPeerState>(&state_path)?;
    let created_new = loaded.is_none();
    let mut state = loaded.unwrap_or_else(|| GatewayPeerState::new_unpaired(now.as_str()));
    let previous = state.clone();
    repair_state_defaults(&mut state, now.as_str());
    state.updated_at = now.clone();
    if previous.started_at.is_empty() {
        state.started_at = now.clone();
    }
    if previous.peer_id.is_empty() {
        state.peer_id = QQBOT_PEER_ID.into();
    }
    if previous.peer_kind.is_empty() {
        state.peer_kind = QQBOT_PEER_KIND.into();
    }

    let mut transition_events: Vec<(&str, Value)> = Vec::new();
    if created_new {
        transition_events.push((
            "channel.peer.runtime_ready",
            json!({
                "runtime_state": state.runtime_state.clone(),
                "connectivity_state": state.connectivity_state.clone(),
                "binding_state": state.binding_state.clone(),
                "reason": "initial_bootstrap",
            }),
        ));
        transition_events.push((
            "channel.peer.binding_unbound",
            json!({
                "runtime_state": state.runtime_state.clone(),
                "connectivity_state": state.connectivity_state.clone(),
                "binding_state": state.binding_state.clone(),
                "reason": "initial_bootstrap",
                "pairing_required": false,
            }),
        ));
    }
    if session_is_expired(&state, now.as_str()) && state.session_valid {
        state.binding_state = "expired".into();
        sync_compat_fields(&mut state);
        state.reconnect_count = state.reconnect_count.saturating_add(1);
        transition_events.push((
            "channel.peer.session_expired",
            json!({
                "runtime_state": state.runtime_state.clone(),
                "connectivity_state": state.connectivity_state.clone(),
                "binding_state": state.binding_state.clone(),
                "reason": "ttl_expired",
                "previous_session_id": state.session_id,
                "session_expires_at": state.session_expires_at,
            }),
        ));
        transition_events.push((
            "channel.peer.binding_unbound",
            json!({
                "runtime_state": state.runtime_state.clone(),
                "connectivity_state": state.connectivity_state.clone(),
                "binding_state": "unbound",
                "reason": "session_expired",
                "pairing_required": false,
            }),
        ));
        release_binding_state(&mut state);
        sync_compat_fields(&mut state);
        clear_target(runtime_home)?;
    }

    persist_state(runtime_home, &state)?;
    for (event_type, payload) in transition_events {
        append_peer_event(runtime_home, &mut state, event_type, payload, now.as_str())?;
    }
    persist_state(runtime_home, &state)?;
    Ok(state)
}

pub(crate) fn ensure_builtin_qqbot_binding(
    runtime_home: &Path,
    expected_session_id: Option<&str>,
) -> Result<GatewayPeerState, CliError> {
    let mut state = ensure_builtin_qqbot_peer(runtime_home)?;
    let Some(expected_session_id) = expected_session_id
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return Ok(state);
    };
    let Some(bound_session_id) = state.session_id.clone() else {
        return Ok(state);
    };
    if !state.session_valid || bound_session_id == expected_session_id {
        return Ok(state);
    }

    let now = local_timestamp_now();
    state.binding_state = "invalidated".into();
    sync_compat_fields(&mut state);
    state.updated_at = now.clone();
    state.reconnect_count = state.reconnect_count.saturating_add(1);

    let invalidated_payload = json!({
        "runtime_state": state.runtime_state.clone(),
        "connectivity_state": state.connectivity_state.clone(),
        "binding_state": state.binding_state.clone(),
        "reason": "binding_mismatch",
        "expected_session_id": expected_session_id,
        "previous_session_id": bound_session_id.clone(),
    });
    append_peer_event(
        runtime_home,
        &mut state,
        "channel.peer.session_invalidated",
        invalidated_payload,
        now.as_str(),
    )?;
    let binding_unbound_payload = json!({
        "runtime_state": state.runtime_state.clone(),
        "connectivity_state": state.connectivity_state.clone(),
        "binding_state": "unbound",
        "reason": "binding_mismatch",
        "pairing_required": false,
        "expected_session_id": expected_session_id,
        "previous_session_id": bound_session_id,
    });
    append_peer_event(
        runtime_home,
        &mut state,
        "channel.peer.binding_unbound",
        binding_unbound_payload,
        now.as_str(),
    )?;
    release_binding_state(&mut state);
    sync_compat_fields(&mut state);
    persist_state(runtime_home, &state)?;
    clear_target(runtime_home)?;
    Ok(state)
}

#[allow(dead_code)]
pub(crate) fn complete_builtin_qqbot_pairing(
    runtime_home: &Path,
    session_id: &str,
    ttl_minutes: Option<u64>,
) -> Result<GatewayPeerState, CliError> {
    let mut state = ensure_builtin_qqbot_peer(runtime_home)?;
    let now = local_timestamp_now();
    let ttl = ttl_minutes.map(|value| value.max(1));
    let expires_at = ttl
        .map(|minutes| add_minutes(now.as_str(), minutes))
        .transpose()?;

    state.binding_state = "bound".into();
    state.paired_at = Some(now.clone());
    state.session_id = Some(session_id.trim().to_string());
    state.session_ttl_minutes = ttl;
    state.session_expires_at = expires_at.clone();
    state.updated_at = now.clone();
    sync_compat_fields(&mut state);
    let pairing_payload = json!({
        "runtime_state": state.runtime_state.clone(),
        "connectivity_state": state.connectivity_state.clone(),
        "binding_state": state.binding_state.clone(),
        "session_id": state.session_id.clone(),
        "session_expires_at": expires_at,
        "ttl_minutes": ttl,
    });
    append_peer_event(
        runtime_home,
        &mut state,
        "channel.peer.pairing_completed",
        pairing_payload,
        now.as_str(),
    )?;
    persist_state(runtime_home, &state)?;
    Ok(state)
}

#[allow(dead_code)]
pub(crate) fn record_builtin_qqbot_heartbeat(
    runtime_home: &Path,
) -> Result<GatewayPeerState, CliError> {
    let mut state = ensure_builtin_qqbot_peer(runtime_home)?;
    let now = local_timestamp_now();
    state.last_heartbeat_at = Some(now.clone());
    state.updated_at = now.clone();
    let heartbeat_payload = json!({
        "runtime_state": state.runtime_state.clone(),
        "connectivity_state": state.connectivity_state.clone(),
        "binding_state": state.binding_state.clone(),
        "session_valid": state.session_valid,
        "session_id": state.session_id.clone(),
        "pairing_required": state.pairing_required,
    });
    append_peer_event(
        runtime_home,
        &mut state,
        "channel.peer.heartbeat_recorded",
        heartbeat_payload,
        now.as_str(),
    )?;
    persist_state(runtime_home, &state)?;
    Ok(state)
}

#[allow(dead_code)]
pub(crate) fn probe_builtin_qqbot_connectivity(
    runtime_home: &Path,
    user_toml_path: Option<&Path>,
) -> Result<GatewayPeerState, CliError> {
    let mut state = ensure_builtin_qqbot_peer(runtime_home)?;
    let now = local_timestamp_now();
    match probe_qqbot_upstream(user_toml_path) {
        Ok(result) => {
            let payload = apply_probe_success(&mut state, result);
            append_peer_event(
                runtime_home,
                &mut state,
                "channel.peer.upstream_authenticated",
                payload,
                now.as_str(),
            )?;
        }
        Err(failure) => {
            let payload = apply_probe_failure(&mut state, failure);
            append_peer_event(
                runtime_home,
                &mut state,
                "channel.peer.connectivity_probe_failed",
                payload,
                now.as_str(),
            )?;
        }
    }
    persist_state(runtime_home, &state)?;
    Ok(state)
}

#[allow(dead_code)]
pub(crate) fn force_expire_builtin_qqbot_session(
    runtime_home: &Path,
    reason: &str,
) -> Result<GatewayPeerState, CliError> {
    let mut state = ensure_builtin_qqbot_peer(runtime_home)?;
    let now = local_timestamp_now();
    if !state.session_valid {
        return Ok(state);
    }

    state.binding_state = "expired".into();
    sync_compat_fields(&mut state);
    state.reconnect_count = state.reconnect_count.saturating_add(1);
    state.updated_at = now.clone();

    let expired_payload = json!({
        "runtime_state": state.runtime_state.clone(),
        "connectivity_state": state.connectivity_state.clone(),
        "binding_state": state.binding_state.clone(),
        "reason": reason,
        "previous_session_id": state.session_id.clone(),
        "session_expires_at": state.session_expires_at.clone(),
    });
    append_peer_event(
        runtime_home,
        &mut state,
        "channel.peer.session_expired",
        expired_payload,
        now.as_str(),
    )?;
    let binding_unbound_payload = json!({
        "runtime_state": state.runtime_state.clone(),
        "connectivity_state": state.connectivity_state.clone(),
        "binding_state": "unbound",
        "reason": reason,
        "pairing_required": false,
    });
    append_peer_event(
        runtime_home,
        &mut state,
        "channel.peer.binding_unbound",
        binding_unbound_payload,
        now.as_str(),
    )?;
    release_binding_state(&mut state);
    sync_compat_fields(&mut state);
    persist_state(runtime_home, &state)?;
    clear_target(runtime_home)?;
    Ok(state)
}

pub(crate) fn active_builtin_qqbot_session(
    runtime_home: &Path,
) -> Result<Option<String>, CliError> {
    let state = ensure_builtin_qqbot_peer(runtime_home)?;
    if state.session_valid {
        Ok(state.session_id)
    } else {
        Ok(None)
    }
}

pub(crate) fn record_builtin_qqbot_runtime_event(
    runtime_home: &Path,
    event_type: &str,
    runtime_state: Option<&str>,
    connectivity_state: Option<&str>,
    binding_state: Option<&str>,
    payload: Value,
) -> Result<GatewayPeerState, CliError> {
    let mut state = ensure_builtin_qqbot_peer(runtime_home)?;
    let now = local_timestamp_now();
    if let Some(value) = runtime_state {
        state.runtime_state = value.to_string();
    }
    if let Some(value) = connectivity_state {
        state.connectivity_state = value.to_string();
    }
    if let Some(value) = binding_state {
        state.binding_state = value.to_string();
        sync_compat_fields(&mut state);
    }
    state.updated_at = now.clone();
    append_peer_event(runtime_home, &mut state, event_type, payload, now.as_str())?;
    persist_state(runtime_home, &state)?;
    Ok(state)
}
