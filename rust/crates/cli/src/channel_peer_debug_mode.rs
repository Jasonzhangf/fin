use crate::{CliError, time::local_timestamp_now};
use serde::{Deserialize, Serialize};
use std::{fs, path::Path};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
struct QqbotDebugModeState {
    #[serde(default)]
    enabled: bool,
    #[serde(default)]
    updated_at: Option<String>,
}

fn debug_mode_path(runtime_home: &Path) -> std::path::PathBuf {
    runtime_home.join("runtime/channels/qqbot/debug_mode.json")
}

pub(crate) fn qqbot_debug_enabled(runtime_home: &Path) -> Result<bool, CliError> {
    let path = debug_mode_path(runtime_home);
    match fs::read_to_string(&path) {
        Ok(content) => serde_json::from_str::<QqbotDebugModeState>(&content)
            .map(|state| state.enabled)
            .map_err(CliError::Serialize),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(source) => Err(CliError::ReadFile {
            path: path.display().to_string(),
            source,
        }),
    }
}

pub(crate) fn set_qqbot_debug_enabled(runtime_home: &Path, enabled: bool) -> Result<(), CliError> {
    let path = debug_mode_path(runtime_home);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|source| CliError::WriteFile {
            path: parent.display().to_string(),
            source,
        })?;
    }
    let state = QqbotDebugModeState {
        enabled,
        updated_at: Some(local_timestamp_now()),
    };
    fs::write(
        &path,
        serde_json::to_vec_pretty(&state).map_err(CliError::Serialize)?,
    )
    .map_err(|source| CliError::WriteFile {
        path: path.display().to_string(),
        source,
    })
}
