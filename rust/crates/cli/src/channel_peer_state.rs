use super::GatewayPeerState;
use crate::CliError;
use chrono::{DateTime, Duration, FixedOffset};

pub(super) fn session_is_expired(state: &GatewayPeerState, now: &str) -> bool {
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

pub(super) fn add_minutes(ts: &str, minutes: u64) -> Result<String, CliError> {
    let Some(parsed) = parse_local_ts(ts) else {
        return Ok(ts.to_string());
    };
    let next = parsed + Duration::minutes(minutes as i64);
    Ok(next.format("%Y-%m-%dT%H:%M:%S%:z").to_string())
}

pub(super) fn repair_state_defaults(state: &mut GatewayPeerState, now: &str) {
    if state.runtime_state.trim().is_empty() {
        state.runtime_state = "ready_local".into();
    }
    if state.connectivity_state.trim().is_empty() {
        state.connectivity_state = "local_only".into();
    }
    if state.binding_state.trim().is_empty() {
        state.binding_state = if state.session_valid {
            "bound".into()
        } else {
            "unbound".into()
        };
    } else if state.binding_state == "pairing_required" && !state.session_valid {
        release_binding_state(state);
    }
    if state.binding_state == "unbound" && !state.session_valid {
        state.session_id = None;
        state.session_expires_at = None;
        state.session_ttl_minutes = None;
    }
    if state.started_at.trim().is_empty() {
        state.started_at = now.into();
    }
    sync_compat_fields(state);
}

pub(super) fn sync_compat_fields(state: &mut GatewayPeerState) {
    state.lifecycle_state = match state.binding_state.as_str() {
        "bound" => "paired_active".into(),
        "expired" => "session_expired".into(),
        "invalidated" => "binding_invalidated".into(),
        _ => {
            if matches!(
                state.connectivity_state.as_str(),
                "connected" | "connecting" | "auth_required" | "auth_failed" | "degraded"
            ) || state.upstream_authenticated_at.is_some()
            {
                "idle_ready".into()
            } else {
                "idle_unpaired".into()
            }
        }
    };
    state.pairing_required = matches!(state.binding_state.as_str(), "pairing_required");
    state.session_valid = matches!(state.binding_state.as_str(), "bound");
}

pub(super) fn release_binding_state(state: &mut GatewayPeerState) {
    state.binding_state = "unbound".into();
    state.session_id = None;
    state.session_expires_at = None;
    state.session_ttl_minutes = None;
}

fn parse_local_ts(ts: &str) -> Option<DateTime<FixedOffset>> {
    DateTime::parse_from_rfc3339(ts).ok()
}

impl GatewayPeerState {
    pub(super) fn new_unpaired(now: &str) -> Self {
        let mut state = Self {
            peer_id: super::QQBOT_PEER_ID.into(),
            peer_kind: super::QQBOT_PEER_KIND.into(),
            runtime_state: "ready_local".into(),
            connectivity_state: "local_only".into(),
            binding_state: "unbound".into(),
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
            next_event_sequence: super::default_next_event_sequence(),
        };
        sync_compat_fields(&mut state);
        state
    }
}
