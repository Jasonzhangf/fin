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
        home.join("sessions/2026/04/session-system-entry/conversation/messages.json")
            .exists()
    );
    let state: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(
            home.join("sessions/2026/04/session-system-entry/control/execution_state.json"),
        )
        .expect("execution state"),
    )
    .expect("state json");
    assert_eq!(state["status"].as_str(), Some("idle"));
    assert_eq!(state["session_id"].as_str(), Some("session-system-entry"));
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
    let paired =
        complete_builtin_qqbot_pairing(&home, "session-test-install-0-1-0001", None).expect("pair");
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

#[test]
fn startup_business_entry_normalizes_stale_idle_execution_state_anchors() {
    let home = temp_runtime_home("normalize-state");
    ensure_runtime_home_layout(&home).expect("runtime home");
    let system = map_system_config(&sample_user_toml()).expect("system");

    ensure_startup_business_entry(&home, &system, "2026-04-26T13:20:00+08:00")
        .expect("initial startup entry");

    let stale_state = serde_json::json!({
        "state_id": "exec-state-op-system-entry-0030",
        "session_id": "session-system-entry",
        "status": "idle",
        "active_turn_id": "turn-op-system-entry-0030",
        "active_step_id": "step-op-system-entry-0030-18-finalize",
        "resume_from_step_id": "step-op-system-entry-0030-18-finalize",
        "resume_checkpoint_ready": false,
        "resume_checkpoint_id": null,
        "pending_input_count": 0,
        "accepts_user_input": true,
        "reason": "closure completed",
        "updated_at": "2026-04-26T13:24:20+08:00"
    });
    let session_state_path =
        home.join("sessions/2026/04/session-system-entry/control/execution_state.json");
    fs::write(
        &session_state_path,
        serde_json::to_vec_pretty(&stale_state).expect("serialize stale state"),
    )
    .expect("write stale state");
    fs::write(
        home.join("runtime/current/current_execution_state.json"),
        serde_json::to_vec_pretty(&stale_state).expect("serialize stale state"),
    )
    .expect("write stale runtime state");
    fs::write(
        home.join("runtime/current/last_run.json"),
        serde_json::to_vec_pretty(&serde_json::json!({
            "session_id": "session-system-entry",
            "operation_id": null
        }))
        .expect("serialize last run"),
    )
    .expect("write last run");

    ensure_startup_business_entry(&home, &system, "2026-04-26T13:30:00+08:00")
        .expect("startup entry should normalize stale idle state");

    let repaired: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(&session_state_path).expect("read repaired execution state"),
    )
    .expect("decode repaired state");
    assert_eq!(
        repaired["state_id"].as_str(),
        Some("exec-state-session-system-entry")
    );
    assert_eq!(repaired["active_turn_id"], serde_json::Value::Null);
    assert_eq!(repaired["active_step_id"], serde_json::Value::Null);
    assert_eq!(repaired["resume_from_step_id"], serde_json::Value::Null);
    assert_eq!(
        repaired["reason"].as_str(),
        Some("startup normalized stale idle execution anchors")
    );

    let current: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(home.join("runtime/current/current_execution_state.json"))
            .expect("read current execution state"),
    )
    .expect("decode current state");
    assert_eq!(current["active_turn_id"], serde_json::Value::Null);
    assert_eq!(current["active_step_id"], serde_json::Value::Null);
    assert_eq!(current["resume_from_step_id"], serde_json::Value::Null);
}
