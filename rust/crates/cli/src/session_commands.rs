use crate::{
    CliError,
    channel_peer::ensure_builtin_qqbot_binding,
    channel_peer_commands::try_handle_channel_peer_command,
    execution_segments::create_interrupted_segment,
    execution_state::{load_pending_inputs, pause_execution, resume_execution},
    local_command_notice::append_notice_messages,
    runtime_home::{
        read_recent_digests, read_recent_reasoning_views, read_recent_tool_records,
        read_session_messages,
    },
    session_binding::{
        ensure_session_layout, find_session_dir, infer_session_task_id,
        infer_session_topic_thread_id, read_json_or_empty, rebind_last_run,
        rebind_last_run_binding, relative_to_runtime, trim_head, write_json,
    },
    session_routing_commands::try_handle_routing_command,
    time::local_timestamp_now,
};
use chrono::{Datelike, Local};
use fin_config::SystemConfig;
use fin_contracts::{ContextSnapshotRecord, EntityRefs};
use fin_debug_server::{ChatSendRequest, ChatSendResponse, DebugBinding};
use fin_runtime::{
    CompactionInput, ContextAssemblyInput, ContextCompactionEngine, ContextViewBuilder,
    create_named_local_worker,
};
use serde_json::json;
use std::{fs, path::Path};

const RECENT_CONTEXT_LIMIT: usize = 8;

pub(crate) fn try_handle_local_command(
    runtime_home: &Path,
    system: &SystemConfig,
    user_toml_path: Option<&Path>,
    request: &ChatSendRequest,
    binding: &DebugBinding,
) -> Result<Option<ChatSendResponse>, CliError> {
    let message = request.message.trim();
    if !message.starts_with('/') {
        return Ok(None);
    }
    if let Some(response) =
        try_handle_channel_peer_command(runtime_home, user_toml_path, request, binding)?
    {
        return Ok(Some(response));
    }
    if let Some(response) = try_handle_routing_command(runtime_home, system, message, binding)? {
        return Ok(Some(response));
    }
    let mut parts = message.split_whitespace();
    let command = parts.next().unwrap_or_default();
    let args = parts.collect::<Vec<_>>();
    let response = match command {
        "/new" => handle_new(runtime_home, binding)?,
        "/clear" => handle_clear(binding)?,
        "/sessions" => handle_sessions(runtime_home, binding)?,
        "/resume" => handle_resume(runtime_home, binding, &args)?,
        "/session" => handle_session_subcommand(runtime_home, binding, &args)?,
        "/pause" => handle_pause(runtime_home, binding, &args)?,
        "/resume-run" => handle_resume_run(runtime_home, binding)?,
        "/tick" => handle_tick(runtime_home, binding)?,
        "/compact" => handle_compact(runtime_home, system, binding)?,
        _ => return Ok(None),
    };
    Ok(Some(response))
}

fn handle_clear(binding: &DebugBinding) -> Result<ChatSendResponse, CliError> {
    Ok(ChatSendResponse {
        binding: binding.clone(),
        answer: "clear done: ui should clear current rendered buffer only; runtime/session truth unchanged".into(),
        digest_id: "digest-local-command-clear".into(),
        events_count: 0,
        response_kind: "system_notice".into(),
        freshness: Some("instant".into()),
        control_feedback: None,
        progress: None,
        note: None,
        routing_action: None,
    })
}

fn handle_sessions(
    runtime_home: &Path,
    binding: &DebugBinding,
) -> Result<ChatSendResponse, CliError> {
    let list = list_sessions(runtime_home)?;
    let preview = list
        .iter()
        .take(20)
        .map(|item| {
            format!(
                "{}{} | {} | {}",
                if item.archived { "[archived] " } else { "" },
                item.session_id,
                item.updated_at,
                item.title.as_deref().unwrap_or("-")
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    let answer = if preview.is_empty() {
        "sessions: none".into()
    } else {
        format!("sessions ({})\n{}", list.len(), preview)
    };
    Ok(ChatSendResponse {
        binding: binding.clone(),
        answer,
        digest_id: "digest-local-command-sessions".into(),
        events_count: 0,
        response_kind: "system_notice".into(),
        freshness: Some("instant".into()),
        control_feedback: None,
        progress: None,
        note: None,
        routing_action: None,
    })
}

fn handle_session_subcommand(
    runtime_home: &Path,
    binding: &DebugBinding,
    args: &[&str],
) -> Result<ChatSendResponse, CliError> {
    let Some(sub) = args.first().copied() else {
        return Ok(system_notice(
            binding,
            "usage: /session <rename|delete|archive> ...",
            "session-usage",
        ));
    };
    match sub {
        "rename" => {
            if args.len() < 3 {
                return Ok(system_notice(
                    binding,
                    "usage: /session rename <session_id> <title>",
                    "session-rename-usage",
                ));
            }
            let session_id = args[1];
            let title = args[2..].join(" ");
            set_session_meta(runtime_home, session_id, Some(title.as_str()), None)?;
            Ok(system_notice(
                binding,
                &format!("session renamed: {session_id}"),
                "session-rename-ok",
            ))
        }
        "archive" => {
            if args.len() < 2 {
                return Ok(system_notice(
                    binding,
                    "usage: /session archive <session_id>",
                    "session-archive-usage",
                ));
            }
            let session_id = args[1];
            set_session_meta(runtime_home, session_id, None, Some(true))?;
            Ok(system_notice(
                binding,
                &format!("session archived: {session_id}"),
                "session-archive-ok",
            ))
        }
        "delete" => {
            if args.len() < 2 {
                return Ok(system_notice(
                    binding,
                    "usage: /session delete <session_id> [--force]",
                    "session-delete-usage",
                ));
            }
            let session_id = args[1];
            let force = args.iter().any(|value| *value == "--force");
            let manual_note = parse_delete_manual_note(args);
            if !force {
                extract_session_knowledge(runtime_home, session_id, manual_note.as_deref())?;
            }
            delete_session(runtime_home, session_id)?;
            let message = if force {
                format!("session deleted by force: {session_id}")
            } else {
                format!("session deleted with knowledge extracted: {session_id}")
            };
            Ok(system_notice(
                binding,
                message.as_str(),
                "session-delete-ok",
            ))
        }
        _ => Ok(system_notice(
            binding,
            "unsupported subcommand",
            "session-unsupported",
        )),
    }
}

fn system_notice(binding: &DebugBinding, answer: &str, suffix: &str) -> ChatSendResponse {
    ChatSendResponse {
        binding: binding.clone(),
        answer: answer.into(),
        digest_id: format!("digest-local-command-{suffix}"),
        events_count: 0,
        response_kind: "system_notice".into(),
        freshness: Some("instant".into()),
        control_feedback: None,
        progress: None,
        note: None,
        routing_action: None,
    }
}

#[derive(Debug, Clone)]
struct SessionListItem {
    session_id: String,
    updated_at: String,
    title: Option<String>,
    archived: bool,
}

fn list_sessions(runtime_home: &Path) -> Result<Vec<SessionListItem>, CliError> {
    let root = runtime_home.join("sessions");
    if !root.exists() {
        return Ok(Vec::new());
    }
    let mut items = Vec::new();
    for year in fs::read_dir(&root).map_err(|source| CliError::ReadFile {
        path: root.display().to_string(),
        source,
    })? {
        let year = match year {
            Ok(value) => value,
            Err(_) => continue,
        };
        for month in match fs::read_dir(year.path()) {
            Ok(value) => value,
            Err(_) => continue,
        } {
            let month = match month {
                Ok(value) => value,
                Err(_) => continue,
            };
            for session in match fs::read_dir(month.path()) {
                Ok(value) => value,
                Err(_) => continue,
            } {
                let session = match session {
                    Ok(value) => value,
                    Err(_) => continue,
                };
                if !session.path().is_dir() {
                    continue;
                }
                let session_id = session.file_name().to_string_lossy().to_string();
                let message_path = session.path().join("conversation/messages.json");
                let updated_at = fs::metadata(&message_path)
                    .ok()
                    .and_then(|m| m.modified().ok())
                    .map(|t| format!("{t:?}"))
                    .unwrap_or_else(|| "unknown".into());
                let meta = read_session_meta(runtime_home, &session_id);
                items.push(SessionListItem {
                    session_id,
                    updated_at,
                    title: meta
                        .as_ref()
                        .and_then(|v| v.get("title").and_then(|x| x.as_str()).map(str::to_string)),
                    archived: meta
                        .as_ref()
                        .and_then(|v| v.get("archived").and_then(|x| x.as_bool()))
                        .unwrap_or(false),
                });
            }
        }
    }
    items.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
    Ok(items)
}

fn session_meta_path(runtime_home: &Path, session_id: &str) -> std::path::PathBuf {
    runtime_home
        .join("sessions")
        .join("meta")
        .join(format!("{session_id}.json"))
}

fn read_session_meta(runtime_home: &Path, session_id: &str) -> Option<serde_json::Value> {
    let path = session_meta_path(runtime_home, session_id);
    fs::read_to_string(path)
        .ok()
        .and_then(|content| serde_json::from_str::<serde_json::Value>(&content).ok())
}

fn set_session_meta(
    runtime_home: &Path,
    session_id: &str,
    title: Option<&str>,
    archived: Option<bool>,
) -> Result<(), CliError> {
    let path = session_meta_path(runtime_home, session_id);
    let mut value = read_session_meta(runtime_home, session_id)
        .unwrap_or_else(|| json!({"session_id":session_id}));
    if let Some(title) = title {
        value["title"] = json!(title);
    }
    if let Some(archived) = archived {
        value["archived"] = json!(archived);
    }
    write_json(&path, &value)
}

fn parse_delete_manual_note(args: &[&str]) -> Option<String> {
    let idx = args.iter().position(|value| *value == "--note")?;
    if idx + 1 >= args.len() {
        return None;
    }
    let note = args[idx + 1..].join(" ").trim().to_string();
    if note.is_empty() { None } else { Some(note) }
}

fn extract_session_knowledge(
    runtime_home: &Path,
    session_id: &str,
    manual_note: Option<&str>,
) -> Result<(), CliError> {
    let Some((_y, _m, session_dir)) = find_session_dir(runtime_home, session_id) else {
        return Ok(());
    };
    let control_path = session_dir.join("control/latest.json");
    let digest_path = session_dir.join("digests/recent_digests.json");
    let control = fs::read_to_string(&control_path)
        .ok()
        .and_then(|content| serde_json::from_str::<serde_json::Value>(&content).ok())
        .unwrap_or_else(|| json!({}));
    let digests = fs::read_to_string(&digest_path)
        .ok()
        .and_then(|content| serde_json::from_str::<serde_json::Value>(&content).ok())
        .unwrap_or_else(|| json!([]));
    let kb_root = runtime_home
        .join("projects")
        .join("fin")
        .join("knowledge-base");
    let entries_dir = kb_root.join("entries");
    let indexes_dir = kb_root.join("indexes");
    fs::create_dir_all(&entries_dir).map_err(|source| CliError::WriteFile {
        path: entries_dir.display().to_string(),
        source,
    })?;
    fs::create_dir_all(&indexes_dir).map_err(|source| CliError::WriteFile {
        path: indexes_dir.display().to_string(),
        source,
    })?;
    let entry_id = format!("kb-{}-{}", session_id, Local::now().format("%Y%m%d%H%M%S"));
    let entry = json!({
        "entry_id": entry_id,
        "session_id": session_id,
        "extracted_at": local_timestamp_now(),
        "manual_note": manual_note.unwrap_or(""),
        "learning_candidates": {
            "note_candidate": control.get("note_candidate"),
            "digest_candidate": control.get("digest_candidate"),
            "reason": control.get("reason"),
        },
        "digests": digests,
    });
    write_json(&entries_dir.join(format!("{entry_id}.json")), &entry)?;

    let by_session_path = indexes_dir.join("by_session.json");
    let mut by_session = fs::read_to_string(&by_session_path)
        .ok()
        .and_then(|content| serde_json::from_str::<serde_json::Value>(&content).ok())
        .unwrap_or_else(|| json!({}));
    let list = by_session
        .get(session_id)
        .and_then(|v| v.as_array().cloned())
        .unwrap_or_default();
    let mut updated = list;
    updated.push(json!(entry_id));
    by_session[session_id] = json!(updated);
    write_json(&by_session_path, &by_session)?;
    Ok(())
}

fn delete_session(runtime_home: &Path, session_id: &str) -> Result<(), CliError> {
    let Some((_year, _month, session_dir)) = find_session_dir(runtime_home, session_id) else {
        return Ok(());
    };
    fs::remove_dir_all(&session_dir).map_err(|source| CliError::WriteFile {
        path: session_dir.display().to_string(),
        source,
    })?;
    let meta = session_meta_path(runtime_home, session_id);
    let _ = fs::remove_file(meta);
    Ok(())
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
        routing_action: None,
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
            routing_action: None,
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
            routing_action: None,
        });
    };
    let task_id = infer_session_task_id(&session_dir);
    append_notice_messages(
        &session_dir.join("conversation/messages.json"),
        session_id,
        task_id.as_deref(),
        &format!("/resume {session_id}"),
        "resumed existing session binding",
    )?;
    let rebound = rebind_last_run_binding(
        runtime_home,
        session_id,
        task_id.as_deref(),
        infer_session_topic_thread_id(&session_dir).as_deref(),
        year,
        month,
    )?;
    let _ = ensure_builtin_qqbot_binding(runtime_home, Some(session_id))?;
    Ok(ChatSendResponse {
        binding: DebugBinding {
            project_id: binding.project_id.clone(),
            project_label: binding.project_label.clone(),
            runtime_home: binding.runtime_home.clone(),
            session_id: Some(session_id.to_string()),
            task_id: task_id.clone(),
            session_messages_path: rebound.session_messages_path.clone(),
            recent_contexts_path: rebound.recent_contexts_path.clone(),
            recent_digests_path: rebound.recent_digests_path.clone(),
        },
        answer: match task_id.as_deref() {
            Some(task_id) => format!("resumed session: {session_id} / {task_id}"),
            None => format!("resumed tentative session: {session_id}"),
        },
        digest_id: format!("digest-local-command-resume-{session_id}"),
        events_count: 0,
        response_kind: "system_notice".into(),
        freshness: Some("instant".into()),
        control_feedback: None,
        progress: None,
        note: None,
        routing_action: None,
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
            routing_action: None,
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
            routing_action: None,
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

    let worker = create_named_local_worker(
        system,
        runtime_home,
        Some("system-compact"),
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
            recent_digests: recent_digests.clone(),
            recent_reasoning_views,
            recent_tool_records: recent_tool_records.clone(),
            project_label: Some(binding.project_label.clone()),
            runtime_home: Some(runtime_home.display().to_string()),
            cwd: std::env::current_dir()
                .ok()
                .map(|path| path.display().to_string()),
            selected_paths: Vec::new(),
            attachment_summaries: Vec::new(),
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

    let compacted = ContextCompactionEngine.compact(CompactionInput {
        session_id: session_id.to_string(),
        task_id: Some(task_id.clone()),
        trigger_reason: "manual_slash_compact".into(),
        recent_messages: messages
            .iter()
            .map(|item| format!("{}: {}", item.role, item.content))
            .collect(),
        digest_records: recent_digests.clone(),
        tool_records: recent_tool_records.clone(),
        retain_recent_count: 8,
        compacted_at: now.clone(),
    });
    let compacted_path = session_dir.join("context/compacted_history.json");
    write_json(&compacted_path, &compacted)?;
    let compact_event_path = session_dir.join("context/compaction-events.jsonl");
    append_jsonl(&compact_event_path, &compacted)?;

    let rebuild_index = json!({
        "rebuilt_at": now,
        "reason": "slash_compact",
        "session_id": session_id,
        "task_id": task_id,
        "operation_id": operation_id,
        "trace_id": trace_id,
        "recent_context_count": recent_contexts.len(),
        "compact_engine": "ContextCompactionEngine",
        "compacted_history_path": relative_to_runtime(runtime_home, &compacted_path),
        "replaced_message_count": compacted.replaced_message_count,
        "retained_artifact_refs": compacted.retained_artifact_refs,
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
        "context compacted via ContextCompactionEngine from recent session artifacts",
    )?;
    let rebound = rebind_last_run_binding(
        runtime_home,
        session_id,
        Some(task_id.as_str()),
        infer_session_topic_thread_id(&session_dir).as_deref(),
        year,
        month,
    )?;

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
            "compact done: ContextCompactionEngine replaced {} messages for {session_id}/{task_id}, recent_contexts={}",
            compacted.replaced_message_count,
            recent_contexts.len()
        ),
        digest_id: format!("digest-local-command-compact-{op_suffix}"),
        events_count: 0,
        response_kind: "system_notice".into(),
        freshness: Some("instant".into()),
        control_feedback: None,
        progress: None,
        note: None,
        routing_action: None,
    })
}

fn handle_pause(
    runtime_home: &Path,
    binding: &DebugBinding,
    args: &[&str],
) -> Result<ChatSendResponse, CliError> {
    let reason = (!args.is_empty()).then(|| args.join(" "));
    let now = local_timestamp_now();
    let paused = pause_execution(runtime_home, binding, &now, reason.clone())?;
    if let Some((_, checkpoint)) = paused.as_ref() {
        let _ = create_interrupted_segment(runtime_home, binding, checkpoint)?;
    }
    let pending_count = load_pending_inputs(runtime_home, binding)?.len();
    Ok(ChatSendResponse {
        binding: binding.clone(),
        answer: if paused.is_some() {
            format!(
                "execution paused{}; pending_inputs={pending_count}",
                reason
                    .as_deref()
                    .map(|value| format!(" ({value})"))
                    .unwrap_or_default()
            )
        } else {
            "pause skipped: no active session binding".into()
        },
        digest_id: format!(
            "digest-local-command-pause-{}",
            binding.session_id.as_deref().unwrap_or("tentative")
        ),
        events_count: 0,
        response_kind: "system_notice".into(),
        freshness: Some("instant".into()),
        control_feedback: None,
        progress: None,
        note: None,
        routing_action: None,
    })
}

fn handle_resume_run(
    runtime_home: &Path,
    binding: &DebugBinding,
) -> Result<ChatSendResponse, CliError> {
    let now = local_timestamp_now();
    let resumed = resume_execution(runtime_home, binding, &now)?;
    let pending_count = load_pending_inputs(runtime_home, binding)?.len();
    Ok(ChatSendResponse {
        binding: binding.clone(),
        answer: if resumed.is_some() {
            format!("execution resumed; pending_inputs={pending_count}")
        } else {
            "resume-run skipped: no active session binding".into()
        },
        digest_id: format!(
            "digest-local-command-resume-run-{}",
            binding.session_id.as_deref().unwrap_or("tentative")
        ),
        events_count: 0,
        response_kind: "system_notice".into(),
        freshness: Some("instant".into()),
        control_feedback: None,
        progress: None,
        note: None,
        routing_action: None,
    })
}

fn handle_tick(runtime_home: &Path, binding: &DebugBinding) -> Result<ChatSendResponse, CliError> {
    let pending_count = load_pending_inputs(runtime_home, binding)?.len();
    Ok(ChatSendResponse {
        binding: binding.clone(),
        answer: format!("scheduler tick requested; pending_inputs={pending_count}"),
        digest_id: format!(
            "digest-local-command-tick-{}",
            binding.session_id.as_deref().unwrap_or("tentative")
        ),
        events_count: 0,
        response_kind: "system_notice".into(),
        freshness: Some("instant".into()),
        control_feedback: None,
        progress: None,
        note: None,
        routing_action: None,
    })
}

fn append_jsonl<T: serde::Serialize>(path: &Path, value: &T) -> Result<(), CliError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|source| CliError::WriteFile {
            path: parent.display().to_string(),
            source,
        })?;
    }
    let mut line = serde_json::to_string(value)?;
    line.push('\n');
    use std::io::Write;
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
        })
}
