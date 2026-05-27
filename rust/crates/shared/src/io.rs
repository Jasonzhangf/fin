use serde::{Serialize, de::DeserializeOwned};
use std::fs;
use std::io::Write;
use std::path::Path;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum SharedIoError {
    #[error("io error at '{path}': {source}")]
    Io {
        path: String,
        #[source]
        source: std::io::Error,
    },
    #[error("json error at '{path}': {source}")]
    Json {
        path: String,
        #[source]
        source: serde_json::Error,
    },
}

pub fn read_json_or_empty<T: DeserializeOwned>(path: &Path) -> Result<Vec<T>, SharedIoError> {
    match fs::read_to_string(path) {
        Ok(content) => serde_json::from_str(&content).map_err(|source| SharedIoError::Json {
            path: path.display().to_string(),
            source,
        }),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(source) => Err(SharedIoError::Io {
            path: path.display().to_string(),
            source,
        }),
    }
}

pub fn write_json<T: Serialize>(path: &Path, value: &T) -> Result<(), SharedIoError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|source| SharedIoError::Io {
            path: parent.display().to_string(),
            source,
        })?;
    }
    let bytes = serde_json::to_vec_pretty(value).map_err(|source| SharedIoError::Json {
        path: path.display().to_string(),
        source,
    })?;
    fs::write(path, bytes).map_err(|source| SharedIoError::Io {
        path: path.display().to_string(),
        source,
    })
}

pub fn append_jsonl<T: Serialize>(path: &Path, value: &T) -> Result<(), SharedIoError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|source| SharedIoError::Io {
            path: parent.display().to_string(),
            source,
        })?;
    }
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|source| SharedIoError::Io {
            path: path.display().to_string(),
            source,
        })?;
    let line = serde_json::to_string(value).map_err(|source| SharedIoError::Json {
        path: path.display().to_string(),
        source,
    })?;
    writeln!(file, "{line}").map_err(|source| SharedIoError::Io {
        path: path.display().to_string(),
        source,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::{Deserialize, Serialize};
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    struct Item {
        name: String,
    }

    static TEMP_SEQ: AtomicU64 = AtomicU64::new(1);

    fn temp_path(name: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!(
            "fin-shared-io-{name}-{}-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("time")
                .as_nanos(),
            TEMP_SEQ.fetch_add(1, Ordering::Relaxed),
        ))
    }

    #[test]
    fn read_json_or_empty_returns_empty_for_missing_file() {
        let path = temp_path("missing");
        let values: Vec<Item> = read_json_or_empty(&path).expect("read missing");
        assert!(values.is_empty());
    }

    #[test]
    fn write_json_then_read_json_or_empty_roundtrips() {
        let path = temp_path("roundtrip");
        let values = vec![Item { name: "a".into() }, Item { name: "b".into() }];
        write_json(&path, &values).expect("write json");
        let loaded: Vec<Item> = read_json_or_empty(&path).expect("read json");
        assert_eq!(loaded, values);
    }

    #[test]
    fn append_jsonl_appends_lines() {
        let path = temp_path("jsonl");
        append_jsonl(&path, &Item { name: "a".into() }).expect("append first");
        append_jsonl(&path, &Item { name: "b".into() }).expect("append second");
        let content = fs::read_to_string(&path).expect("read file");
        let lines = content.lines().collect::<Vec<_>>();
        assert_eq!(lines.len(), 2);
        assert!(lines[0].contains("\"a\""));
        assert!(lines[1].contains("\"b\""));
    }
}
