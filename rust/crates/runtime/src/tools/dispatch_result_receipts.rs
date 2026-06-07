use super::dispatch::{ToolDispatchInput, runtime_home_from_context};
use fin_contracts::ToolExecutionRecord;
use serde_json::{Value, json};
use std::{fs, path::Path, path::PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FailureFeedback {
    pub(crate) failure_kind: &'static str,
    pub(crate) retryable: bool,
    pub(crate) retry_hint: String,
    pub(crate) correction_summary: String,
}

pub(super) fn persist_tool_result_receipt(
    input: &ToolDispatchInput<'_>,
    tool_call_id: &str,
    receipt: &Value,
) -> Result<Option<String>, String> {
    let Some(runtime_home) = runtime_home_from_context(input.context) else {
        return Ok(None);
    };
    let path = runtime_home.join(format!("runtime/tools/tool_receipts/{tool_call_id}.json"));
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    fs::write(
        &path,
        serde_json::to_vec_pretty(receipt).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    let relative = path
        .strip_prefix(&runtime_home)
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|_| path.display().to_string());
    Ok(Some(relative))
}

pub(super) fn default_tool_result_receipt(record: &ToolExecutionRecord) -> Value {
    let mut receipt = json!({
        "tool_call_id": record.tool_call_id,
        "tool_name": record.tool_name,
        "status": record.status,
        "title": record.title,
        "purpose": record.purpose,
        "target_kind": record.target_kind,
        "target_ref": record.target_ref,
        "input_summary": record.input_summary,
        "output_summary": record.output_summary,
        "error_summary": record.error_summary,
        "duration_ms": record.duration_ms,
        "side_effects": record.side_effects,
        "artifact_refs": record.artifact_refs,
        "started_at": record.started_at,
        "ended_at": record.ended_at,
    });
    if record.status == "failed" {
        if let Some(reason) = record.error_summary.as_deref() {
            let failure = derive_failure_feedback(record.tool_name.as_str(), reason);
            if let Some(obj) = receipt.as_object_mut() {
                obj.insert(
                    "failure_kind".into(),
                    Value::String(failure.failure_kind.into()),
                );
                obj.insert("retryable".into(), Value::Bool(failure.retryable));
                obj.insert("retry_hint".into(), Value::String(failure.retry_hint));
                obj.insert(
                    "correction_summary".into(),
                    Value::String(failure.correction_summary),
                );
            }
        }
    }
    receipt
}

pub(crate) fn derive_failure_feedback(tool_name: &str, reason: &str) -> FailureFeedback {
    let trimmed = reason.trim();
    if trimmed.contains("open_stdin_session=true requires context.project.runtime_home") {
        return FailureFeedback { failure_kind: "missing_runtime_home", retryable: false, retry_hint: "open_stdin_session needs a runtime-bound context so the exec session can be persisted; retry from a session with context.project.runtime_home".into(), correction_summary: "exec replay session requested without runtime_home-backed persistence".into() };
    }
    if trimmed.contains("old_string may be empty only when creating a new file") {
        return FailureFeedback { failure_kind: "invalid_patch_replace_contract", retryable: true, retry_hint: "retry with the exact current old_string from the file, or use `write_file` when you intend a full-file overwrite".into(), correction_summary: "apply_patch replace call used an invalid empty old_string".into() };
    }
    if trimmed.contains("command exited with non-zero status:")
        || trimmed.contains("replay command exited with non-zero status:")
    {
        return FailureFeedback { failure_kind: "command_non_zero_exit", retryable: true, retry_hint: "the command ran but failed; inspect stdout/stderr in the receipt, correct the command or environment, and retry only after the non-zero exit cause is addressed".into(), correction_summary: "shell command completed with a non-zero exit status".into() };
    }
    if trimmed.starts_with("failed to spawn command:") {
        return FailureFeedback { failure_kind: "command_spawn_failed", retryable: true, retry_hint: "the shell command could not start; verify cmd, cwd, and required binaries are valid in this environment before retrying".into(), correction_summary: "shell command failed before execution started".into() };
    }
    if trimmed.contains("missing required argument: session_id") {
        return FailureFeedback { failure_kind: "missing_session_reference", retryable: true, retry_hint: "retry with a concrete session_id; for interactive stdin, first create/open the exec session and then pass its session_id".into(), correction_summary: "tool call lacked required session reference".into() };
    }
    FailureFeedback {
        failure_kind: "tool_execution_failed",
        retryable: true,
        retry_hint: format!(
            "inspect the exact error_summary, correct the tool arguments or target state, and retry {tool_name} only after the cause is addressed"
        ),
        correction_summary: {
            let mut chars = trimmed.chars();
            let s: String = chars.by_ref().take(160).collect();
            if chars.next().is_some() {
                format!("{s}…")
            } else {
                s
            }
        },
    }
}
