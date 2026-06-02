use crate::{
    CliError,
    runtime_home::{SessionMessageRecord, read_last_run_value},
};
use chrono::Datelike;
use fin_debug_server::DebugBinding;
use serde::Serialize;
use serde_json::{Value, json};
use std::{
    fs,
    path::{Path, PathBuf},
};

#[derive(Debug, Clone)]
pub(crate) struct RebindResult {
    pub(crate) session_messages_path: Option<String>,
    pub(crate) recent_contexts_path: Option<String>,
    pub(crate) recent_digests_path: Option<String>,
}

pub(crate) fn resolve_binding_for_session(
    runtime_home: &Path,
    base_binding: &DebugBinding,
    session_id: &str,
) -> Result<DebugBinding, CliError> {
    let Some((year, month, _)) = find_session_dir(runtime_home, session_id) else {
        return Err(CliError::InvalidInstallState(format!(
            "session not found for qqbot binding: {session_id}"
        )));
    };
    let binding = build_binding_for_session(runtime_home, base_binding, session_id, None)?;
    let rebound = rebind_last_run_binding(
        runtime_home,
        session_id,
        binding.task_id.as_deref(),
        infer_session_topic_thread_id(
            &runtime_home.join(format!("sessions/{year:04}/{month:02}/{session_id}")),
        )
        .as_deref(),
        year,
        month,
    )?;
    Ok(DebugBinding {
        session_messages_path: rebound.session_messages_path,
        recent_contexts_path: rebound.recent_contexts_path,
        recent_digests_path: rebound.recent_digests_path,
        ..binding
    })
}

pub(crate) fn build_binding_for_session(
    runtime_home: &Path,
    base_binding: &DebugBinding,
    session_id: &str,
    task_id_override: Option<&str>,
) -> Result<DebugBinding, CliError> {
    let Some((year, month, session_dir)) = find_session_dir(runtime_home, session_id) else {
        return Err(CliError::InvalidInstallState(format!(
            "session not found for session binding: {session_id}"
        )));
    };
    let task_id = task_id_override
        .map(str::to_string)
        .or_else(|| infer_session_task_id(&session_dir));
    Ok(DebugBinding {
        project_id: base_binding.project_id.clone(),
        project_label: base_binding.project_label.clone(),
        runtime_home: base_binding.runtime_home.clone(),
        session_id: Some(session_id.to_string()),
        task_id,
        session_messages_path: Some(format!(
            "sessions/{year:04}/{month:02}/{session_id}/conversation/messages.json"
        )),
        recent_contexts_path: Some(format!(
            "sessions/{year:04}/{month:02}/{session_id}/context/recent_contexts.json"
        )),
        recent_digests_path: Some(format!(
            "sessions/{year:04}/{month:02}/{session_id}/digests/recent_digests.json"
        )),
    })
}

pub(crate) fn rebind_last_run(
    runtime_home: &Path,
    session_id: &str,
    task_id: &str,
    year: i32,
    month: u32,
) -> Result<RebindResult, CliError> {
    rebind_last_run_binding(runtime_home, session_id, Some(task_id), None, year, month)
}

pub(crate) fn rebind_last_run_binding(
    runtime_home: &Path,
    session_id: &str,
    task_id: Option<&str>,
    topic_thread_id: Option<&str>,
    year: i32,
    month: u32,
) -> Result<RebindResult, CliError> {
    let mut last_run = read_last_run_value(runtime_home).unwrap_or_else(|_| json!({}));
    if !last_run.is_object() {
        last_run = json!({});
    }
    let message_path =
        format!("sessions/{year:04}/{month:02}/{session_id}/conversation/messages.json");
    let context_path =
        format!("sessions/{year:04}/{month:02}/{session_id}/context/recent_contexts.json");
    let digest_path =
        format!("sessions/{year:04}/{month:02}/{session_id}/digests/recent_digests.json");
    let object = last_run.as_object_mut().expect("object");
    object.insert("session_id".into(), Value::String(session_id.to_string()));
    if let Some(task_id) = task_id {
        object.insert("task_id".into(), Value::String(task_id.to_string()));
    } else {
        object.remove("task_id");
    }
    if let Some(topic_thread_id) = topic_thread_id {
        object.insert(
            "topic_thread_id".into(),
            Value::String(topic_thread_id.to_string()),
        );
    } else {
        object.remove("topic_thread_id");
    }
    object.insert(
        "session_messages_path".into(),
        Value::String(message_path.clone()),
    );
    object.insert(
        "session_recent_contexts_path".into(),
        Value::String(context_path.clone()),
    );
    object.insert(
        "session_recent_digests_path".into(),
        Value::String(digest_path.clone()),
    );
    write_json(
        &runtime_home.join("runtime/current/last_run.json"),
        &last_run,
    )?;
    Ok(RebindResult {
        session_messages_path: Some(message_path),
        recent_contexts_path: Some(context_path),
        recent_digests_path: Some(digest_path),
    })
}

pub(crate) fn ensure_tentative_session_binding(
    runtime_home: &Path,
    base_binding: &DebugBinding,
) -> Result<DebugBinding, CliError> {
    if base_binding.session_id.is_some() {
        return Ok(base_binding.clone());
    }
    let now = chrono::Local::now();
    let stamp = now.format("%Y%m%d%H%M%S").to_string();
    let session_id = format!("session-{stamp}");
    let session_dir = ensure_session_layout(runtime_home, now.year(), now.month(), &session_id)?;
    let rebound = rebind_last_run_binding(
        runtime_home,
        &session_id,
        None,
        None,
        now.year(),
        now.month(),
    )?;
    let relative = relative_to_runtime(
        runtime_home,
        &session_dir.join("conversation/messages.json"),
    )
    .unwrap_or_else(|| {
        format!(
            "sessions/{:04}/{:02}/{}/conversation/messages.json",
            now.year(),
            now.month(),
            session_id
        )
    });
    Ok(DebugBinding {
        project_id: base_binding.project_id.clone(),
        project_label: base_binding.project_label.clone(),
        runtime_home: base_binding.runtime_home.clone(),
        session_id: Some(session_id),
        task_id: None,
        session_messages_path: Some(relative),
        recent_contexts_path: rebound.recent_contexts_path,
        recent_digests_path: rebound.recent_digests_path,
    })
}

pub(crate) fn infer_session_task_id(session_dir: &Path) -> Option<String> {
    read_json_or_empty::<SessionMessageRecord>(&session_dir.join("conversation/messages.json"))
        .ok()?
        .into_iter()
        .rev()
        .find_map(|item| item.task_id)
}

pub(crate) fn infer_session_topic_thread_id(session_dir: &Path) -> Option<String> {
    fs::read_to_string(session_dir.join("topics/latest.json"))
        .ok()
        .and_then(|content| serde_json::from_str::<Value>(&content).ok())
        .and_then(|value| {
            value
                .get("topic_thread_id")
                .and_then(serde_json::Value::as_str)
                .map(str::to_string)
        })
}

pub(crate) fn find_session_dir(
    runtime_home: &Path,
    session_id: &str,
) -> Option<(i32, u32, PathBuf)> {
    let root = runtime_home.join("sessions");
    let years = fs::read_dir(root).ok()?;
    for year in years.flatten() {
        let year_name = year.file_name().to_string_lossy().to_string();
        let Ok(year_num) = year_name.parse::<i32>() else {
            continue;
        };
        let months = fs::read_dir(year.path()).ok()?;
        for month in months.flatten() {
            let month_name = month.file_name().to_string_lossy().to_string();
            let Ok(month_num) = month_name.parse::<u32>() else {
                continue;
            };
            let dir = month.path().join(session_id);
            if dir.exists() {
                return Some((year_num, month_num, dir));
            }
        }
    }
    None
}

pub(crate) fn ensure_session_layout(
    runtime_home: &Path,
    year: i32,
    month: u32,
    session_id: &str,
) -> Result<PathBuf, CliError> {
    let session_dir = runtime_home
        .join("sessions")
        .join(format!("{year:04}"))
        .join(format!("{month:02}"))
        .join(session_id);
    for rel in [
        "events",
        "progress",
        "control",
        "queue",
        "interrupts",
        "notes",
        "digests",
        "context",
        "conversation",
        "reasoning",
        "tools",
        "rounds",
        "closures",
        "topics",
        "topics/registry",
    ] {
        fs::create_dir_all(session_dir.join(rel)).map_err(|source| CliError::WriteFile {
            path: session_dir.join(rel).display().to_string(),
            source,
        })?;
    }
    ensure_json_file(&session_dir.join("conversation/messages.json"), b"[]")?;
    ensure_json_file(&session_dir.join("digests/recent_digests.json"), b"[]")?;
    ensure_json_file(&session_dir.join("context/recent_contexts.json"), b"[]")?;
    ensure_json_file(&session_dir.join("queue/pending_inputs.json"), b"[]")?;
    ensure_json_file(&session_dir.join("interrupts/recent_segments.json"), b"[]")?;
    ensure_json_file(&session_dir.join("interrupts/recent_merges.json"), b"[]")?;
    ensure_json_file(
        &session_dir.join("reasoning/recent_reasoning_views.json"),
        b"[]",
    )?;
    ensure_json_file(&session_dir.join("tools/recent_tool_records.json"), b"[]")?;
    ensure_json_file(&session_dir.join("rounds/recent_rounds.json"), b"[]")?;
    ensure_json_file(&session_dir.join("closures/recent_closures.json"), b"[]")?;
    ensure_json_file(&session_dir.join("events/stream.jsonl"), b"")?;
    Ok(session_dir)
}

pub(crate) fn ensure_json_file(path: &Path, default: &[u8]) -> Result<(), CliError> {
    if path.exists() {
        return Ok(());
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|source| CliError::WriteFile {
            path: parent.display().to_string(),
            source,
        })?;
    }
    fs::write(path, default).map_err(|source| CliError::WriteFile {
        path: path.display().to_string(),
        source,
    })
}

pub(crate) fn relative_to_runtime(runtime_home: &Path, path: &Path) -> Option<String> {
    path.strip_prefix(runtime_home)
        .ok()
        .map(|value| value.to_string_lossy().to_string())
}

pub(crate) fn trim_head<T>(items: &mut Vec<T>, limit: usize) {
    if items.len() > limit {
        let drain_count = items.len() - limit;
        items.drain(0..drain_count);
    }
}

pub(crate) fn read_json_or_empty<T: for<'de> serde::Deserialize<'de>>(
    path: &Path,
) -> Result<Vec<T>, CliError> {
    match fs::read_to_string(path) {
        Ok(content) => serde_json::from_str(&content).map_err(CliError::Serialize),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(source) => Err(CliError::ReadFile {
            path: path.display().to_string(),
            source,
        }),
    }
}

pub(crate) fn write_json<T: Serialize>(path: &Path, value: &T) -> Result<(), CliError> {
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
