use thiserror::Error;

#[derive(Debug, Error)]
pub enum CliError {
    #[error(
        "usage: fin <config-check|home-init|runtime-demo|debug-projection|transcript-demo|web-debug|build-dev|install-dev|promote|rollback> <user.toml> [input|port|build-version]"
    )]
    Usage,
    #[error("invalid port: {0}")]
    InvalidPort(String),
    #[error("invalid build version: {0}")]
    InvalidBuildVersion(String),
    #[error("command failed: {command} (exit={exit_code:?})")]
    ProcessFailed {
        command: String,
        exit_code: Option<i32>,
    },
    #[error("missing install target: {0}")]
    MissingInstallTarget(String),
    #[error("invalid install state: {0}")]
    InvalidInstallState(String),
    #[error("failed to read file '{path}': {source}")]
    ReadFile {
        path: String,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to write file '{path}': {source}")]
    WriteFile {
        path: String,
        #[source]
        source: std::io::Error,
    },
    #[error(transparent)]
    Config(#[from] fin_config::ConfigError),
    #[error(transparent)]
    Provider(#[from] fin_provider::ProviderError),
    #[error(transparent)]
    Runtime(#[from] fin_runtime::RuntimeError),
    #[error(transparent)]
    DebugData(#[from] fin_debug_server::DebugDataError),
    #[error(transparent)]
    Serialize(#[from] serde_json::Error),
}