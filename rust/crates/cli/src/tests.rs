use crate::{
    command::{Command, parse_command},
    config::{load_system_config, map_system_config},
    demo::{demo_identity, run_demo, sanitize_id_fragment},
    runtime_home::{SessionMessageRecord, ensure_runtime_home_layout, persist_runtime_demo},
    transcript::{TranscriptScenario, TranscriptTurn, run_transcript_demo},
    versioning::resolve_build_version,
};
use fin_config::SystemConfig;
use fin_contracts::{ContextSnapshotRecord, DigestRecord};
#[cfg(test)]
use fin_provider::StaticProviderClient;
use std::{
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

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
    let path = std::env::temp_dir().join(format!(
        "fin-cli-test-{}.toml",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time should work")
            .as_nanos()
    ));
    fs::write(&path, sample_user_toml()).expect("temp config should write");
    path.display().to_string()
}

fn temp_runtime_home() -> PathBuf {
    std::env::temp_dir().join(format!(
        "fin-runtime-home-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time should work")
            .as_nanos()
    ))
}

fn static_provider(system: &SystemConfig) -> StaticProviderClient {
    StaticProviderClient::new(fin_provider::ProviderDescriptor::from_resolved(
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
    assert!(
        home.join("runtime/projections/current_projection.json")
            .exists()
    );
    assert!(
        home.join("sessions/2026/04/session-cli-demo/events/stream.jsonl")
            .exists()
    );
    assert!(
        home.join("sessions/2026/04/session-cli-demo/conversation/messages.json")
            .exists()
    );
    assert!(
        home.join("sessions/2026/04/session-cli-demo/digests/recent_digests.json")
            .exists()
    );
}

#[test]
fn transcript_demo_persists_recent_context_history() {
    let user_toml = sample_user_toml();
    let system = sample_system_config();
    let home = temp_runtime_home();
    let scenario = TranscriptScenario {
        session_id: Some("session-test-transcript".into()),
        task_id: Some("task-test-transcript".into()),
        turns: vec![
            TranscriptTurn {
                input: "first turn".into(),
            },
            TranscriptTurn {
                input: "second turn".into(),
            },
            TranscriptTurn {
                input: "third turn".into(),
            },
        ],
    };

    let transcript = run_transcript_demo(&system, &static_provider(&system), &scenario)
        .expect("transcript demo should run");
    let mut last_session_dir = None;
    for run in &transcript.runs {
        let artifacts = persist_runtime_demo(&user_toml, &system, run, Some(home.as_path()))
            .expect("turn artifacts should persist");
        last_session_dir = Some(artifacts.session_dir);
    }

    let session_dir = last_session_dir.expect("session dir should exist");
    let recent_contexts: Vec<ContextSnapshotRecord> = serde_json::from_str(
        &fs::read_to_string(session_dir.join("context/recent_contexts.json"))
            .expect("recent contexts should exist"),
    )
    .expect("recent contexts should decode");
    assert_eq!(recent_contexts.len(), 3);
    assert!(recent_contexts[0].context.continuity_tail.is_empty());
    assert_eq!(
        recent_contexts[1].context.continuity_tail,
        vec![
            "first turn".to_string(),
            "simulated response for first turn".to_string(),
        ]
    );
    assert!(
        recent_contexts[2]
            .context
            .continuity_tail
            .contains(&"simulated response for second turn".to_string())
    );
    assert!(
        recent_contexts[2]
            .context
            .summary
            .as_deref()
            .unwrap_or_default()
            .contains("second turn")
    );

    let current_context: ContextSnapshotRecord = serde_json::from_str(
        &fs::read_to_string(home.join("runtime/current/current_context.json"))
            .expect("current context should exist"),
    )
    .expect("current context should decode");
    assert_eq!(current_context.input, "third turn");
    assert_eq!(
        current_context.refs.session_id.as_deref(),
        Some("session-test-transcript")
    );
    assert_eq!(
        current_context.refs.task_id.as_deref(),
        Some("task-test-transcript")
    );

    let recent_digests: Vec<DigestRecord> = serde_json::from_str(
        &fs::read_to_string(session_dir.join("digests/recent_digests.json"))
            .expect("recent digests should exist"),
    )
    .expect("recent digests should decode");
    assert_eq!(recent_digests.len(), 3);
    assert!(
        recent_digests[2]
            .summary
            .contains("third turn")
    );

    let messages: Vec<SessionMessageRecord> = serde_json::from_str(
        &fs::read_to_string(session_dir.join("conversation/messages.json"))
            .expect("messages should exist"),
    )
    .expect("messages should decode");
    assert_eq!(messages.len(), 6);
    assert_eq!(messages[0].role, "user");
    assert_eq!(messages[1].role, "assistant");
    assert_eq!(messages[4].content, "third turn");
    assert!(
        messages[5]
            .content
            .contains("simulated response for third turn")
    );
}

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
