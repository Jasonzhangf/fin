use crate::{
    CliError,
    agent_presence::mark_entry_agent_idle,
    channel_peer::{complete_builtin_qqbot_pairing, ensure_builtin_qqbot_peer},
    demo::sanitize_id_fragment,
    runtime_home::SessionMessageRecord,
    session_binding::{ensure_session_layout, find_session_dir, rebind_last_run_binding},
};
use chrono::{Datelike, Local};
use fin_config::SystemConfig;
use fin_contracts::{EntityRefs, ExecutionStateRecord};
use serde_json::Value;
use std::{fs, path::Path};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct StartupBusinessEntry {
    pub(crate) session_id: String,
    pub(crate) qqbot_bound_session_id: Option<String>,
}

const LEGACY_TEST_SESSION_EXACT: &[&str] = &["session-cli-demo"];
const TEST_SESSION_PREFIXES: &[&str] = &[
    "session-test-",
    "session-mainline-demo",
    "session-transcript-demo",
];

pub(crate) fn ensure_startup_business_entry(
    runtime_home: &Path,
    system: &SystemConfig,
    updated_at: &str,
) -> Result<StartupBusinessEntry, CliError> {
    let now = Local::now();
    let session_id = canonical_entry_session_id(system);
    let session_dir = ensure_session_layout(runtime_home, now.year(), now.month(), &session_id)?;
    write_startup_notice_if_empty(&session_dir, &session_id, updated_at)?;
    ensure_startup_execution_state(runtime_home, &session_dir, &session_id, updated_at)?;
    let _ = rebind_last_run_binding(
        runtime_home,
        &session_id,
        None,
        None,
        now.year(),
        now.month(),
    )?;
    let _ = mark_entry_agent_idle(
        system,
        runtime_home,
        Some(&session_id),
        None,
        None,
        updated_at,
        "system entry ready on canonical startup session",
    )?;
    let qqbot_bound_session_id = ensure_qqbot_default_business_binding(runtime_home, &session_id)?;
    persist_current_startup_entry_session(runtime_home, &session_id, updated_at)?;
    Ok(StartupBusinessEntry {
        session_id,
        qqbot_bound_session_id,
    })
}

pub(crate) fn canonical_entry_session_id(system: &SystemConfig) -> String {
    let role = sanitize_id_fragment(system.policy.entry_role.as_str());
    let scope = if role.is_empty() {
        "system-entry".to_string()
    } else {
        format!("{role}-entry")
    };
    format!("session-{scope}")
}

fn ensure_qqbot_default_business_binding(
    runtime_home: &Path,
    canonical_session_id: &str,
) -> Result<Option<String>, CliError> {
    let state = ensure_builtin_qqbot_peer(runtime_home)?;
    let should_bind = match state.session_id.as_deref() {
        None => true,
        Some(_session_id) if !state.session_valid => true,
        Some(session_id) if !session_exists(runtime_home, session_id) => true,
        Some(session_id) if is_disposable_test_session(session_id) => true,
        Some(session_id) if session_id == canonical_session_id => false,
        Some(_) => false,
    };
    if should_bind {
        let rebound = complete_builtin_qqbot_pairing(runtime_home, canonical_session_id, None)?;
        return Ok(rebound.session_id);
    }
    Ok(state.session_id)
}

fn write_startup_notice_if_empty(
    session_dir: &Path,
    session_id: &str,
    updated_at: &str,
) -> Result<(), CliError> {
    let path = session_dir.join("conversation/messages.json");
    let mut messages: Vec<SessionMessageRecord> = match fs::read_to_string(&path) {
        Ok(content) => serde_json::from_str(&content).map_err(CliError::Serialize)?,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Vec::new(),
        Err(source) => {
            return Err(CliError::ReadFile {
                path: path.display().to_string(),
                source,
            });
        }
    };
    if messages.is_empty() {
        messages.push(SessionMessageRecord {
            message_id: format!(
                "system-startup-{}",
                updated_at
                    .chars()
                    .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '-' })
                    .collect::<String>()
            ),
            role: "system".into(),
            content: "canonical system startup session ready".into(),
            created_at: updated_at.into(),
            session_id: session_id.into(),
            task_id: None,
            operation_id: None,
            trace_id: None,
            closure_id: None,
            sender_kind: Some("system_notice".into()),
            role_id: Some("system".into()),
            agent_name: Some("system".into()),
            display_name: Some("System Agent".into()),
            source_kind: Some("startup_notice".into()),
        });
        fs::write(
            &path,
            serde_json::to_vec_pretty(&messages).map_err(CliError::Serialize)?,
        )
        .map_err(|source| CliError::WriteFile {
            path: path.display().to_string(),
            source,
        })?;
    }
    Ok(())
}

fn ensure_startup_execution_state(
    runtime_home: &Path,
    session_dir: &Path,
    session_id: &str,
    updated_at: &str,
) -> Result<(), CliError> {
    let path = session_dir.join("control/execution_state.json");
    let maybe_existing = match fs::read_to_string(&path) {
        Ok(content) => Some(
            serde_json::from_str::<ExecutionStateRecord>(&content).map_err(CliError::Serialize)?,
        ),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => None,
        Err(source) => {
            return Err(CliError::ReadFile {
                path: path.display().to_string(),
                source,
            });
        }
    };
    if let Some(existing) = maybe_existing {
        let normalized =
            normalize_idle_startup_execution_state(runtime_home, session_id, updated_at, existing)?;
        if let Some(state) = normalized {
            write_json_pretty(&path, &state)?;
            write_json_pretty(
                &runtime_home.join("runtime/current/current_execution_state.json"),
                &state,
            )?;
            update_last_run_paths(
                runtime_home,
                serde_json::json!({
                    "current_execution_state_path": "runtime/current/current_execution_state.json"
                }),
            )?;
        }
        return Ok(());
    }
    let state = idle_startup_execution_state(session_id, updated_at);
    write_json_pretty(&path, &state)?;
    write_json_pretty(
        &runtime_home.join("runtime/current/current_execution_state.json"),
        &state,
    )?;
    update_last_run_paths(
        runtime_home,
        serde_json::json!({
            "current_execution_state_path": "runtime/current/current_execution_state.json"
        }),
    )?;
    Ok(())
}

fn idle_startup_execution_state(session_id: &str, updated_at: &str) -> ExecutionStateRecord {
    ExecutionStateRecord {
        state_id: format!("exec-state-{session_id}"),
        refs: EntityRefs {
            session_id: Some(session_id.into()),
            ..EntityRefs::default()
        },
        status: "idle".into(),
        active_turn_id: None,
        active_step_id: None,
        pending_input_count: 0,
        accepts_user_input: true,
        reason: Some("system entry ready on canonical startup session".into()),
        updated_at: updated_at.into(),
    }
}

fn normalize_idle_startup_execution_state(
    runtime_home: &Path,
    session_id: &str,
    updated_at: &str,
    state: ExecutionStateRecord,
) -> Result<Option<ExecutionStateRecord>, CliError> {
    let has_stale_active_anchor = state.active_turn_id.is_some()
        || state.active_step_id.is_some()
        ;
    let should_normalize = state.status == "idle"
        && state.pending_input_count == 0
        && has_stale_active_anchor
        && state.refs.session_id.as_deref() == Some(session_id)
        && read_last_run_operation_id(runtime_home)?.is_none();
    if !should_normalize {
        return Ok(None);
    }
    let mut normalized = idle_startup_execution_state(session_id, updated_at);
    normalized.reason = Some("startup normalized stale idle execution anchors".into());
    Ok(Some(normalized))
}

fn read_last_run_operation_id(runtime_home: &Path) -> Result<Option<String>, CliError> {
    let path = runtime_home.join("runtime/current/last_run.json");
    let value = match fs::read_to_string(&path) {
        Ok(content) => serde_json::from_str::<Value>(&content).map_err(CliError::Serialize)?,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(source) => {
            return Err(CliError::ReadFile {
                path: path.display().to_string(),
                source,
            });
        }
    };
    Ok(value
        .get("operation_id")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string))
}

fn write_json_pretty<T: serde::Serialize>(path: &Path, value: &T) -> Result<(), CliError> {
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

fn update_last_run_paths(runtime_home: &Path, updates: serde_json::Value) -> Result<(), CliError> {
    let last_run_path = runtime_home.join("runtime/current/last_run.json");
    let mut value = match fs::read_to_string(&last_run_path) {
        Ok(content) => {
            serde_json::from_str::<serde_json::Value>(&content).map_err(CliError::Serialize)?
        }
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => serde_json::json!({}),
        Err(source) => {
            return Err(CliError::ReadFile {
                path: last_run_path.display().to_string(),
                source,
            });
        }
    };
    if !value.is_object() {
        value = serde_json::json!({});
    }
    let object = value.as_object_mut().expect("object");
    for (key, val) in updates.as_object().into_iter().flatten() {
        object.insert(key.clone(), val.clone());
    }
    write_json_pretty(&last_run_path, &value)
}

fn persist_current_startup_entry_session(
    runtime_home: &Path,
    session_id: &str,
    updated_at: &str,
) -> Result<(), CliError> {
    let path = runtime_home.join("runtime/current/current_startup_entry_session.json");
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|source| CliError::WriteFile {
            path: parent.display().to_string(),
            source,
        })?;
    }
    fs::write(
        &path,
        serde_json::to_vec_pretty(&serde_json::json!({
            "session_id": session_id,
            "updated_at": updated_at,
            "entry_kind": "canonical_system_startup"
        }))
        .map_err(CliError::Serialize)?,
    )
    .map_err(|source| CliError::WriteFile {
        path: path.display().to_string(),
        source,
    })
}

fn session_exists(runtime_home: &Path, session_id: &str) -> bool {
    find_session_dir(runtime_home, session_id).is_some()
}

fn is_disposable_test_session(session_id: &str) -> bool {
    LEGACY_TEST_SESSION_EXACT.contains(&session_id)
        || TEST_SESSION_PREFIXES
            .iter()
            .any(|prefix| session_id.starts_with(prefix))
}

#[cfg(test)]
#[path = "startup_business_entry_tests.rs"]
mod tests;
