use super::*;
use fin_config::RuntimeRetentionConfig;
use std::io::Write;

pub(crate) fn persist_event_stream(
    runtime_home: &Path,
    session_dir: &Path,
    year: &str,
    month: &str,
    session_id: &str,
    events: &[EventEnvelope<Value>],
    retention: &RuntimeRetentionConfig,
) -> Result<(), RuntimeError> {
    let stream_path = session_dir.join("events/stream.jsonl");
    let archive_dir = session_dir.join("events/archive");
    let cold_archive_dir = runtime_home
        .join("archive/sessions")
        .join(year)
        .join(month)
        .join(session_id)
        .join("events");
    super::create_dir_all(&archive_dir)?;
    super::create_dir_all(&cold_archive_dir)?;

    let mut lines = read_json_lines_or_empty(&stream_path)?;
    lines.extend(
        events
            .iter()
            .map(serde_json::to_string)
            .collect::<Result<Vec<_>, _>>()?,
    );

    if lines.len() > retention.session_event_hot_limit {
        let overflow = lines.len() - retention.session_event_hot_limit;
        let drained = lines.drain(0..overflow).collect::<Vec<_>>();
        if !drained.is_empty() {
            let next_segment_index =
                next_event_archive_segment_index(&archive_dir, &cold_archive_dir)?;
            let archive_path = archive_dir.join(format!("segment-{next_segment_index:06}.jsonl"));
            write_json_lines_raw(&archive_path, &drained)?;
        }
    }
    write_json_lines_raw(&stream_path, &lines)?;
    rebalance_event_archives(&archive_dir, &cold_archive_dir, retention)?;

    let local_segments = list_event_archive_paths(&archive_dir)?;
    let cold_segments = list_event_archive_paths(&cold_archive_dir)?;
    let index = serde_json::json!({
        "session_id": session_id,
        "live_stream_path": format!("sessions/{year}/{month}/{session_id}/events/stream.jsonl"),
        "local_archive_dir": format!("sessions/{year}/{month}/{session_id}/events/archive"),
        "cold_archive_dir": format!("archive/sessions/{year}/{month}/{session_id}/events"),
        "live_event_count": lines.len(),
        "local_archive_file_count": local_segments.len(),
        "cold_archive_file_count": cold_segments.len(),
    });
    super::write_json_file(&session_dir.join("events/archive_index.json"), &index)?;
    super::write_json_file(
        &runtime_home.join("runtime/current/current_event_archive_index.json"),
        &index,
    )?;
    Ok(())
}

pub(crate) fn session_archive_coords(
    session_dir: &Path,
) -> Result<(String, String, String), RuntimeError> {
    let session_id = session_dir
        .file_name()
        .and_then(|value| value.to_str())
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| invalid_session_dir(session_dir))?
        .to_string();
    let month = session_dir
        .parent()
        .and_then(|value| value.file_name())
        .and_then(|value| value.to_str())
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| invalid_session_dir(session_dir))?
        .to_string();
    let year = session_dir
        .parent()
        .and_then(Path::parent)
        .and_then(|value| value.file_name())
        .and_then(|value| value.to_str())
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| invalid_session_dir(session_dir))?
        .to_string();
    Ok((year, month, session_id))
}

fn invalid_session_dir(session_dir: &Path) -> RuntimeError {
    RuntimeError::Io {
        path: session_dir.display().to_string(),
        source: std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "session_dir must look like sessions/<year>/<month>/<session_id>",
        ),
    }
}

fn write_json_lines_raw(path: &Path, lines: &[String]) -> Result<(), RuntimeError> {
    let mut file = fs::File::create(path).map_err(|source| RuntimeError::Io {
        path: path.display().to_string(),
        source,
    })?;
    for line in lines {
        writeln!(file, "{line}").map_err(|source| RuntimeError::Io {
            path: path.display().to_string(),
            source,
        })?;
    }
    file.flush().map_err(|source| RuntimeError::Io {
        path: path.display().to_string(),
        source,
    })
}

fn read_json_lines_or_empty(path: &Path) -> Result<Vec<String>, RuntimeError> {
    match fs::read_to_string(path) {
        Ok(content) => Ok(content
            .lines()
            .filter(|line| !line.trim().is_empty())
            .map(str::to_string)
            .collect()),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(source) => Err(RuntimeError::Io {
            path: path.display().to_string(),
            source,
        }),
    }
}

fn list_event_archive_paths(dir: &Path) -> Result<Vec<PathBuf>, RuntimeError> {
    let mut entries = match fs::read_dir(dir) {
        Ok(entries) => entries
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| path.extension().and_then(|value| value.to_str()) == Some("jsonl"))
            .collect::<Vec<_>>(),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Vec::new(),
        Err(source) => {
            return Err(RuntimeError::Io {
                path: dir.display().to_string(),
                source,
            });
        }
    };
    entries.sort();
    Ok(entries)
}

fn next_event_archive_segment_index(
    local_dir: &Path,
    cold_dir: &Path,
) -> Result<u64, RuntimeError> {
    let mut max_index = 0u64;
    for path in list_event_archive_paths(local_dir)?
        .into_iter()
        .chain(list_event_archive_paths(cold_dir)?.into_iter())
    {
        let Some(stem) = path.file_stem().and_then(|value| value.to_str()) else {
            continue;
        };
        let Some(index_text) = stem.strip_prefix("segment-") else {
            continue;
        };
        if let Ok(value) = index_text.parse::<u64>() {
            max_index = max_index.max(value);
        }
    }
    Ok(max_index + 1)
}

fn rebalance_event_archives(
    local_dir: &Path,
    cold_dir: &Path,
    retention: &RuntimeRetentionConfig,
) -> Result<(), RuntimeError> {
    let local_paths = list_event_archive_paths(local_dir)?;
    if local_paths.len() <= retention.session_event_local_archive_file_limit {
        return Ok(());
    }
    let move_count = local_paths.len() - retention.session_event_local_archive_file_limit;
    for path in local_paths.into_iter().take(move_count) {
        let target = cold_dir.join(
            path.file_name()
                .and_then(|value| value.to_str())
                .unwrap_or("segment-unknown.jsonl"),
        );
        fs::rename(&path, &target).map_err(|source| RuntimeError::Io {
            path: format!("{} -> {}", path.display(), target.display()),
            source,
        })?;
    }
    Ok(())
}
