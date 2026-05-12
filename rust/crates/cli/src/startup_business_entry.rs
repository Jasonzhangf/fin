use crate::{
    CliError,
    agent_presence::mark_entry_agent_idle,
    channel_peer::{complete_builtin_qqbot_pairing, ensure_builtin_qqbot_peer},
    session_run::sanitize_id_fragment,
    runtime_home::SessionMessageRecord,
    session_binding::{ensure_session_layout, find_session_dir, rebind_last_run_binding},
};
use chrono::{Datelike, Local};
use fin_config::SystemConfig;
use std::{fs, path::Path};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct StartupBusinessEntry {
    pub(crate) session_id: String,
    pub(crate) qqbot_bound_session_id: Option<String>,
}

const LEGACY_TEST_SESSION_EXACT: &[&str] = &["session-cli-session"];
const TEST_SESSION_PREFIXES: &[&str] = &[
    "session-test-",
    "session-mainline-scenario",
    "session-transcript-session",
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

fn canonical_entry_session_id(system: &SystemConfig) -> String {
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
mod tests {
    use super::*;
    use crate::{
        channel_peer::complete_builtin_qqbot_pairing, config::map_system_config,
        runtime_home::ensure_runtime_home_layout,
    };
    use std::{
        path::PathBuf,
        time::{SystemTime, UNIX_EPOCH},
    };

    fn temp_runtime_home(prefix: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "fin-startup-entry-{prefix}-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("time")
                .as_nanos()
        ))
    }

    fn sample_user_toml() -> String {
        r#"
default_provider = "openai"

[providers.openai]
protocol = "open-ai-compatible"
base_url = "https://api.example.com/v1"
model = "gpt-5"
api_key_env = "OPENAI_API_KEY"
"#
        .into()
    }

    #[test]
    fn startup_business_entry_materializes_canonical_system_session_and_binds_qqbot() {
        let home = temp_runtime_home("canonical");
        ensure_runtime_home_layout(&home).expect("runtime home");
        let system = map_system_config(&sample_user_toml()).expect("system");

        let report = ensure_startup_business_entry(&home, &system, "2026-04-22T20:00:00+08:00")
            .expect("startup entry");

        assert_eq!(report.session_id, "session-system-entry");
        assert_eq!(
            report.qqbot_bound_session_id.as_deref(),
            Some("session-system-entry")
        );
        assert!(
            home.join(format!(
                "sessions/{}/{:02}/session-system-entry/conversation/messages.json",
                chrono::Local::now().year(),
                chrono::Local::now().month()
            ))
                .exists()
        );
        let last_run: serde_json::Value = serde_json::from_str(
            &fs::read_to_string(home.join("runtime/current/last_run.json")).expect("last run"),
        )
        .expect("json");
        assert_eq!(
            last_run["session_id"].as_str(),
            Some("session-system-entry")
        );
        let qqbot_state: serde_json::Value = serde_json::from_str(
            &fs::read_to_string(home.join("runtime/peers/qqbot/state.json")).expect("state"),
        )
        .expect("json");
        assert_eq!(
            qqbot_state["session_id"].as_str(),
            Some("session-system-entry")
        );
        let presence: serde_json::Value = serde_json::from_str(
            &fs::read_to_string(home.join("runtime/agents/state/localhost.system.json"))
                .or_else(|_| {
                    fs::read_to_string(home.join("runtime/current/current_agent_presence.json"))
                })
                .expect("presence"),
        )
        .unwrap_or_default();
        if presence.is_object() {
            assert_eq!(
                presence["current_session_id"].as_str(),
                Some("session-system-entry")
            );
        }
    }

    #[test]
    fn startup_business_entry_repairs_test_bound_qqbot_session_to_canonical_session() {
        let home = temp_runtime_home("repair");
        ensure_runtime_home_layout(&home).expect("runtime home");
        let system = map_system_config(&sample_user_toml()).expect("system");
        let _ = ensure_session_layout(&home, 2026, 4, "session-test-install-0-1-0001")
            .expect("test session");
        let paired = complete_builtin_qqbot_pairing(&home, "session-test-install-0-1-0001", None)
            .expect("pair");
        assert_eq!(
            paired.session_id.as_deref(),
            Some("session-test-install-0-1-0001")
        );

        let report = ensure_startup_business_entry(&home, &system, "2026-04-22T20:10:00+08:00")
            .expect("startup entry");

        assert_eq!(
            report.qqbot_bound_session_id.as_deref(),
            Some("session-system-entry")
        );
    }
}
