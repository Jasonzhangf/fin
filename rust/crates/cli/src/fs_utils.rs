use crate::CliError;
use std::{fs, path::Path};

pub(crate) fn read_file(path: &Path) -> Result<String, CliError> {
    fs::read_to_string(path).map_err(|source| CliError::ReadFile {
        path: path.display().to_string(),
        source,
    })
}

pub(crate) fn write_file(path: &Path, bytes: &[u8]) -> Result<(), CliError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|source| CliError::WriteFile {
            path: parent.display().to_string(),
            source,
        })?;
    }
    fs::write(path, bytes).map_err(|source| CliError::WriteFile {
        path: path.display().to_string(),
        source,
    })
}
