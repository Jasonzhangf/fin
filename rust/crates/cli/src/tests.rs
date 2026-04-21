use crate::{
    command::{Command, parse_command},
    config::{load_system_config, map_system_config},
    demo::{demo_identity, run_demo, sanitize_id_fragment},
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

#[test]
fn parse_command_accepts_home_init() {
    let args = vec!["home-init".into(), "/tmp/user.toml".into()];
    assert_eq!(
        parse_command(&args).expect("command should parse"),
        Command::HomeInit {
            path: "/tmp/user.toml".into(),
        }
    );
}

#[test]
fn parse_command_accepts_start() {
    let args = vec!["start".into(), "/tmp/user.toml".into()];
    assert_eq!(
        parse_command(&args).expect("command should parse"),
        Command::Start {
            path: "/tmp/user.toml".into(),
        }
    );
}

#[test]
fn parse_command_accepts_stop() {
    let args = vec!["stop".into(), "/tmp/user.toml".into()];
    assert_eq!(
        parse_command(&args).expect("command should parse"),
        Command::Stop {
            path: "/tmp/user.toml".into(),
        }
    );
}

#[test]
fn parse_command_accepts_daemon_run() {
    let args = vec!["daemon-run".into(), "/tmp/user.toml".into()];
    assert_eq!(
        parse_command(&args).expect("command should parse"),
        Command::DaemonRun {
            path: "/tmp/user.toml".into(),
        }
    );
}

#[test]
fn parse_command_accepts_control_boundary_demo() {
    let args = vec!["control-boundary-demo".into(), "/tmp/user.toml".into()];
    assert_eq!(
        parse_command(&args).expect("command should parse"),
        Command::ControlBoundaryDemo {
            path: "/tmp/user.toml".into(),
        }
    );
}

#[test]
fn parse_command_accepts_mainline_demo() {
    let args = vec!["mainline-demo".into(), "/tmp/user.toml".into()];
    assert_eq!(
        parse_command(&args).expect("command should parse"),
        Command::MainlineDemo {
            path: "/tmp/user.toml".into(),
        }
    );
}

#[test]
fn parse_command_accepts_web_debug_default_port() {
    let args = vec!["web-debug".into(), "/tmp/user.toml".into()];
    assert_eq!(
        parse_command(&args).expect("command should parse"),
        Command::WebDebug {
            path: "/tmp/user.toml".into(),
            port: 4040,
        }
    );
}

#[test]
fn parse_command_accepts_provider_live_smoke() {
    let args = vec!["provider-live-smoke".into(), "/tmp/user.toml".into()];
    assert_eq!(
        parse_command(&args).expect("command should parse"),
        Command::ProviderLiveSmoke {
            path: "/tmp/user.toml".into(),
            transcript_path: None,
        }
    );
}

#[test]
fn parse_command_accepts_provider_live_smoke_with_transcript() {
    let args = vec![
        "provider-live-smoke".into(),
        "/tmp/user.toml".into(),
        "/tmp/provider-live.json".into(),
    ];
    assert_eq!(
        parse_command(&args).expect("command should parse"),
        Command::ProviderLiveSmoke {
            path: "/tmp/user.toml".into(),
            transcript_path: Some("/tmp/provider-live.json".into()),
        }
    );
}

#[test]
fn parse_command_accepts_qqbot_live_receipt() {
    let args = vec![
        "qqbot-live-receipt".into(),
        "/tmp/user.toml".into(),
        "qqbot:c2c:user-1".into(),
    ];
    assert_eq!(
        parse_command(&args).expect("command should parse"),
        Command::QqbotLiveReceipt {
            path: "/tmp/user.toml".into(),
            target: "qqbot:c2c:user-1".into(),
            run_id: None,
        }
    );
}

#[test]
fn parse_command_accepts_qqbot_live_receipt_with_run_id() {
    let args = vec![
        "qqbot-live-receipt".into(),
        "/tmp/user.toml".into(),
        "qqbot:c2c:user-1".into(),
        "qqbot-live-run".into(),
    ];
    assert_eq!(
        parse_command(&args).expect("command should parse"),
        Command::QqbotLiveReceipt {
            path: "/tmp/user.toml".into(),
            target: "qqbot:c2c:user-1".into(),
            run_id: Some("qqbot-live-run".into()),
        }
    );
}

#[test]
fn parse_command_accepts_transcript_demo() {
    let args = vec![
        "transcript-demo".into(),
        "/tmp/user.toml".into(),
        "/tmp/transcript.json".into(),
    ];
    assert_eq!(
        parse_command(&args).expect("command should parse"),
        Command::TranscriptDemo {
            path: "/tmp/user.toml".into(),
            transcript_path: "/tmp/transcript.json".into(),
        }
    );
}

#[test]
fn parse_command_accepts_install_dev_build_version_override() {
    let args = vec![
        "install-dev".into(),
        "/tmp/user.toml".into(),
        "0.1.0007".into(),
    ];
    assert_eq!(
        parse_command(&args).expect("command should parse"),
        Command::InstallDev {
            path: "/tmp/user.toml".into(),
            build_version: Some("0.1.0007".into()),
        }
    );
}

#[test]
fn parse_command_accepts_build_dev() {
    let args = vec![
        "build-dev".into(),
        "/tmp/user.toml".into(),
        "0.1.0008".into(),
    ];
    assert_eq!(
        parse_command(&args).expect("command should parse"),
        Command::BuildDev {
            path: "/tmp/user.toml".into(),
            build_version: Some("0.1.0008".into()),
        }
    );
}

#[test]
fn parse_command_accepts_promote() {
    let args = vec!["promote".into(), "/tmp/user.toml".into(), "0.1.0009".into()];
    assert_eq!(
        parse_command(&args).expect("command should parse"),
        Command::Promote {
            path: "/tmp/user.toml".into(),
            build_version: "0.1.0009".into(),
        }
    );
}

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
    persist_runtime_demo(&user_toml, &system, &run, Some(home.as_path()))
        .expect("artifacts should persist");

    assert!(home.join("config/user.toml").exists());
    let system_template =
        fs::read_to_string(home.join("config/system.template.toml")).expect("system template");
    assert!(system_template.contains("policy.entry_role"));
    assert!(system_template.contains("entry_role = \"system\""));
    assert!(system_template.contains("default_role = \"project\""));
    for relative in [
        "runtime/projections/current_projection.json",
        "sessions/2026/04/session-cli-demo/events/stream.jsonl",
        "sessions/2026/04/session-cli-demo/conversation/messages.json",
        "sessions/2026/04/session-cli-demo/digests/recent_digests.json",
        "sessions/2026/04/session-cli-demo/control/latest.json",
        "sessions/2026/04/session-cli-demo/reasoning/latest.json",
        "sessions/2026/04/session-cli-demo/tools/latest.json",
        "sessions/2026/04/session-cli-demo/provider/latest_requests.json",
        "sessions/2026/04/session-cli-demo/provider/latest_responses.json",
        "sessions/2026/04/session-cli-demo/rounds/latest.json",
        "sessions/2026/04/session-cli-demo/steps/latest.json",
        "sessions/2026/04/session-cli-demo/turns/latest.json",
        "sessions/2026/04/session-cli-demo/tasks/routing/latest.json",
        "sessions/2026/04/session-cli-demo/closures/latest.json",
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
        assert!(home.join(relative).exists(), "missing {relative}");
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
