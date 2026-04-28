use crate::tool_dispatch::{ToolDispatchInput, runtime_home_from_context};
use fin_contracts::ToolExecutionRecord;
use serde_json::{Value, json};
use std::{
    fs,
    path::{Path, PathBuf},
};

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
    write_json(&path, receipt)?;
    Ok(Some(relative_artifact(&runtime_home, &path)))
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
            if let Some(object) = receipt.as_object_mut() {
                object.insert(
                    "failure_kind".into(),
                    Value::String(failure.failure_kind.into()),
                );
                object.insert("retryable".into(), Value::Bool(failure.retryable));
                object.insert("retry_hint".into(), Value::String(failure.retry_hint));
                object.insert(
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
        return FailureFeedback {
            failure_kind: "missing_runtime_home",
            retryable: false,
            retry_hint:
                "open_stdin_session needs a runtime-bound context so the exec session can be persisted; retry from a session with context.project.runtime_home"
                    .into(),
            correction_summary:
                "exec replay session requested without runtime_home-backed persistence".into(),
        };
    }
    if let Some(argument) = trimmed.strip_prefix("missing required argument:") {
        let argument = argument.trim();
        return FailureFeedback {
            failure_kind: "missing_argument",
            retryable: true,
            retry_hint: format!(
                "retry {tool_name} with the required argument `{argument}` filled in using a concrete value"
            ),
            correction_summary: format!("missing required argument `{argument}`"),
        };
    }
    if trimmed.contains("owner self-claim rejected") && trimmed.contains("agent.assign") {
        return FailureFeedback {
            failure_kind: "owner_mode_dispatch_violation",
            retryable: true,
            retry_hint:
                "do not claim the ready task as the owner; call `agent.assign` first and let the target worker call `project.task.claim`"
                    .into(),
            correction_summary: "owner attempted self-claim while visible worker capacity exists"
                .into(),
        };
    }
    if trimmed.contains("restricted to markdown management artifacts") {
        return FailureFeedback {
            failure_kind: "owner_mode_write_scope_violation",
            retryable: true,
            retry_hint:
                "write a markdown management artifact under `plans/`, `reports/`, `reviews/`, or `status/`, or delegate the non-markdown file change to a worker/execution path"
                    .into(),
            correction_summary: "owner-mode file write target is outside the allowed markdown management scope".into(),
        };
    }
    if trimmed.contains("task not found in runtime truth:") {
        return FailureFeedback {
            failure_kind: "missing_runtime_target",
            retryable: true,
            retry_hint:
                "inspect current task/session truth first (for example via status/list tools), then retry with a valid existing target id"
                    .into(),
            correction_summary: "referenced runtime target does not exist".into(),
        };
    }
    if trimmed.contains("missing context.project.runtime_home") {
        return FailureFeedback {
            failure_kind: "missing_runtime_home",
            retryable: false,
            retry_hint:
                "this turn lacks runtime_home in context; switch to a runtime-bound session/context before retrying this persistence-backed tool"
                    .into(),
            correction_summary: "tool requires runtime_home-backed persistence context".into(),
        };
    }
    if trimmed.contains("exec replay session not found:") {
        return FailureFeedback {
            failure_kind: "missing_session_reference",
            retryable: true,
            retry_hint:
                "the exec replay session does not exist in runtime truth; first call `exec_command` with `open_stdin_session=true` or retry with a valid existing session_id"
                    .into(),
            correction_summary: "write_stdin referenced a missing exec replay session".into(),
        };
    }
    if trimmed.contains("missing worker_id and refs.worker_id is unavailable")
        || trimmed.contains("missing worker_id and current refs.worker_id is unavailable")
    {
        return FailureFeedback {
            failure_kind: "missing_worker_binding",
            retryable: true,
            retry_hint:
                "retry with an explicit `worker_id`, or ensure the current refs include a bound worker before using this task-write action"
                    .into(),
            correction_summary: "worker-bound tool call lacked worker identity".into(),
        };
    }
    if trimmed.contains("missing task_id and refs.task_id is unavailable")
        || trimmed.contains("missing task_id and current refs.task_id is unavailable")
    {
        return FailureFeedback {
            failure_kind: "missing_task_binding",
            retryable: true,
            retry_hint:
                "retry with an explicit `task_id`, or switch to a context whose refs already bind the intended task"
                    .into(),
            correction_summary: "task-bound tool call lacked task identity".into(),
        };
    }
    if trimmed.starts_with("peer_id '") && trimmed.contains("not found in current context snapshot")
    {
        return FailureFeedback {
            failure_kind: "missing_context_target",
            retryable: true,
            retry_hint:
                "the requested peer is not present in current context.peer truth; call `peer.list` first, then retry `peer.describe` or routing with a listed peer_id"
                    .into(),
            correction_summary: "requested peer_id is absent from the current peer snapshot"
                .into(),
        };
    }
    if trimmed.starts_with("capability_id '")
        && trimmed.contains("not found in current peer context descriptors")
    {
        return FailureFeedback {
            failure_kind: "missing_context_capability",
            retryable: true,
            retry_hint:
                "inspect the current peer descriptors first (for example with `peer.list` / `peer.describe`) and retry with a capability_id that is actually advertised"
                    .into(),
            correction_summary:
                "requested capability_id is absent from the current peer descriptors".into(),
        };
    }
    if trimmed == "current agent presence registry is unavailable" {
        return FailureFeedback {
            failure_kind: "missing_runtime_snapshot",
            retryable: true,
            retry_hint:
                "this runtime has not materialized agent presence truth yet; run the startup/daemon path that produces `current_agent_presence_registry.json`, then retry"
                    .into(),
            correction_summary: "agent presence snapshot is not yet available in runtime truth"
                .into(),
        };
    }
    if trimmed == "current project supervision snapshot is unavailable" {
        return FailureFeedback {
            failure_kind: "missing_runtime_snapshot",
            retryable: true,
            retry_hint:
                "this runtime has not materialized project supervision truth yet; produce `current_project_supervision.json` first, then retry the supervision query"
                    .into(),
            correction_summary:
                "project supervision snapshot is not yet available in runtime truth".into(),
        };
    }
    if trimmed.contains("unsupported apply_patch mode:") {
        return FailureFeedback {
            failure_kind: "unsupported_mode",
            retryable: true,
            retry_hint:
                "retry apply_patch with `mode=replace` for one exact in-file change or `mode=patch` for a multi-file patch"
                    .into(),
            correction_summary: "tool mode is not supported by runtime".into(),
        };
    }
    if trimmed.contains("does not exist; old_string must be empty to create a new file") {
        return FailureFeedback {
            failure_kind: "create_vs_patch_mismatch",
            retryable: true,
            retry_hint:
                "for a new file, use `write_file` or call `apply_patch` with `old_string=\"\"`; do not patch a missing file with a non-empty old_string"
                    .into(),
            correction_summary: "tool attempted patch semantics on a missing file".into(),
        };
    }
    if trimmed.contains("old_string may be empty only when creating a new file") {
        return FailureFeedback {
            failure_kind: "invalid_patch_replace_contract",
            retryable: true,
            retry_hint:
                "retry with the exact current old_string from the file, or use `write_file` when you intend a full-file overwrite"
                    .into(),
            correction_summary: "apply_patch replace call used an invalid empty old_string".into(),
        };
    }
    if trimmed.contains("command exited with non-zero status:")
        || trimmed.contains("replay command exited with non-zero status:")
    {
        return FailureFeedback {
            failure_kind: "command_non_zero_exit",
            retryable: true,
            retry_hint:
                "the command ran but failed; inspect stdout/stderr in the receipt, correct the command or environment, and retry only after the non-zero exit cause is addressed"
                    .into(),
            correction_summary: "shell command completed with a non-zero exit status".into(),
        };
    }
    if trimmed.starts_with("failed to spawn command:") {
        return FailureFeedback {
            failure_kind: "command_spawn_failed",
            retryable: true,
            retry_hint:
                "the shell command could not start; verify `cmd`, `cwd`, and required binaries are valid in this environment before retrying"
                    .into(),
            correction_summary: "shell command failed before execution started".into(),
        };
    }
    if trimmed.contains("missing required argument: session_id") {
        return FailureFeedback {
            failure_kind: "missing_session_reference",
            retryable: true,
            retry_hint:
                "retry with a concrete `session_id`; for interactive stdin, first create/open the exec session and then pass its session_id"
                    .into(),
            correction_summary: "tool call lacked required session reference".into(),
        };
    }
    FailureFeedback {
        failure_kind: "tool_execution_failed",
        retryable: true,
        retry_hint: format!(
            "inspect the exact error_summary, correct the tool arguments or target state, and retry {tool_name} only after the cause is addressed"
        ),
        correction_summary: short_error(trimmed),
    }
}

pub(super) fn is_authoritative_receipt_ref(artifact_ref: &str) -> bool {
    artifact_ref.contains("/exec_receipts/")
        || artifact_ref.contains("/write_stdin_receipts/")
        || artifact_ref.contains("/patch_receipts/")
        || artifact_ref.contains("/tool_receipts/")
}

fn write_json(path: &Path, value: &Value) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|err| err.to_string())?;
    }
    fs::write(
        path,
        serde_json::to_vec_pretty(value).map_err(|err| err.to_string())?,
    )
    .map_err(|err| err.to_string())
}

fn relative_artifact(runtime_home: &Path, absolute: &PathBuf) -> String {
    absolute
        .strip_prefix(runtime_home)
        .map(|relative| relative.to_string_lossy().to_string())
        .unwrap_or_else(|_| absolute.display().to_string())
}

fn short_error(value: &str) -> String {
    let mut chars = value.chars();
    let out = chars.by_ref().take(160).collect::<String>();
    if chars.next().is_some() {
        format!("{out}…")
    } else {
        out
    }
}
