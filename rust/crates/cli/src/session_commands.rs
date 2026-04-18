use crate::{
    CliError,
    channel_peer::ensure_builtin_qqbot_binding,
    channel_peer_commands::try_handle_channel_peer_command,
    local_command_notice::append_notice_messages,
    runtime_home::{
        SessionMessageRecord, read_last_run_value, read_recent_digests,
        read_recent_reasoning_views, read_recent_tool_records, read_session_messages,
    },
    time::local_timestamp_now,
};
use chrono::{Datelike, Local};
use fin_config::SystemConfig;
use fin_contracts::{ContextSnapshotRecord, EntityRefs};
use fin_debug_server::{ChatSendRequest, ChatSendResponse, DebugBinding};
use fin_runtime::{ContextAssemblyInput, ContextViewBuilder, WorkerRuntime};
use serde::Serialize;
use serde_json::{Value, json};
use std::{
    fs,
    path::{Path, PathBuf},
};

const RECENT_CONTEXT_LIMIT: usize = 8;

pub(crate) fn try_handle_local_command(
    runtime_home: &Path,
    system: &SystemConfig,
    request: &ChatSendRequest,
    binding: &DebugBinding,
) -> Result<Option<ChatSendResponse>, CliError> {
    let message = request.message.trim();
    if !message.starts_with('/') {
        return Ok(None);
    }
    if let Some(response) = try_handle_channel_peer_command(runtime_home, request, binding)? {
        return Ok(Some(response));
    }
    let mut parts = message.split_whitespace();
    let command = parts.next().unwrap_or_default();
    let args = parts.collect::<Vec<_>>();
    let response = match command {
        "/new" => handle_new(runtime_home, binding)?,
        "/resume" => handle_resume(runtime_home, binding, &args)?,
        "/compact" => handle_compact(runtime_home, system, binding)?,
        _ => return Ok(None),
    };
    Ok(Some(response))
}

fn handle_new(runtime_home: &Path, binding: &DebugBinding) -> Result<ChatSendResponse, CliError> {
    let now = Local::now();
    let stamp = now.format("%Y%m%d%H%M%S").to_string();
    let session_id = format!("session-{stamp}");
    let task_id = format!("task-{stamp}");
    let session_dir = ensure_session_layout(runtime_home, now.year(), now.month(), &session_id)?;
    let message_path = session_dir.join("conversation/messages.json");
    let relative = relative_to_runtime(runtime_home, &message_path).unwrap_or_else(|| {
        format!(
            "sessions/{:04}/{:02}/{}/conversation/messages.json",
            now.year(),
            now.month(),
            session_id
        )
    });
    append_notice_messages(
        &message_path,
        &session_id,
        Some(task_id.as_str()),
        "/new",
        "created new session and switched active binding",
    )?;
    let rebound = rebind_last_run(runtime_home, &session_id, &task_id, now.year(), now.month())?;
    let _ = ensure_builtin_qqbot_binding(runtime_home, Some(session_id.as_str()))?;
    Ok(ChatSendResponse {
        binding: DebugBinding {
            project_id: binding.project_id.clone(),
            project_label: binding.project_label.clone(),
            runtime_home: binding.runtime_home.clone(),
            session_id: Some(session_id.clone()),
            task_id: Some(task_id.clone()),
            session_messages_path: Some(relative),
            recent_contexts_path: rebound.recent_contexts_path.clone(),
            recent_digests_path: rebound.recent_digests_path.clone(),
        },
        answer: format!("new session ready: {session_id} / {task_id}"),
        digest_id: format!("digest-local-command-new-{stamp}"),
        events_count: 0,
        response_kind: "system_notice".into(),
        freshness: Some("instant".into()),
        control_feedback: None,
        progress: None,
        note: None,
    })
}

fn handle_resume(
    runtime_home: &Path,
    binding: &DebugBinding,
    args: &[&str],
) -> Result<ChatSendResponse, CliError> {
    let Some(session_id) = args.first().copied() else {
        return Ok(ChatSendResponse {
            binding: binding.clone(),
            answer: "usage: /resume <session_id>".into(),
            digest_id: "digest-local-command-resume-usage".into(),
            events_count: 0,
            response_kind: "system_notice".into(),
            freshness: Some("instant".into()),
            control_feedback: None,
            progress: None,
            note: None,
        });
    };
    let Some((year, month, session_dir)) = find_session_dir(runtime_home, session_id) else {
        return Ok(ChatSendResponse {
            binding: binding.clone(),
            answer: format!("session not found: {session_id}"),
            digest_id: "digest-local-command-resume-miss".into(),
            events_count: 0,
            response_kind: "system_notice".into(),
            freshness: Some("instant".into()),
            control_feedback: None,
            progress: None,
            note: None,
        });
    };
    let task_id =
        infer_session_task_id(&session_dir).unwrap_or_else(|| format!("task-{session_id}"));
    append_notice_messages(
        &session_dir.join("conversation/messages.json"),
        session_id,
        Some(task_id.as_str()),
        &format!("/resume {session_id}"),
        "resumed existing session binding",
    )?;
    let rebound = rebind_last_run(runtime_home, session_id, &task_id, year, month)?;
    let _ = ensure_builtin_qqbot_binding(runtime_home, Some(session_id))?;
    Ok(ChatSendResponse {
        binding: DebugBinding {
            project_id: binding.project_id.clone(),
            project_label: binding.project_label.clone(),
            runtime_home: binding.runtime_home.clone(),
            session_id: Some(session_id.to_string()),
            task_id: Some(task_id.clone()),
            session_messages_path: rebound.session_messages_path.clone(),
            recent_contexts_path: rebound.recent_contexts_path.clone(),
            recent_digests_path: rebound.recent_digests_path.clone(),
        },
        answer: format!("resumed session: {session_id} / {task_id}"),
        digest_id: format!("digest-local-command-resume-{session_id}"),
        events_count: 0,
        response_kind: "system_notice".into(),
        freshness: Some("instant".into()),
        control_feedback: None,
        progress: None,
        note: None,
    })
}

fn handle_compact(
    runtime_home: &Path,
    system: &SystemConfig,
    binding: &DebugBinding,
) -> Result<ChatSendResponse, CliError> {
    let Some(session_id) = binding.session_id.as_deref() else {
        return Ok(ChatSendResponse {
            binding: binding.clone(),
            answer: "compact skipped: no active session binding".into(),
            digest_id: "digest-local-command-compact-no-session".into(),
            events_count: 0,
            response_kind: "system_notice".into(),
            freshness: Some("instant".into()),
            control_feedback: None,
            progress: None,
            note: None,
        });
    };
    let Some((year, month, session_dir)) = find_session_dir(runtime_home, session_id) else {
        return Ok(ChatSendResponse {
            binding: binding.clone(),
            answer: format!("compact skipped: session not found {session_id}"),
            digest_id: "digest-local-command-compact-session-miss".into(),
            events_count: 0,
            response_kind: "system_notice".into(),
            freshness: Some("instant".into()),
            control_feedback: None,
            progress: None,
            note: None,
        });
    };
    let task_id = binding
        .task_id
        .clone()
        .or_else(|| infer_session_task_id(&session_dir))
        .unwrap_or_else(|| format!("task-{session_id}"));
    let messages = read_session_messages(&session_dir.join("conversation/messages.json"))?;
    let recent_messages = messages
        .iter()
        .map(|item| format!("{}: {}", item.role, item.content))
        .collect::<Vec<_>>();
    let recent_digests = read_recent_digests(&session_dir.join("digests/recent_digests.json"))?;
    let recent_reasoning_views =
        read_recent_reasoning_views(&session_dir.join("reasoning/recent_reasoning_views.json"))?;
    let recent_tool_records =
        read_recent_tool_records(&session_dir.join("tools/recent_tool_records.json"))?;

    let worker = WorkerRuntime::from_system(
        system,
        "agent-system",
        "worker-system-compact",
        "cli.local_command",
        None,
    )?;
    let now = local_timestamp_now();
    let op_suffix = Local::now().format("%Y%m%d%H%M%S").to_string();
    let operation_id = format!("op-compact-{op_suffix}");
    let trace_id = format!("trace-compact-{op_suffix}");
    let refs = EntityRefs {
        session_id: Some(session_id.to_string()),
        task_id: Some(task_id.clone()),
        worker_id: Some(worker.worker_id.clone()),
        ..EntityRefs::default()
    };
    let context = ContextViewBuilder.build(
        &worker,
        ContextAssemblyInput {
            operation_id: operation_id.clone(),
            trace_id: trace_id.clone(),
            refs: refs.clone(),
            input: "compact rebuild".into(),
            source: "local_command".into(),
            recent_messages,
            recent_digests,
            recent_reasoning_views,
            recent_tool_records,
            project_label: Some(binding.project_label.clone()),
            runtime_home: Some(runtime_home.display().to_string()),
            cwd: std::env::current_dir()
                .ok()
                .map(|path| path.display().to_string()),
            selected_paths: Vec::new(),
        },
    );
    let snapshot = ContextSnapshotRecord {
        operation_id: operation_id.clone(),
        trace_id: trace_id.clone(),
        refs: refs.clone(),
        input: "compact rebuild".into(),
        context: context.clone(),
        role: worker.policy.role.clone(),
        provider_path: worker.policy.provider_path.clone(),
        provider_strategy: worker.policy.provider_strategy,
        protocol_version: worker.policy.protocol_version.clone(),
        stream: worker.policy.stream,
        captured_at: now.clone(),
    };
    write_json(
        &runtime_home.join("runtime/current/current_context.json"),
        &snapshot,
    )?;
    let recent_context_path = session_dir.join("context/recent_contexts.json");
    let mut recent_contexts = read_json_or_empty::<ContextSnapshotRecord>(&recent_context_path)?;
    recent_contexts.push(snapshot.clone());
    trim_head(&mut recent_contexts, RECENT_CONTEXT_LIMIT);
    write_json(&recent_context_path, &recent_contexts)?;

    let rebuild_index = json!({
        "rebuilt_at": now,
        "reason": "slash_compact",
        "session_id": session_id,
        "task_id": task_id,
        "operation_id": operation_id,
        "trace_id": trace_id,
        "recent_context_count": recent_contexts.len(),
    });
    write_json(
        &runtime_home.join("runtime/current/current_rebuild_index.json"),
        &rebuild_index,
    )?;
    write_json(
        &session_dir.join("context/rebuild-index.json"),
        &rebuild_index,
    )?;
    append_notice_messages(
        &session_dir.join("conversation/messages.json"),
        session_id,
        Some(task_id.as_str()),
        "/compact",
        "context rebuilt from recent session artifacts",
    )?;
    let rebound = rebind_last_run(runtime_home, session_id, &task_id, year, month)?;

    Ok(ChatSendResponse {
        binding: DebugBinding {
            project_id: binding.project_id.clone(),
            project_label: binding.project_label.clone(),
            runtime_home: binding.runtime_home.clone(),
            session_id: Some(session_id.to_string()),
            task_id: Some(task_id.clone()),
            session_messages_path: rebound.session_messages_path.clone(),
            recent_contexts_path: rebound.recent_contexts_path.clone(),
            recent_digests_path: rebound.recent_digests_path.clone(),
        },
        answer: format!(
            "compact done: rebuilt context for {session_id}/{task_id}, recent_contexts={}",
            recent_contexts.len()
        ),
        digest_id: format!("digest-local-command-compact-{op_suffix}"),
        events_count: 0,
        response_kind: "system_notice".into(),
        freshness: Some("instant".into()),
        control_feedback: None,
        progress: None,
        note: None,
    })
}

#[derive(Debug, Clone)]
struct RebindResult {
    session_messages_path: Option<String>,
    recent_contexts_path: Option<String>,
    recent_digests_path: Option<String>,
}

fn rebind_last_run(
    runtime_home: &Path,
    session_id: &str,
    task_id: &str,
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
    object.insert("task_id".into(), Value::String(task_id.to_string()));
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

fn infer_session_task_id(session_dir: &Path) -> Option<String> {
    read_json_or_empty::<SessionMessageRecord>(&session_dir.join("conversation/messages.json"))
        .ok()?
        .into_iter()
        .rev()
        .find_map(|item| item.task_id)
}

fn find_session_dir(runtime_home: &Path, session_id: &str) -> Option<(i32, u32, PathBuf)> {
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

fn ensure_session_layout(
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
        "notes",
        "digests",
        "context",
        "conversation",
        "reasoning",
        "tools",
        "closures",
    ] {
        fs::create_dir_all(session_dir.join(rel)).map_err(|source| CliError::WriteFile {
            path: session_dir.join(rel).display().to_string(),
            source,
        })?;
    }
    ensure_json_file(&session_dir.join("conversation/messages.json"), b"[]")?;
    ensure_json_file(&session_dir.join("digests/recent_digests.json"), b"[]")?;
    ensure_json_file(&session_dir.join("context/recent_contexts.json"), b"[]")?;
    ensure_json_file(
        &session_dir.join("reasoning/recent_reasoning_views.json"),
        b"[]",
    )?;
    ensure_json_file(&session_dir.join("tools/recent_tool_records.json"), b"[]")?;
    ensure_json_file(&session_dir.join("closures/recent_closures.json"), b"[]")?;
    ensure_json_file(&session_dir.join("events/stream.jsonl"), b"")?;
    Ok(session_dir)
}

fn ensure_json_file(path: &Path, default: &[u8]) -> Result<(), CliError> {
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

fn relative_to_runtime(runtime_home: &Path, path: &Path) -> Option<String> {
    path.strip_prefix(runtime_home)
        .ok()
        .map(|value| value.to_string_lossy().to_string())
}

fn trim_head<T>(items: &mut Vec<T>, limit: usize) {
    if items.len() > limit {
        let drain_count = items.len() - limit;
        items.drain(0..drain_count);
    }
}

fn read_json_or_empty<T: for<'de> serde::Deserialize<'de>>(
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

fn write_json<T: Serialize>(path: &Path, value: &T) -> Result<(), CliError> {
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
