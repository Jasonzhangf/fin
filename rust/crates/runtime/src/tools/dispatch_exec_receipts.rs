use super::dispatch::{ToolDispatchInput, runtime_home_from_context};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(super) struct ExecCommandReceipt {
    pub(super) tool_call_id: String,
    pub(super) tool_name: String,
    pub(super) cmd: String,
    pub(super) cwd: Option<String>,
    pub(super) open_stdin_session: bool,
    pub(super) session_id: Option<String>,
    pub(super) exit_code: i32,
    pub(super) stdout: String,
    pub(super) stderr: String,
    pub(super) duration_ms: u64,
    pub(super) occurred_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(super) struct WriteStdinReceipt {
    pub(super) tool_call_id: String,
    pub(super) tool_name: String,
    pub(super) session_id: String,
    pub(super) chars: String,
    pub(super) exit_code: i32,
    pub(super) stdout: String,
    pub(super) stderr: String,
    pub(super) duration_ms: u64,
    pub(super) run_count: u64,
    pub(super) occurred_at: String,
}

pub(super) fn persist_exec_receipt(
    input: &ToolDispatchInput<'_>,
    tool_call_id: &str,
    receipt: &ExecCommandReceipt,
) -> Result<Option<String>, String> {
    persist_receipt(
        input,
        &format!("runtime/tools/exec_receipts/{tool_call_id}.json"),
        receipt,
    )
}

pub(super) fn persist_write_stdin_receipt(
    input: &ToolDispatchInput<'_>,
    tool_call_id: &str,
    receipt: &WriteStdinReceipt,
) -> Result<Option<String>, String> {
    persist_receipt(
        input,
        &format!("runtime/tools/write_stdin_receipts/{tool_call_id}.json"),
        receipt,
    )
}

fn persist_receipt<T: Serialize>(
    input: &ToolDispatchInput<'_>,
    relative_path: &str,
    receipt: &T,
) -> Result<Option<String>, String> {
    let Some(runtime_home) = runtime_home_from_context(input.context) else {
        return Ok(None);
    };
    let path = runtime_home.join(relative_path);
    write_json(&path, receipt)?;
    Ok(Some(relative_artifact(input, &path)))
}

fn write_json<T: Serialize>(path: &Path, value: &T) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|err| err.to_string())?;
    }
    fs::write(
        path,
        serde_json::to_vec_pretty(value).map_err(|err| err.to_string())?,
    )
    .map_err(|err| err.to_string())
}

fn relative_artifact(input: &ToolDispatchInput<'_>, absolute: &PathBuf) -> String {
    runtime_home_from_context(input.context)
        .and_then(|home| {
            absolute
                .strip_prefix(home)
                .ok()
                .map(|value| value.to_string_lossy().to_string())
        })
        .unwrap_or_else(|| absolute.display().to_string())
}
