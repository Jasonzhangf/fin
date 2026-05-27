use fin_shared::{AgentIdentity, AgentRunRecord};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
};

pub(crate) fn read_json<T: for<'de> Deserialize<'de>>(path: &Path) -> Result<Option<T>, String> {
    match fs::read_to_string(path) {
        Ok(content) => serde_json::from_str::<T>(&content)
            .map(Some)
            .map_err(|err| err.to_string()),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(err) => Err(err.to_string()),
    }
}

pub(crate) fn write_json<T: Serialize + ?Sized>(path: &Path, value: &T) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|err| err.to_string())?;
    }
    fs::write(
        path,
        serde_json::to_vec_pretty(value).map_err(|err| err.to_string())?,
    )
    .map_err(|err| err.to_string())
}

pub(crate) fn safe_file_name(value: &str) -> String {
    value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.') {
                ch
            } else {
                '_'
            }
        })
        .collect()
}

pub(crate) fn sanitize_id(value: &str) -> String {
    safe_file_name(value).trim_matches('_').to_string()
}

pub(crate) fn identities_path(root: &Path) -> PathBuf {
    root.join("identities.json")
}

pub(crate) fn runs_path(root: &Path) -> PathBuf {
    root.join("runs.json")
}

pub(crate) fn identity_path(root: &Path, agent_id: &str) -> PathBuf {
    root.join("identities")
        .join(format!("{}.json", safe_file_name(agent_id)))
}

pub(crate) fn run_path(root: &Path, agent_run_id: &str) -> PathBuf {
    root.join("runs")
        .join(format!("{}.json", safe_file_name(agent_run_id)))
}

pub(crate) fn mailbox_path(root: &Path, agent_id: &str) -> PathBuf {
    root.join("mailbox")
        .join(safe_file_name(agent_id))
        .join("inbox.json")
}

pub(crate) fn read_identity_required(root: &Path, agent_id: &str) -> Result<AgentIdentity, String> {
    read_json(&identity_path(root, agent_id))?
        .ok_or_else(|| format!("unknown agent identity: {agent_id}"))
}

pub(crate) fn read_run_required(root: &Path, agent_run_id: &str) -> Result<AgentRunRecord, String> {
    read_json(&run_path(root, agent_run_id))?
        .ok_or_else(|| format!("unknown agent run: {agent_run_id}"))
}
