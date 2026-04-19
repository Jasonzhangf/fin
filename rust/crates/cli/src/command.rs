use crate::CliError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Command {
    ConfigCheck {
        path: String,
    },
    HomeInit {
        path: String,
    },
    ControlBoundaryDemo {
        path: String,
    },
    MainlineDemo {
        path: String,
    },
    RuntimeDemo {
        path: String,
        input: String,
    },
    DebugProjection {
        path: String,
        input: String,
    },
    TranscriptDemo {
        path: String,
        transcript_path: String,
    },
    WebDebug {
        path: String,
        port: u16,
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
        [cmd, path] if cmd == "config-check" => Ok(Command::ConfigCheck { path: path.clone() }),
        [cmd, path] if cmd == "home-init" => Ok(Command::HomeInit { path: path.clone() }),
        [cmd, path] if cmd == "control-boundary-demo" => {
            Ok(Command::ControlBoundaryDemo { path: path.clone() })
        }
        [cmd, path] if cmd == "mainline-demo" => Ok(Command::MainlineDemo { path: path.clone() }),
        [cmd, path, input] if cmd == "runtime-demo" => Ok(Command::RuntimeDemo {
            path: path.clone(),
            input: input.clone(),
        }),
        [cmd, path, input] if cmd == "debug-projection" => Ok(Command::DebugProjection {
            path: path.clone(),
            input: input.clone(),
        }),
        [cmd, path, transcript_path] if cmd == "transcript-demo" => Ok(Command::TranscriptDemo {
            path: path.clone(),
            transcript_path: transcript_path.clone(),
        }),
        [cmd, path] if cmd == "web-debug" => Ok(Command::WebDebug {
            path: path.clone(),
            port: 4040,
        }),
        [cmd, path, port] if cmd == "web-debug" => Ok(Command::WebDebug {
            path: path.clone(),
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
