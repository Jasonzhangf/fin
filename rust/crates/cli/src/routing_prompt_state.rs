use crate::{CliError, session_binding::find_session_dir};
use fin_contracts::{RoutingActionRecord, RoutingDecisionRecord};
use fin_debug_server::{ChatSendResponse, DebugBinding};
use serde::Serialize;
use std::{
    fs,
    path::{Path, PathBuf},
};

pub(crate) fn load_pending_routing_action(
    runtime_home: &Path,
    binding: &DebugBinding,
) -> Result<Option<RoutingActionRecord>, CliError> {
    let Some(session_dir) = resolve_session_dir(runtime_home, binding)? else {
        return Ok(None);
    };
    let Some(action) = read_json_if_exists::<RoutingActionRecord>(
        &session_dir.join("tasks/routing/latest_action.json"),
    )?
    else {
        return Ok(None);
    };
    Ok(action.prompt_user.then_some(action))
}

pub(crate) fn load_latest_routing_decision(
    runtime_home: &Path,
    binding: &DebugBinding,
) -> Result<Option<RoutingDecisionRecord>, CliError> {
    let Some(session_dir) = resolve_session_dir(runtime_home, binding)? else {
        return Ok(None);
    };
    read_json_if_exists::<RoutingDecisionRecord>(&session_dir.join("tasks/routing/latest.json"))
}

pub(crate) fn resolve_pending_routing_action(
    runtime_home: &Path,
    binding: &DebugBinding,
    resolution_kind: &str,
    reason: &str,
    resolved_at: &str,
) -> Result<Option<RoutingActionRecord>, CliError> {
    let Some(session_dir) = resolve_session_dir(runtime_home, binding)? else {
        return Ok(None);
    };
    let Some(mut action) = read_json_if_exists::<RoutingActionRecord>(
        &session_dir.join("tasks/routing/latest_action.json"),
    )?
    else {
        return Ok(None);
    };
    action.prompt_user = false;
    action.apply_immediately = true;
    action.action_kind = resolution_kind.into();
    action.created_at = resolved_at.into();
    action.reason = reason.into();
    write_json(
        &session_dir.join("tasks/routing/latest_action.json"),
        &action,
    )?;
    write_json(
        &runtime_home.join("runtime/current/current_routing_action.json"),
        &action,
    )?;
    Ok(Some(action))
}

pub(crate) fn prompt_user_choice_response(
    binding: DebugBinding,
    action: &RoutingActionRecord,
) -> ChatSendResponse {
    let prompt = action.prompt_text.clone().unwrap_or_else(|| {
        format!(
            "当前有待确认的 routing 动作：{}。输入 /formalize 或 /stay。",
            action.action_kind
        )
    });
    ChatSendResponse {
        binding,
        answer: prompt,
        digest_id: format!("digest-routing-prompt-{}", action.action_id),
        events_count: 0,
        response_kind: "system_notice".into(),
        freshness: Some("instant".into()),
        control_feedback: None,
        progress: None,
        note: None,
        routing_action: Some(action.clone()),
    }
}

fn resolve_session_dir(
    runtime_home: &Path,
    binding: &DebugBinding,
) -> Result<Option<PathBuf>, CliError> {
    let Some(session_id) = binding.session_id.as_deref() else {
        return Ok(None);
    };
    Ok(find_session_dir(runtime_home, session_id).map(|(_, _, dir)| dir))
}

fn read_json_if_exists<T: for<'de> serde::Deserialize<'de>>(
    path: &Path,
) -> Result<Option<T>, CliError> {
    match fs::read_to_string(path) {
        Ok(content) => serde_json::from_str(&content)
            .map(Some)
            .map_err(CliError::Serialize),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(None),
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
