use crate::{
    CliError,
    channel_peer_connectivity::{apply_probe_failure, apply_probe_success, probe_qqbot_upstream},
    channel_peer_store::{append_peer_event, persist_state, read_json},
    time::local_timestamp_now,
};
use chrono::{DateTime, Duration, FixedOffset};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{fs, path::Path};

const QQBOT_PEER_ID: &str = "peer-channel-gateway-qqbot-local";
const QQBOT_PEER_KIND: &str = "channel_gateway.qqbot";
#[allow(dead_code)]
const DEFAULT_PAIRING_TTL_MINUTES: u64 = 30;

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
            "channel.peer.pairing_required",
            json!({
                "runtime_state": state.runtime_state.clone(),
                "connectivity_state": state.connectivity_state.clone(),
                "binding_state": state.binding_state.clone(),
                "reason": "initial_bootstrap",
                "pairing_required": true,
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
            "channel.peer.pairing_required",
            json!({
                "runtime_state": state.runtime_state.clone(),
                "connectivity_state": state.connectivity_state.clone(),
                "binding_state": "pairing_required",
                "reason": "session_expired",
                "pairing_required": true,
            }),
        ));
        state.binding_state = "pairing_required".into();
        sync_compat_fields(&mut state);
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
        "previous_session_id": bound_session_id,
    });
    append_peer_event(
        runtime_home,
        &mut state,
        "channel.peer.session_invalidated",
        invalidated_payload,
        now.as_str(),
    )?;
    let pairing_required_payload = json!({
        "runtime_state": state.runtime_state.clone(),
        "connectivity_state": state.connectivity_state.clone(),
        "binding_state": "pairing_required",
        "reason": "binding_mismatch",
        "pairing_required": true,
        "expected_session_id": expected_session_id,
    });
    append_peer_event(
        runtime_home,
        &mut state,
        "channel.peer.pairing_required",
        pairing_required_payload,
        now.as_str(),
    )?;
    state.binding_state = "pairing_required".into();
    sync_compat_fields(&mut state);
    persist_state(runtime_home, &state)?;
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
    let ttl = ttl_minutes.unwrap_or(DEFAULT_PAIRING_TTL_MINUTES).max(1);
    let expires_at = add_minutes(now.as_str(), ttl)?;

    state.binding_state = "bound".into();
    state.paired_at = Some(now.clone());
    state.session_id = Some(session_id.trim().to_string());
    state.session_ttl_minutes = Some(ttl);
    state.session_expires_at = Some(expires_at.clone());
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
) -> Result<GatewayPeerState, CliError> {
    let mut state = ensure_builtin_qqbot_peer(runtime_home)?;
    let now = local_timestamp_now();
    match probe_qqbot_upstream() {
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
    let pairing_required_payload = json!({
        "runtime_state": state.runtime_state.clone(),
        "connectivity_state": state.connectivity_state.clone(),
        "binding_state": "pairing_required",
        "reason": reason,
        "pairing_required": true,
    });
    append_peer_event(
        runtime_home,
        &mut state,
        "channel.peer.pairing_required",
        pairing_required_payload,
        now.as_str(),
    )?;
    state.binding_state = "pairing_required".into();
    sync_compat_fields(&mut state);
    persist_state(runtime_home, &state)?;
    Ok(state)
}

fn session_is_expired(state: &GatewayPeerState, now: &str) -> bool {
    let Some(expires_at) = state.session_expires_at.as_deref() else {
        return false;
    };
    let Some(expiry) = parse_local_ts(expires_at) else {
        return false;
    };
    let Some(now_value) = parse_local_ts(now) else {
        return false;
    };
    now_value >= expiry
}

#[allow(dead_code)]
fn add_minutes(ts: &str, minutes: u64) -> Result<String, CliError> {
    let Some(parsed) = parse_local_ts(ts) else {
        return Ok(ts.to_string());
    };
    let next = parsed + Duration::minutes(minutes as i64);
    Ok(next.format("%Y-%m-%dT%H:%M:%S%:z").to_string())
}

fn parse_local_ts(ts: &str) -> Option<DateTime<FixedOffset>> {
    DateTime::parse_from_rfc3339(ts).ok()
}

impl GatewayPeerState {
    fn new_unpaired(now: &str) -> Self {
        let mut state = Self {
            peer_id: QQBOT_PEER_ID.into(),
            peer_kind: QQBOT_PEER_KIND.into(),
            runtime_state: "ready_local".into(),
            connectivity_state: "local_only".into(),
            binding_state: "pairing_required".into(),
            lifecycle_state: String::new(),
            pairing_required: false,
            session_valid: false,
            started_at: now.into(),
            updated_at: now.into(),
            paired_at: None,
            session_id: None,
            session_expires_at: None,
            session_ttl_minutes: None,
            last_heartbeat_at: None,
            connectivity_checked_at: None,
            upstream_authenticated_at: None,
            upstream_expires_at: None,
            credential_source: None,
            last_connectivity_error: None,
            reconnect_count: 0,
            next_event_sequence: default_next_event_sequence(),
        };
        sync_compat_fields(&mut state);
        state
    }
}

fn repair_state_defaults(state: &mut GatewayPeerState, now: &str) {
    if state.runtime_state.trim().is_empty() {
        state.runtime_state = "ready_local".into();
    }
    if state.connectivity_state.trim().is_empty() {
        state.connectivity_state = "local_only".into();
    }
    if state.binding_state.trim().is_empty() {
        state.binding_state = if state.session_valid {
            "bound".into()
        } else if state.pairing_required {
            "pairing_required".into()
        } else {
            "unpaired".into()
        };
    }
    if state.started_at.trim().is_empty() {
        state.started_at = now.into();
    }
    sync_compat_fields(state);
}

fn sync_compat_fields(state: &mut GatewayPeerState) {
    state.lifecycle_state = match state.binding_state.as_str() {
        "bound" => "paired_active".into(),
        "expired" => "session_expired".into(),
        "invalidated" => "binding_invalidated".into(),
        _ => "idle_unpaired".into(),
    };
    state.pairing_required = !matches!(state.binding_state.as_str(), "bound");
    state.session_valid = matches!(state.binding_state.as_str(), "bound");
}
