use super::dispatch::runtime_home_from_context;
use fin_contracts::DaemonEnsurePeerRequestRecord;
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(super) struct PeerLeaseRecord {
    pub(super) peer_id: String,
    pub(super) peer_kind: String,
    pub(super) lease_ttl_ms: u64,
    pub(super) issued_at: String,
    pub(super) expires_at_hint_ms: u64,
    pub(super) owner: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(super) struct PeerStateRecord {
    pub(super) peer_id: String,
    pub(super) peer_kind: String,
    pub(super) lifecycle_state: String,
    pub(super) last_heartbeat_at: String,
    pub(super) reconnect_backoff_ms: u64,
}

pub(super) fn upsert_ensure_request(
    path: &Path,
    request: &DaemonEnsurePeerRequestRecord,
) -> Result<(), String> {
    let mut requests = match fs::read_to_string(path) {
        Ok(content) => serde_json::from_str::<Vec<DaemonEnsurePeerRequestRecord>>(&content)
            .map_err(|err| err.to_string())?,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Vec::new(),
        Err(err) => return Err(err.to_string()),
    };
    if let Some(existing) = requests
        .iter_mut()
        .find(|item| item.peer_id == request.peer_id)
    {
        *existing = request.clone();
    } else {
        requests.push(request.clone());
    }
    write_json(path, &requests)
}

pub(super) fn write_json<T: Serialize>(path: &Path, value: &T) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|err| err.to_string())?;
    }
    fs::write(
        path,
        serde_json::to_vec_pretty(value).map_err(|err| err.to_string())?,
    )
    .map_err(|err| err.to_string())
}

pub(super) fn sanitize_id(raw: &str) -> String {
    raw.chars()
        .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '-' })
        .collect::<String>()
        .trim_matches('-')
        .to_string()
}

pub(super) fn relative_artifact(
    context: &fin_contracts::MinimalContextView,
    absolute: &Path,
) -> String {
    runtime_home_from_context(context)
        .and_then(|home: PathBuf| {
            absolute
                .strip_prefix(home)
                .ok()
                .map(|value| value.to_string_lossy().to_string())
        })
        .unwrap_or_else(|| absolute.display().to_string())
}
