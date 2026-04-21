use crate::{
    CliError,
    local_command_notice::append_notice_messages,
    routing_prompt_state::{
        load_latest_routing_decision, load_pending_routing_action, resolve_pending_routing_action,
    },
    session_binding::{
        find_session_dir, infer_session_topic_thread_id, rebind_last_run_binding, write_json,
    },
    time::local_timestamp_now,
};
use chrono::Local;
use fin_debug_server::{ChatSendResponse, DebugBinding};
use fin_runtime::{StoredTaskRecord, create_task_record, load_task_record};
use serde_json::json;
use std::path::Path;

pub(crate) fn try_handle_routing_command(
    runtime_home: &Path,
    message: &str,
    binding: &DebugBinding,
) -> Result<Option<ChatSendResponse>, CliError> {
    match message.trim() {
        "/formalize" => handle_formalize(runtime_home, binding).map(Some),
        "/stay" => handle_stay(runtime_home, binding).map(Some),
        _ => Ok(None),
    }
}

fn handle_formalize(
    runtime_home: &Path,
    binding: &DebugBinding,
) -> Result<ChatSendResponse, CliError> {
    let Some(session_id) = binding.session_id.as_deref() else {
        return notice(binding, "formalize skipped: no active session binding");
    };
    let Some(action) = load_pending_routing_action(runtime_home, binding)? else {
        return notice(binding, "formalize skipped: no pending routing choice");
    };
    let decision = load_latest_routing_decision(runtime_home, binding)?;

    match action.action_kind.as_str() {
        "ask_reuse_existing_task" => bind_existing_task(runtime_home, binding, session_id, &action),
        "ask_formalize_task" | "ask_topic_switch" => create_and_bind_formal_task(
            runtime_home,
            binding,
            session_id,
            &action,
            decision.as_ref(),
        ),
        other => notice(
            binding,
            format!("formalize skipped: unsupported routing action {other}").as_str(),
        ),
    }
}

fn handle_stay(runtime_home: &Path, binding: &DebugBinding) -> Result<ChatSendResponse, CliError> {
    let Some(action) = load_pending_routing_action(runtime_home, binding)? else {
        return notice(binding, "stay skipped: no pending routing choice");
    };
    let now = local_timestamp_now();
    let _ = resolve_pending_routing_action(
        runtime_home,
        binding,
        "stay_tentative_session",
        "user chose to stay in current session without formalizing",
        &now,
    )?;
    if let Some(session_id) = binding.session_id.as_deref() {
        if let Some((_, _, session_dir)) = find_session_dir(runtime_home, session_id) {
            let _ = append_notice_messages(
                &session_dir.join("conversation/messages.json"),
                session_id,
                binding.task_id.as_deref(),
                "/stay",
                "kept current session without formalizing a new task",
            );
        }
    }
    Ok(ChatSendResponse {
        binding: binding.clone(),
        answer: action
            .prompt_text
            .map(|text| format!("{text}\n\n已保持当前会话，不进入正式任务。"))
            .unwrap_or_else(|| "已保持当前会话，不进入正式任务。".into()),
        digest_id: format!(
            "digest-routing-stay-{}",
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

fn bind_existing_task(
    runtime_home: &Path,
    binding: &DebugBinding,
    source_session_id: &str,
    action: &fin_contracts::RoutingActionRecord,
) -> Result<ChatSendResponse, CliError> {
    let Some(task_id) = action.suggested_task_id.as_deref() else {
        return notice(binding, "formalize skipped: missing suggested_task_id");
    };
    let Some((summary, _task)) = load_task_record(runtime_home, task_id, None)
        .map_err(|error| CliError::InvalidInstallState(error.to_string()))?
    else {
        return notice(
            binding,
            format!("formalize skipped: suggested task not found {task_id}").as_str(),
        );
    };
    let Some((year, month, session_dir)) = find_session_dir(runtime_home, &summary.session_id)
    else {
        return notice(binding, "formalize skipped: target session missing");
    };
    let topic_thread_id =
        infer_session_topic_thread_id(&session_dir).or(action.suggested_topic_thread_id.clone());
    let rebound = rebind_last_run_binding(
        runtime_home,
        &summary.session_id,
        Some(task_id),
        topic_thread_id.as_deref(),
        year,
        month,
    )?;
    let now = local_timestamp_now();
    let _ = resolve_pending_routing_action(
        runtime_home,
        binding,
        "reused_existing_task",
        format!("user confirmed reuse of existing task {task_id}").as_str(),
        &now,
    )?;
    let _ = append_notice_messages(
        &session_dir.join("conversation/messages.json"),
        &summary.session_id,
        Some(task_id),
        "/formalize",
        format!("reused existing task from routing choice (source_session={source_session_id})")
            .as_str(),
    );
    Ok(ChatSendResponse {
        binding: DebugBinding {
            project_id: binding.project_id.clone(),
            project_label: binding.project_label.clone(),
            runtime_home: binding.runtime_home.clone(),
            session_id: Some(summary.session_id.clone()),
            task_id: Some(task_id.to_string()),
            session_messages_path: rebound.session_messages_path,
            recent_contexts_path: rebound.recent_contexts_path,
            recent_digests_path: rebound.recent_digests_path,
        },
        answer: format!(
            "formalized by reusing existing task: {} / {}",
            summary.session_id, task_id
        ),
        digest_id: format!("digest-routing-formalize-reuse-{task_id}"),
        events_count: 0,
        response_kind: "system_notice".into(),
        freshness: Some("instant".into()),
        control_feedback: None,
        progress: None,
        note: None,
        routing_action: Some(action.clone()),
    })
}

fn create_and_bind_formal_task(
    runtime_home: &Path,
    binding: &DebugBinding,
    session_id: &str,
    action: &fin_contracts::RoutingActionRecord,
    decision: Option<&fin_contracts::RoutingDecisionRecord>,
) -> Result<ChatSendResponse, CliError> {
    let Some((year, month, session_dir)) = find_session_dir(runtime_home, session_id) else {
        return notice(binding, "formalize skipped: session directory missing");
    };
    let stamp = Local::now().format("%Y%m%d%H%M%S").to_string();
    let topic_summary = decision
        .and_then(|value| value.current_topic_summary.as_ref())
        .cloned()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "formalized task".into());
    let task_id = action
        .suggested_task_id
        .clone()
        .unwrap_or_else(|| format!("task-{stamp}"));
    let topic_thread_id = action
        .suggested_topic_thread_id
        .clone()
        .unwrap_or_else(|| format!("topic-{stamp}"));
    let task = StoredTaskRecord {
        task_id: task_id.clone(),
        session_id: session_id.into(),
        title: topic_summary.clone(),
        summary: decision
            .map(|value| value.reason.clone())
            .unwrap_or_else(|| topic_summary.clone()),
        status: "ready".into(),
        created_at: local_timestamp_now(),
        updated_at: local_timestamp_now(),
        ..StoredTaskRecord::default()
    };
    let receipt = create_task_record(runtime_home, session_id, task)
        .map_err(|error| CliError::InvalidInstallState(error.to_string()))?;
    persist_topic_binding(
        &session_dir,
        &topic_thread_id,
        &task_id,
        &topic_summary,
        &receipt.task.updated_at,
    )?;
    let rebound = rebind_last_run_binding(
        runtime_home,
        session_id,
        Some(task_id.as_str()),
        Some(topic_thread_id.as_str()),
        year,
        month,
    )?;
    let now = local_timestamp_now();
    let resolution_kind = if action.action_kind == "ask_topic_switch" {
        "switched_topic_task"
    } else {
        "formalized_new_task"
    };
    let _ = resolve_pending_routing_action(
        runtime_home,
        binding,
        resolution_kind,
        format!("user confirmed formalization into {task_id}/{topic_thread_id}").as_str(),
        &now,
    )?;
    let _ = append_notice_messages(
        &session_dir.join("conversation/messages.json"),
        session_id,
        Some(task_id.as_str()),
        "/formalize",
        format!("formalized current session into task {task_id} with topic {topic_thread_id}")
            .as_str(),
    );
    Ok(ChatSendResponse {
        binding: DebugBinding {
            project_id: binding.project_id.clone(),
            project_label: binding.project_label.clone(),
            runtime_home: binding.runtime_home.clone(),
            session_id: Some(session_id.to_string()),
            task_id: Some(task_id.clone()),
            session_messages_path: rebound.session_messages_path,
            recent_contexts_path: rebound.recent_contexts_path,
            recent_digests_path: rebound.recent_digests_path,
        },
        answer: format!("formalized current session: {session_id} / {task_id} / {topic_thread_id}"),
        digest_id: format!("digest-routing-formalize-{task_id}"),
        events_count: receipt.artifact_refs.len(),
        response_kind: "system_notice".into(),
        freshness: Some("instant".into()),
        control_feedback: None,
        progress: None,
        note: None,
        routing_action: Some(action.clone()),
    })
}

fn persist_topic_binding(
    session_dir: &Path,
    topic_thread_id: &str,
    task_id: &str,
    summary: &str,
    created_at: &str,
) -> Result<(), CliError> {
    let payload = json!({
        "topic_thread_id": topic_thread_id,
        "task_id": task_id,
        "summary": summary,
        "created_at": created_at,
    });
    write_json(&session_dir.join("topics/latest.json"), &payload)?;
    write_json(
        &session_dir.join(format!("topics/registry/{topic_thread_id}.json")),
        &payload,
    )
}

fn notice(binding: &DebugBinding, answer: &str) -> Result<ChatSendResponse, CliError> {
    Ok(ChatSendResponse {
        binding: binding.clone(),
        answer: answer.into(),
        digest_id: "digest-routing-command-notice".into(),
        events_count: 0,
        response_kind: "system_notice".into(),
        freshness: Some("instant".into()),
        control_feedback: None,
        progress: None,
        note: None,
        routing_action: None,
    })
}
