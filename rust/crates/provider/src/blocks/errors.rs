//! Provider error block — stable error taxonomy, no I/O or orchestration.

use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ProviderError {
    #[error("duplicate provider '{name}'")]
    DuplicateProvider { name: String },
    #[error("unsupported protocol for real execution: {protocol:?}")]
    UnsupportedProtocol {
        protocol: fin_config::ProviderProtocol,
    },
    #[error("missing provider credential env '{env_var}'")]
    MissingCredentialEnv { env_var: String },
    #[error("invalid header '{name}': {message}")]
    InvalidHeader { name: String, message: String },
    #[error("http status {status}: {body}")]
    HttpStatus { status: u16, body: String },
    #[error("request failed: {message}")]
    Request { message: String },
    #[error("response parse failed: {message}")]
    ParseResponse { message: String },
}
