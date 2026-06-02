use crate::CliError;
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

#[derive(Debug, Clone, Default)]
pub(crate) struct RuntimeCurrentSnapshot {
    files: BTreeMap<PathBuf, Vec<u8>>,
}

pub(crate) fn with_runtime_current_snapshot<T, F>(
    runtime_home: &Path,
    action: F,
) -> Result<T, CliError>
where
    F: FnOnce() -> Result<T, CliError>,
{
    let snapshot = capture_runtime_current_snapshot(runtime_home)?;
    let result = action();
    restore_runtime_current_snapshot(runtime_home, &snapshot)?;
    result
}

fn capture_runtime_current_snapshot(
    runtime_home: &Path,
) -> Result<RuntimeCurrentSnapshot, CliError> {
    let current_dir = runtime_home.join("runtime/current");
    let mut files = BTreeMap::new();
    collect_files(&current_dir, &current_dir, &mut files)?;
    Ok(RuntimeCurrentSnapshot { files })
}

fn restore_runtime_current_snapshot(
    runtime_home: &Path,
    snapshot: &RuntimeCurrentSnapshot,
) -> Result<(), CliError> {
    let current_dir = runtime_home.join("runtime/current");
    if current_dir.exists() {
        fs::remove_dir_all(&current_dir).map_err(|source| CliError::WriteFile {
            path: current_dir.display().to_string(),
            source,
        })?;
    }
    fs::create_dir_all(&current_dir).map_err(|source| CliError::WriteFile {
        path: current_dir.display().to_string(),
        source,
    })?;
    for (relative, bytes) in &snapshot.files {
        let path = current_dir.join(relative);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|source| CliError::WriteFile {
                path: parent.display().to_string(),
                source,
            })?;
        }
        fs::write(&path, bytes).map_err(|source| CliError::WriteFile {
            path: path.display().to_string(),
            source,
        })?;
    }
    Ok(())
}

fn collect_files(
    root: &Path,
    dir: &Path,
    files: &mut BTreeMap<PathBuf, Vec<u8>>,
) -> Result<(), CliError> {
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(source) => {
            return Err(CliError::ReadFile {
                path: dir.display().to_string(),
                source,
            });
        }
    };
    for entry in entries {
        let entry = entry.map_err(|source| CliError::ReadFile {
            path: dir.display().to_string(),
            source,
        })?;
        let path = entry.path();
        if path.is_dir() {
            collect_files(root, &path, files)?;
            continue;
        }
        let relative = path
            .strip_prefix(root)
            .map_err(|_| CliError::Usage)?
            .to_path_buf();
        let bytes = fs::read(&path).map_err(|source| CliError::ReadFile {
            path: path.display().to_string(),
            source,
        })?;
        files.insert(relative, bytes);
    }
    Ok(())
}
