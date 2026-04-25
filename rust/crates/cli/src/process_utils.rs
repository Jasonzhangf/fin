use crate::CliError;
use std::{
    env, fs,
    path::{Path, PathBuf},
    process::Command as ProcessCommand,
    time::{SystemTime, UNIX_EPOCH},
};

const PLAIN_LOG_TAIL_BYTES_LIMIT: usize = 512 * 1024;

pub(crate) fn repo_root() -> Result<PathBuf, CliError> {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../..")
        .canonicalize()
        .map_err(|source| CliError::ReadFile {
            path: "repo root".into(),
            source,
        })
}

pub(crate) fn short_git_sha() -> Option<String> {
    let repo_root = repo_root().ok()?;
    let output = ProcessCommand::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .current_dir(repo_root)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

pub(crate) fn now_unix_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time should be after epoch")
        .as_secs()
}

pub(crate) fn run_process_and_log(
    mut command: ProcessCommand,
    label: &str,
    log_path: &Path,
) -> Result<(), CliError> {
    append_log(log_path, &format!("$ [{}] {:?}\n", label, command))?;
    let output = command.output().map_err(|source| CliError::ReadFile {
        path: format!("process: {label}"),
        source,
    })?;
    append_log(log_path, &String::from_utf8_lossy(&output.stdout))?;
    append_log(log_path, &String::from_utf8_lossy(&output.stderr))?;
    if !output.status.success() {
        return Err(CliError::ProcessFailed {
            command: label.into(),
            exit_code: output.status.code(),
        });
    }
    Ok(())
}

pub(crate) fn append_log(path: &Path, line: &str) -> Result<(), CliError> {
    use std::io::Write;

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|source| CliError::WriteFile {
            path: parent.display().to_string(),
            source,
        })?;
    }
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|source| CliError::WriteFile {
            path: path.display().to_string(),
            source,
        })?;
    file.write_all(line.as_bytes())
        .map_err(|source| CliError::WriteFile {
            path: path.display().to_string(),
            source,
        })?;
    trim_file_to_tail_bytes(path, PLAIN_LOG_TAIL_BYTES_LIMIT)
}

fn trim_file_to_tail_bytes(path: &Path, limit: usize) -> Result<(), CliError> {
    let bytes = fs::read(path).map_err(|source| CliError::ReadFile {
        path: path.display().to_string(),
        source,
    })?;
    if bytes.len() <= limit {
        return Ok(());
    }
    let keep_from = bytes.len() - limit;
    fs::write(path, &bytes[keep_from..]).map_err(|source| CliError::WriteFile {
        path: path.display().to_string(),
        source,
    })
}

pub(crate) fn file_checksum_hex(path: &Path) -> Result<String, CliError> {
    let bytes = fs::read(path).map_err(|source| CliError::ReadFile {
        path: path.display().to_string(),
        source,
    })?;
    let checksum = bytes.iter().fold(0_u64, |acc, byte| {
        acc.wrapping_mul(16777619) ^ u64::from(*byte)
    });
    Ok(format!("{checksum:016x}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_log_path() -> PathBuf {
        std::env::temp_dir().join(format!(
            "fin-process-utils-log-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("time")
                .as_nanos()
        ))
    }

    #[test]
    fn append_log_trims_to_recent_tail_bytes() {
        let path = temp_log_path();
        append_log(&path, &"a".repeat(PLAIN_LOG_TAIL_BYTES_LIMIT)).expect("seed log");
        append_log(&path, "tail").expect("append tail");

        let bytes = fs::read(&path).expect("read log");
        assert_eq!(bytes.len(), PLAIN_LOG_TAIL_BYTES_LIMIT);
        assert!(String::from_utf8_lossy(&bytes).ends_with("tail"));
    }
}
