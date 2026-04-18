use crate::{CliError, channel_peer::GatewayPeerState};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{fs, io::Write, path::Path};

const QQBOT_PEER_ID: &str = "peer-channel-gateway-qqbot-local";
const QQBOT_PEER_KIND: &str = "channel_gateway.qqbot";
const PEER_PROTOCOL_VERSION: &str = "fin.peer.m1";

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
struct PeerRegistry {
    #[serde(default)]
    peers: Vec<PeerRegistryEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct PeerRegistryEntry {
    peer_id: String,
    peer_kind: String,
    presence_state: String,
    #[serde(default)]
    runtime_state: Option<String>,
    #[serde(default)]
    connectivity_state: Option<String>,
    #[serde(default)]
    binding_state: Option<String>,
    lifecycle_state: String,
    updated_at: String,
    #[serde(default)]
    pairing_required: Option<bool>,
    #[serde(default)]
    session_valid: Option<bool>,
    #[serde(default)]
    session_id: Option<String>,
    #[serde(default)]
    last_heartbeat_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct GatewayPeerEvent {
    event_id: String,
    event_type: String,
    sender: String,
    source: String,
    protocol_version: String,
    sequence: u64,
    occurred_at: String,
    peer_id: String,
    payload: Value,
}

pub(super) fn persist_state(runtime_home: &Path, state: &GatewayPeerState) -> Result<(), CliError> {
    let state_path = runtime_home.join("runtime/peers/qqbot/state.json");
    write_json(&state_path, state)?;
    ensure_peer_registry(runtime_home, state)
}

pub(super) fn read_json<T: for<'de> Deserialize<'de>>(path: &Path) -> Result<Option<T>, CliError> {
    match fs::read_to_string(path) {
        Ok(content) => serde_json::from_str(&content)
            .map(Some)
            .map_err(CliError::Serialize),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(source) => Err(CliError::ReadFile {
            path: path.display().to_string(),
            source,
        }),
    }
}

pub(super) fn append_peer_event(
    runtime_home: &Path,
    state: &mut GatewayPeerState,
    event_type: &str,
    payload: Value,
    occurred_at: &str,
) -> Result<(), CliError> {
    let event = GatewayPeerEvent {
        event_id: format!("peer-evt-{:06}", state.next_event_sequence),
        event_type: event_type.into(),
        sender: QQBOT_PEER_ID.into(),
        source: "cli.channel_peer".into(),
        protocol_version: PEER_PROTOCOL_VERSION.into(),
        sequence: state.next_event_sequence,
        occurred_at: occurred_at.to_string(),
        peer_id: QQBOT_PEER_ID.into(),
        payload,
    };
    let events_path = runtime_home.join("runtime/peers/qqbot/events.jsonl");
    if let Some(parent) = events_path.parent() {
        fs::create_dir_all(parent).map_err(|source| CliError::WriteFile {
            path: parent.display().to_string(),
            source,
        })?;
    }
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&events_path)
        .map_err(|source| CliError::WriteFile {
            path: events_path.display().to_string(),
            source,
        })?;
    let line = format!(
        "{}\n",
        serde_json::to_string(&event).map_err(CliError::Serialize)?
    );
    file.write_all(line.as_bytes())
        .map_err(|source| CliError::WriteFile {
            path: events_path.display().to_string(),
            source,
        })?;
    state.next_event_sequence = state.next_event_sequence.saturating_add(1);
    Ok(())
}

fn ensure_peer_registry(runtime_home: &Path, state: &GatewayPeerState) -> Result<(), CliError> {
    let peers_dir = runtime_home.join("runtime/peers");
    fs::create_dir_all(&peers_dir).map_err(|source| CliError::WriteFile {
        path: peers_dir.display().to_string(),
        source,
    })?;
    let registry_path = peers_dir.join("registry.json");
    let mut registry = read_json::<PeerRegistry>(&registry_path)?.unwrap_or_default();
    if let Some(existing) = registry
        .peers
        .iter_mut()
        .find(|entry| entry.peer_id == QQBOT_PEER_ID)
    {
        existing.peer_kind = QQBOT_PEER_KIND.into();
        existing.presence_state = "online".into();
        existing.runtime_state = Some(state.runtime_state.clone());
        existing.connectivity_state = Some(state.connectivity_state.clone());
        existing.binding_state = Some(state.binding_state.clone());
        existing.lifecycle_state = state.lifecycle_state.clone();
        existing.updated_at = state.updated_at.clone();
        existing.pairing_required = Some(state.pairing_required);
        existing.session_valid = Some(state.session_valid);
        existing.session_id = state.session_id.clone();
        existing.last_heartbeat_at = state.last_heartbeat_at.clone();
    } else {
        registry.peers.push(PeerRegistryEntry {
            peer_id: QQBOT_PEER_ID.into(),
            peer_kind: QQBOT_PEER_KIND.into(),
            presence_state: "online".into(),
            runtime_state: Some(state.runtime_state.clone()),
            connectivity_state: Some(state.connectivity_state.clone()),
            binding_state: Some(state.binding_state.clone()),
            lifecycle_state: state.lifecycle_state.clone(),
            updated_at: state.updated_at.clone(),
            pairing_required: Some(state.pairing_required),
            session_valid: Some(state.session_valid),
            session_id: state.session_id.clone(),
            last_heartbeat_at: state.last_heartbeat_at.clone(),
        });
    }
    write_json(&registry_path, &registry)
}

fn write_json<T: Serialize>(path: &Path, value: &T) -> Result<(), CliError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|source| CliError::WriteFile {
            path: parent.display().to_string(),
            source,
        })?;
    }
    fs::write(
        path,
        serde_json::to_vec_pretty(value).map_err(CliError::Serialize)?,
    )
    .map_err(|source| CliError::WriteFile {
        path: path.display().to_string(),
        source,
    })
}
