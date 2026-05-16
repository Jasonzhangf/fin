use crate::CliError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Command {
    Start {
        path: String,
    },
    Stop {
        path: String,
    },
    DaemonRun {
        path: String,
    },
    ConfigCheck {
        path: String,
    },
    HomeInit {
        path: String,
    },
    ControlBoundaryScenario {
        path: String,
    },
    MainlineScenario {
        path: String,
    },
    RuntimeSession {
        path: String,
        input: String,
    },
    DebugProjection {
        path: String,
        input: String,
    },
    TranscriptSession {
        path: String,
        transcript_path: String,
    },
    WebDebug {
        path: String,
        host: String,
        port: u16,
    },
    ProviderLiveSmoke {
        path: String,
        transcript_path: Option<String>,
    },
    QqbotLiveReceipt {
        path: String,
        target: String,
        run_id: Option<String>,
    },
    BuildDev {
        path: String,
        build_version: Option<String>,
    },
    InstallDev {
        path: String,
        build_version: Option<String>,
    },
    Promote {
        path: String,
        build_version: String,
    },
    Rollback {
        path: String,
    },
}

pub(crate) fn parse_command(args: &[String]) -> Result<Command, CliError> {
    match args {
        [cmd, path] if cmd == "start" => Ok(Command::Start { path: path.clone() }),
        [cmd, path] if cmd == "stop" => Ok(Command::Stop { path: path.clone() }),
        [cmd, path] if cmd == "daemon-run" => Ok(Command::DaemonRun { path: path.clone() }),
        [cmd, path] if cmd == "config-check" => Ok(Command::ConfigCheck { path: path.clone() }),
        [cmd, path] if cmd == "home-init" => Ok(Command::HomeInit { path: path.clone() }),
        [cmd, path] if cmd == "control-boundary-scenario" => {
            Ok(Command::ControlBoundaryScenario { path: path.clone() })
        }
        [cmd, path] if cmd == "mainline-scenario" => Ok(Command::MainlineScenario { path: path.clone() }),
        [cmd, path, input] if cmd == "runtime-session" => Ok(Command::RuntimeSession {
            path: path.clone(),
            input: input.clone(),
        }),
        [cmd, path, input] if cmd == "debug-projection" => Ok(Command::DebugProjection {
            path: path.clone(),
            input: input.clone(),
        }),
        [cmd, path, transcript_path] if cmd == "transcript-session" => Ok(Command::TranscriptSession {
            path: path.clone(),
            transcript_path: transcript_path.clone(),
        }),
        [cmd, path] if cmd == "provider-live-smoke" => Ok(Command::ProviderLiveSmoke {
            path: path.clone(),
            transcript_path: None,
        }),
        [cmd, path, transcript_path] if cmd == "provider-live-smoke" => {
            Ok(Command::ProviderLiveSmoke {
                path: path.clone(),
                transcript_path: Some(transcript_path.clone()),
            })
        }
        [cmd, path, target] if cmd == "qqbot-live-receipt" => Ok(Command::QqbotLiveReceipt {
            path: path.clone(),
            target: target.clone(),
            run_id: None,
        }),
        [cmd, path, target, run_id] if cmd == "qqbot-live-receipt" => {
            Ok(Command::QqbotLiveReceipt {
                path: path.clone(),
                target: target.clone(),
                run_id: Some(run_id.clone()),
            })
        }
        [cmd, path] if cmd == "web-debug" => Ok(Command::WebDebug {
            path: path.clone(),
            host: "127.0.0.1".into(),
            port: 4040,
        }),
        [cmd, path, host_or_port] if cmd == "web-debug" => {
            if let Ok(port) = host_or_port.parse::<u16>() {
                Ok(Command::WebDebug {
                    path: path.clone(),
                    host: "127.0.0.1".into(),
                    port,
                })
            } else {
                Ok(Command::WebDebug {
                    path: path.clone(),
                    host: host_or_port.clone(),
                    port: 4040,
                })
            }
        }
        [cmd, path, host, port] if cmd == "web-debug" => Ok(Command::WebDebug {
            path: path.clone(),
            host: host.clone(),
            port: port
                .parse::<u16>()
                .map_err(|_| CliError::InvalidPort(port.clone()))?,
        }),
        [cmd, path] if cmd == "build-dev" => Ok(Command::BuildDev {
            path: path.clone(),
            build_version: None,
        }),
        [cmd, path, build_version] if cmd == "build-dev" => Ok(Command::BuildDev {
            path: path.clone(),
            build_version: Some(build_version.clone()),
        }),
        [cmd, path] if cmd == "install-dev" => Ok(Command::InstallDev {
            path: path.clone(),
            build_version: None,
        }),
        [cmd, path, build_version] if cmd == "install-dev" => Ok(Command::InstallDev {
            path: path.clone(),
            build_version: Some(build_version.clone()),
        }),
        [cmd, path, build_version] if cmd == "promote" => Ok(Command::Promote {
            path: path.clone(),
            build_version: build_version.clone(),
        }),
        [cmd, path] if cmd == "rollback" => Ok(Command::Rollback { path: path.clone() }),
        _ => Err(CliError::Usage),
    }
}
