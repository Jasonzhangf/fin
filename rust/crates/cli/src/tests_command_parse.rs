use crate::command::{Command, parse_command};

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
