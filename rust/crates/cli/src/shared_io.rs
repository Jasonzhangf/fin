use crate::CliError;
use fin_shared::SharedIoError;
use serde::{Serialize, de::DeserializeOwned};
use std::path::Path;

pub(crate) fn map_shared_io(err: SharedIoError) -> CliError {
    match err {
        SharedIoError::Io { path, source } => CliError::WriteFile { path, source },
        SharedIoError::Json { source, .. } => CliError::Serialize(source),
    }
}

pub(crate) fn shared_read_json_or_empty<T: DeserializeOwned>(
    path: &Path,
) -> Result<Vec<T>, CliError> {
    fin_shared::read_json_or_empty(path).map_err(map_shared_io)
}

pub(crate) fn shared_write_json<T: Serialize>(path: &Path, value: &T) -> Result<(), CliError> {
    fin_shared::write_json(path, value).map_err(map_shared_io)
}

pub(crate) fn shared_append_jsonl<T: Serialize>(path: &Path, value: &T) -> Result<(), CliError> {
    fin_shared::append_jsonl(path, value).map_err(map_shared_io)
}
