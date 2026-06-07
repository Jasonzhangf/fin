use crate::{
    config::{load_system_config, map_system_config},
    demo::{DemoRequest, demo_identity, run_demo, run_demo_request, sanitize_id_fragment},
    runtime_home::{
        SessionMessageRecord, ensure_runtime_home_layout, persist_runtime_demo, read_last_run_value,
    },
    transcript::{TranscriptScenario, TranscriptTurn, run_transcript_demo},
    versioning::resolve_build_version,
};
use fin_config::SystemConfig;
use fin_contracts::{ContextSnapshotRecord, ControlFeedback, DigestRecord};
#[cfg(test)]
use fin_provider::StructuredStaticProviderClient;
use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

static TEST_UNIQUIFIER: AtomicU64 = AtomicU64::new(0);

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

fn sample_system_config() -> SystemConfig {
    map_system_config(&sample_user_toml()).expect("system config should map")
}

fn write_temp_user_config() -> String {
    let seq = TEST_UNIQUIFIER.fetch_add(1, Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!(
        "fin-cli-test-{}-{seq}.toml",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time should work")
            .as_nanos()
    ));
    fs::write(&path, sample_user_toml()).expect("temp config should write");
    path.display().to_string()
}

fn temp_runtime_home() -> PathBuf {
    let seq = TEST_UNIQUIFIER.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!(
        "fin-runtime-home-{}-{seq}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time should work")
            .as_nanos()
    ))
}

fn static_provider(system: &SystemConfig) -> StructuredStaticProviderClient {
    StructuredStaticProviderClient::new(fin_provider::ProviderDescriptor::from_resolved(
        system.default_provider_config().expect("default provider"),
    ))
}

fn read_json_value(path: &Path) -> serde_json::Value {
    serde_json::from_slice(&fs::read(path).expect("json file should exist")).expect("valid json")
}

fn read_json_messages(path: &Path) -> Vec<SessionMessageRecord> {
    serde_json::from_slice(&fs::read(path).expect("messages should exist")).expect("messages json")
}

#[path = "tests_command_parse.rs"]
mod tests_command_parse;

#[test]
fn load_system_config_maps_user_config() {
    let path = write_temp_user_config();
    let system = load_system_config(Path::new(&path)).expect("system config should load");
    assert_eq!(system.default_provider, "openai");
}

#[test]
fn runtime_demo_persists_home_artifacts() {
    let user_toml = sample_user_toml();
    let system = sample_system_config();
    let home = temp_runtime_home();
    let run =
        run_demo(&system, &static_provider(&system), "hello").expect("runtime demo should run");
    let artifacts = persist_runtime_demo(&user_toml, &system, &run, Some(home.as_path()))
        .expect("artifacts should persist");
    let session_prefix = artifacts
        .session_dir
        .strip_prefix(&home)
        .expect("session dir should live under runtime home")
        .to_string_lossy()
        .to_string();

    assert!(home.join("config/user.toml").exists());
    let system_template =
        fs::read_to_string(home.join("config/system.template.toml")).expect("system template");
    assert!(system_template.contains("policy.entry_role"));
    assert!(system_template.contains("entry_role = \"system\""));
    assert!(system_template.contains("default_role = \"project\""));
    for relative in [
        "runtime/projections/current_projection.json",
        "events/stream.jsonl",
        "conversation/messages.json",
        "digests/recent_digests.json",
        "control/latest.json",
        "reasoning/latest.json",
        "tools/latest.json",
        "provider/latest_requests.json",
        "provider/latest_responses.json",
        "rounds/latest.json",
        "steps/latest.json",
        "turns/latest.json",
        "tasks/routing/latest.json",
        "closures/latest.json",
        "runtime/current/current_control_feedback.json",
        "runtime/current/current_reasoning_view.json",
        "runtime/current/current_provider_requests.json",
        "runtime/current/current_provider_responses.json",
        "runtime/current/current_rounds.json",
        "runtime/current/current_step_records.json",
        "runtime/current/current_turn.json",
        "runtime/current/current_routing_decision.json",
        "runtime/current/current_closure_trace.json",
    ] {
        let path = if relative.starts_with("runtime/") {
            home.join(relative)
        } else {
            home.join(&session_prefix).join(relative)
        };
        assert!(path.exists(), "missing {}", path.display());
    }

    let control_feedback: ControlFeedback = serde_json::from_str(
        &fs::read_to_string(home.join("runtime/current/current_control_feedback.json"))
            .expect("current control feedback should exist"),
    )
    .expect("control feedback should decode");
    assert_eq!(control_feedback.origin, "model_output_contract_v1");
    let last_run = read_last_run_value(&home).expect("last_run should exist");
    assert_eq!(
        last_run
            .get("current_control_feedback_path")
            .and_then(serde_json::Value::as_str),
        Some("runtime/current/current_control_feedback.json")
    );
    assert_eq!(
        last_run
            .get("turn_index")
            .and_then(serde_json::Value::as_u64),
        None
    );
    assert_eq!(
        last_run
            .get("current_turn_record_path")
            .and_then(serde_json::Value::as_str),
        Some("runtime/current/current_turn.json")
    );
}

#[test]
fn hidden_framework_resume_turn_does_not_pollute_normal_session_history() {
    let user_toml = sample_user_toml();
    let system = sample_system_config();
    let home = temp_runtime_home();
    let provider = static_provider(&system);

    let visible_run = run_demo_request(
        &system,
        &provider,
        DemoRequest {
            operation_id: "op-visible-0001".into(),
            trace_id: "trace-visible-0001".into(),
            session_id: "session-hidden-guard".into(),
            task_id: Some("task-hidden-guard".into()),
            topic_thread_id: None,
            agent_name: Some("system".into()),
            role_id: Some("system".into()),
            input: "visible user turn".into(),
            source: "cli.user".into(),
            recent_messages: Vec::new(),
            recent_digests: Vec::new(),
            recent_reasoning_views: Vec::new(),
            recent_tool_records: Vec::new(),
            project_label: Some("fin".into()),
            runtime_home: Some(home.display().to_string()),
            cwd: None,
            selected_paths: Vec::new(),
            attachment_summaries: Vec::new(),
            submitted_at: "2026-04-24T10:00:00+08:00".into(),
        },
    )
    .expect("visible run");
    persist_runtime_demo(&user_toml, &system, &visible_run, Some(home.as_path()))
        .expect("persist visible run");

    let session_dir = home.join("sessions/2026/04/session-hidden-guard");
    let messages_before = read_json_messages(&session_dir.join("conversation/messages.json"));
    assert_eq!(messages_before.len(), 2);
    assert_eq!(messages_before[0].role, "user");
    assert_eq!(messages_before[1].role, "assistant");
    let digests_before = read_json_value(&session_dir.join("digests/recent_digests.json"));
    let reasoning_before =
        read_json_value(&session_dir.join("reasoning/recent_reasoning_views.json"));
    let tools_before = read_json_value(&session_dir.join("tools/recent_tool_records.json"));
    let turns_before = read_json_value(&session_dir.join("turns/recent_turns.json"));
    let events_before = std::fs::read_to_string(session_dir.join("events/stream.jsonl"))
        .expect("visible stream should exist");
    let session_archive_index_before =
        std::fs::read_to_string(session_dir.join("events/archive_index.json")).ok();
    let current_archive_index_before =
        std::fs::read_to_string(home.join("runtime/current/current_event_archive_index.json")).ok();

    let hidden_run = run_demo_request(
        &system,
        &provider,
        DemoRequest {
            operation_id: "op-hidden-0002".into(),
            trace_id: "trace-hidden-0002".into(),
            session_id: "session-hidden-guard".into(),
            task_id: Some("task-hidden-guard".into()),
            topic_thread_id: None,
            agent_name: Some("system".into()),
            role_id: Some("system".into()),
            input: "Continue the same turn.".into(),
            source: "framework.resume_checkpoint.wait".into(),
            recent_messages: Vec::new(),
            recent_digests: Vec::new(),
            recent_reasoning_views: Vec::new(),
            recent_tool_records: Vec::new(),
            project_label: Some("fin".into()),
            runtime_home: Some(home.display().to_string()),
            cwd: None,
            selected_paths: Vec::new(),
            attachment_summaries: Vec::new(),
            submitted_at: "2026-04-24T10:01:00+08:00".into(),
        },
    )
    .expect("hidden run");
    persist_runtime_demo(&user_toml, &system, &hidden_run, Some(home.as_path()))
        .expect("persist hidden run");

    let messages_after = read_json_messages(&session_dir.join("conversation/messages.json"));
    assert_eq!(messages_after, messages_before);
    assert_eq!(
        read_json_value(&session_dir.join("digests/recent_digests.json")),
        digests_before
    );
    assert_eq!(
        read_json_value(&session_dir.join("reasoning/recent_reasoning_views.json")),
        reasoning_before
    );
    assert_eq!(
        read_json_value(&session_dir.join("tools/recent_tool_records.json")),
        tools_before
    );
    assert_eq!(
        read_json_value(&session_dir.join("turns/recent_turns.json")),
        turns_before
    );
    assert_eq!(
        std::fs::read_to_string(session_dir.join("events/stream.jsonl"))
            .expect("hidden turn should not rewrite event stream"),
        events_before
    );
    assert!(
        std::fs::read_to_string(session_dir.join("events/archive_index.json")).ok()
            == session_archive_index_before,
        "hidden turn should not rewrite session archive index"
    );
    assert!(
        std::fs::read_to_string(home.join("runtime/current/current_event_archive_index.json")).ok()
            == current_archive_index_before,
        "hidden turn should not rewrite current archive index"
    );
    let last_run = read_last_run_value(&home).expect("last run should stay visible");
    assert_eq!(
        last_run
            .get("operation_id")
            .and_then(serde_json::Value::as_str),
        Some("op-visible-0001")
    );
}

#[path = "tests_runtime_artifacts.rs"]
mod tests_runtime_artifacts;

#[test]
fn demo_identity_uses_test_namespace_when_provided() {
    let ids = demo_identity(Some("test-provider-smoke"));
    assert_eq!(ids.session_id, "session-test-provider-smoke");
    assert_eq!(ids.task_id, "task-test-provider-smoke");
    assert_eq!(ids.operation_id, "op-test-provider-smoke");
}

#[test]
fn sanitize_id_fragment_normalizes_non_identifier_chars() {
    assert_eq!(
        sanitize_id_fragment(" test/provider smoke "),
        "test-provider-smoke"
    );
}

#[test]
fn debug_projection_command_runs() {
    let user_toml = sample_user_toml();
    let system = sample_system_config();
    let home = temp_runtime_home();
    let run =
        run_demo(&system, &static_provider(&system), "hello").expect("debug projection should run");
    persist_runtime_demo(&user_toml, &system, &run, Some(home.as_path()))
        .expect("artifacts should persist");
    assert!(
        home.join("runtime/projections/current_snapshot.json")
            .exists()
    );
}

#[test]
fn resolve_build_version_starts_at_expected_series_and_bumps() {
    let home = temp_runtime_home();
    ensure_runtime_home_layout(&home).expect("runtime home should init");

    let first = resolve_build_version(&home, None).expect("first build version");
    let second = resolve_build_version(&home, None).expect("second build version");

    assert_eq!(first, "0.1.0001");
    assert_eq!(second, "0.1.0002");
}

#[test]
fn resolve_build_version_override_updates_sequence_floor() {
    let home = temp_runtime_home();
    ensure_runtime_home_layout(&home).expect("runtime home should init");

    let build_version =
        resolve_build_version(&home, Some("0.1.0042".into())).expect("override should work");
    let next = resolve_build_version(&home, None).expect("next build version");

    assert_eq!(build_version, "0.1.0042");
    assert_eq!(next, "0.1.0043");
}

#[test]
fn ensure_runtime_home_layout_creates_skills_dir() {
    let home = temp_runtime_home();
    ensure_runtime_home_layout(&home).expect("runtime home should init");
    assert!(home.join("skills").is_dir());
}

#[test]
fn ensure_runtime_home_layout_creates_sessions_dir() {
    let home = temp_runtime_home();
    ensure_runtime_home_layout(&home).expect("runtime home should init");
    assert!(home.join("sessions").is_dir());
}
