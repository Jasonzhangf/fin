use crate::{CliError, fs_utils::write_file, process_utils::now_unix_seconds};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct BuildSequenceState {
    pub(crate) major: u16,
    pub(crate) minor: u16,
    pub(crate) last_build: u32,
    pub(crate) updated_at: u64,
}

impl Default for BuildSequenceState {
    fn default() -> Self {
        Self {
            major: 0,
            minor: 1,
            last_build: 0,
            updated_at: 0,
        }
    }
}

impl BuildSequenceState {
    fn next_build_version(&mut self) -> String {
        self.last_build += 1;
        self.updated_at = now_unix_seconds();
        format!("{}.{}.{:04}", self.major, self.minor, self.last_build)
    }
}

fn build_sequence_path(runtime_home: &Path) -> PathBuf {
    runtime_home.join("install/build-seq.json")
}

fn load_build_sequence_state(runtime_home: &Path) -> Result<BuildSequenceState, CliError> {
    let path = build_sequence_path(runtime_home);
    match fs::read_to_string(&path) {
        Ok(content) => Ok(serde_json::from_str(&content)?),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(BuildSequenceState::default()),
        Err(source) => Err(CliError::ReadFile {
            path: path.display().to_string(),
            source,
        }),
    }
}

pub(crate) fn parse_build_version(raw: &str) -> Result<(u16, u16, u32), CliError> {
    let parts: Vec<_> = raw.trim().split('.').collect();
    if parts.len() != 3 {
        return Err(CliError::InvalidBuildVersion(raw.into()));
    }
    let major = parts[0]
        .parse::<u16>()
        .map_err(|_| CliError::InvalidBuildVersion(raw.into()))?;
    let minor = parts[1]
        .parse::<u16>()
        .map_err(|_| CliError::InvalidBuildVersion(raw.into()))?;
    let build = parts[2]
        .parse::<u32>()
        .map_err(|_| CliError::InvalidBuildVersion(raw.into()))?;
    if build == 0 || parts[2].len() != 4 {
        return Err(CliError::InvalidBuildVersion(raw.into()));
    }
    Ok((major, minor, build))
}

fn persist_build_sequence_state(
    runtime_home: &Path,
    state: &BuildSequenceState,
) -> Result<(), CliError> {
    write_file(
        &build_sequence_path(runtime_home),
        serde_json::to_vec_pretty(state)?.as_slice(),
    )
}

pub(crate) fn resolve_build_version(
    runtime_home: &Path,
    build_version_override: Option<String>,
) -> Result<String, CliError> {
    let mut state = load_build_sequence_state(runtime_home)?;
    match build_version_override {
        Some(build_version) => {
            let (major, minor, build) = parse_build_version(&build_version)?;
            if major != state.major || minor != state.minor {
                return Err(CliError::InvalidBuildVersion(build_version));
            }
            if build > state.last_build {
                state.last_build = build;
            }
            state.updated_at = now_unix_seconds();
            persist_build_sequence_state(runtime_home, &state)?;
            Ok(build_version)
        }
        None => {
            let build_version = state.next_build_version();
            persist_build_sequence_state(runtime_home, &state)?;
            Ok(build_version)
        }
    }
}
