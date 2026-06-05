use super::tool_dispatch::{
    ToolDispatchInput, ToolDispatchOutcome, failed_record, read_bool, read_string,
    runtime_home_from_context, short_text,
};
use super::tool_dispatch as tool_dispatch;
use fin_contracts::ToolExecutionRecord;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    fs,
    io::Write,
    path::Path,
    process::{Command, Stdio},
    time::Instant,
};

#[path = "tool_dispatch_extended_exec_receipts.rs"]
mod tool_dispatch_extended_exec_receipts;
use super::tool_dispatch_extended_exec_receipts::{
    ExecCommandReceipt, WriteStdinReceipt, persist_exec_receipt, persist_write_stdin_receipt,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct ExecReplaySession {
    session_id: String,
    cmd: String,
    cwd: Option<String>,
    created_at: String,
    last_exit_code: Option<i32>,
    last_stdout: String,
    last_stderr: String,
    run_count: u64,
}

#[derive(Debug, Clone)]
struct CommandRun {
    exit_code: i32,
    stdout: String,
    stderr: String,
    duration_ms: u64,
}

pub(super) fn handle_exec_command(
    outcome: &mut ToolDispatchOutcome,
    input: &ToolDispatchInput<'_>,
    tool_call_id: &str,
    arguments: &Value,
) -> bool {
    let Some(cmd) = read_string(arguments, "cmd").or_else(|| read_string(arguments, "command"))
    else {
        outcome.tool_records.push(failed_record(
            input,
            tool_call_id.into(),
            "exec_command",
            "missing required argument: cmd",
        ));
        return true;
    };
    let cwd = read_string(arguments, "cwd");
    let open_stdin_session = read_bool(arguments, "open_stdin_session").unwrap_or(false);

    let run = match run_command(cmd.as_str(), cwd.as_deref(), None) {
        Ok(value) => value,
        Err(err) => {
            outcome.tool_records.push(failed_record(
                input,
                tool_call_id.into(),
                "exec_command",
                err.as_str(),
            ));
            return true;
        }
    };

    let mut artifact_refs = Vec::new();
    let mut session_id: Option<String> = None;
    if open_stdin_session {
        if let Some(runtime_home) = runtime_home_from_context(input.context) {
            let new_session_id = format!("exec-replay-{}-{tool_call_id}", input.operation_id);
            let session = ExecReplaySession {
                session_id: new_session_id.clone(),
                cmd: cmd.clone(),
                cwd: cwd.clone(),
                created_at: input.occurred_at.into(),
                last_exit_code: Some(run.exit_code),
                last_stdout: run.stdout.clone(),
                last_stderr: run.stderr.clone(),
                run_count: 1,
            };
            let path =
                runtime_home.join(format!("runtime/tools/exec_sessions/{new_session_id}.json"));
            if let Err(err) = write_json(&path, &session) {
                outcome.tool_records.push(failed_record(
                    input,
                    tool_call_id.into(),
                    "exec_command",
                    format!("failed to persist exec replay session: {err}").as_str(),
                ));
                return true;
            }
            artifact_refs.push(relative_artifact(input.context, &path));
            session_id = Some(new_session_id);
        } else {
            outcome.tool_records.push(failed_record(
                input,
                tool_call_id.into(),
                "exec_command",
                "open_stdin_session=true requires context.project.runtime_home",
            ));
            return true;
        }
    }

    match persist_exec_receipt(
        input,
        tool_call_id,
        &ExecCommandReceipt {
            tool_call_id: tool_call_id.into(),
            tool_name: "exec_command".into(),
            cmd: cmd.clone(),
            cwd: cwd.clone(),
            open_stdin_session,
            session_id: session_id.clone(),
            exit_code: run.exit_code,
            stdout: run.stdout.clone(),
            stderr: run.stderr.clone(),
            duration_ms: run.duration_ms,
            occurred_at: input.occurred_at.into(),
        },
    ) {
        Ok(Some(receipt_ref)) => artifact_refs.push(receipt_ref),
        Ok(None) => {}
        Err(err) => {
            outcome.tool_records.push(failed_record(
                input,
                tool_call_id.into(),
                "exec_command",
                format!("failed to persist exec receipt: {err}").as_str(),
            ));
            return true;
        }
    }

    let status = if run.exit_code == 0 {
        "completed"
    } else {
        "failed"
    };
    let output_summary = format!(
        "exit_code={}, stdout={}, stderr={}{}",
        run.exit_code,
        short_text(run.stdout.as_str(), 120),
        short_text(run.stderr.as_str(), 120),
        session_id
            .as_ref()
            .map(|value| format!(", session_id={value}"))
            .unwrap_or_default()
    );
    outcome.tool_records.push(ToolExecutionRecord {
        tool_call_id: tool_call_id.into(),
        operation_id: input.operation_id.into(),
        trace_id: input.trace_id.into(),
        refs: input.refs.clone(),
        tool_name: "exec_command".into(),
        tool_kind: "agent_tool".into(),
        title: "Execute Local Command".into(),
        purpose: "run one local shell command and capture deterministic stdout/stderr result"
            .into(),
        target_kind: Some("local_shell".into()),
        target_ref: cwd.clone(),
        input_summary: Some(format!("cmd={}{}", cmd, cwd_suffix(&cwd))),
        output_summary: Some(output_summary),
        status: status.into(),
        started_at: input.occurred_at.into(),
        ended_at: Some(input.occurred_at.into()),
        duration_ms: Some(run.duration_ms),
        side_effects: vec!["spawn_local_command".into()],
        artifact_refs,
        error_summary: if run.exit_code == 0 {
            None
        } else {
            Some(format!(
                "command exited with non-zero status: {}",
                run.exit_code
            ))
        },
    });
    outcome.events.push((
        "tool.exec_command_completed".into(),
        json!({
            "tool_call_id": tool_call_id,
            "exit_code": run.exit_code,
            "duration_ms": run.duration_ms,
            "session_id": session_id,
            "cwd": cwd,
        }),
    ));
    outcome.note_hints.push(format!(
        "exec_command exit={} stdout={} stderr={}",
        run.exit_code,
        short_text(run.stdout.as_str(), 80),
        short_text(run.stderr.as_str(), 80)
    ));
    true
}

pub(super) fn handle_write_stdin(
    outcome: &mut ToolDispatchOutcome,
    input: &ToolDispatchInput<'_>,
    tool_call_id: &str,
    arguments: &Value,
) -> bool {
    let Some(session_id) = read_string(arguments, "session_id") else {
        outcome.tool_records.push(failed_record(
            input,
            tool_call_id.into(),
            "write_stdin",
            "missing required argument: session_id",
        ));
        return true;
    };
    let chars = read_string(arguments, "chars").unwrap_or_default();

    let Some(runtime_home) = runtime_home_from_context(input.context) else {
        outcome.tool_records.push(failed_record(
            input,
            tool_call_id.into(),
            "write_stdin",
            "missing context.project.runtime_home, cannot resolve exec replay session",
        ));
        return true;
    };

    let path = runtime_home.join(format!("runtime/tools/exec_sessions/{session_id}.json"));
    let mut session = match read_json::<ExecReplaySession>(&path) {
        Ok(Some(value)) => value,
        Ok(None) => {
            outcome.tool_records.push(failed_record(
                input,
                tool_call_id.into(),
                "write_stdin",
                format!("exec replay session not found: {session_id}").as_str(),
            ));
            return true;
        }
        Err(err) => {
            outcome.tool_records.push(failed_record(
                input,
                tool_call_id.into(),
                "write_stdin",
                format!("failed to read exec replay session: {err}").as_str(),
            ));
            return true;
        }
    };

    let run = match run_command(
        session.cmd.as_str(),
        session.cwd.as_deref(),
        Some(chars.as_str()),
    ) {
        Ok(value) => value,
        Err(err) => {
            outcome.tool_records.push(failed_record(
                input,
                tool_call_id.into(),
                "write_stdin",
                err.as_str(),
            ));
            return true;
        }
    };

    session.last_exit_code = Some(run.exit_code);
    session.last_stdout = run.stdout.clone();
    session.last_stderr = run.stderr.clone();
    session.run_count += 1;
    if let Err(err) = write_json(&path, &session) {
        outcome.tool_records.push(failed_record(
            input,
            tool_call_id.into(),
            "write_stdin",
            format!("failed to update exec replay session: {err}").as_str(),
        ));
        return true;
    }

    let receipt_ref = match persist_write_stdin_receipt(
        input,
        tool_call_id,
        &WriteStdinReceipt {
            tool_call_id: tool_call_id.into(),
            tool_name: "write_stdin".into(),
            session_id: session_id.clone(),
            chars: chars.clone(),
            exit_code: run.exit_code,
            stdout: run.stdout.clone(),
            stderr: run.stderr.clone(),
            duration_ms: run.duration_ms,
            run_count: session.run_count,
            occurred_at: input.occurred_at.into(),
        },
    ) {
        Ok(value) => value,
        Err(err) => {
            outcome.tool_records.push(failed_record(
                input,
                tool_call_id.into(),
                "write_stdin",
                format!("failed to persist write_stdin receipt: {err}").as_str(),
            ));
            return true;
        }
    };

    outcome.tool_records.push(ToolExecutionRecord {
        tool_call_id: tool_call_id.into(),
        operation_id: input.operation_id.into(),
        trace_id: input.trace_id.into(),
        refs: input.refs.clone(),
        tool_name: "write_stdin".into(),
        tool_kind: "agent_tool".into(),
        title: "Write Stdin (Replay Session)".into(),
        purpose: "re-run the stored exec replay command with provided stdin payload".into(),
        target_kind: Some("exec_replay_session".into()),
        target_ref: Some(session_id.clone()),
        input_summary: Some(format!(
            "session_id={session_id}, chars={}",
            short_text(chars.as_str(), 80)
        )),
        output_summary: Some(format!(
            "exit_code={}, stdout={}, stderr={}",
            run.exit_code,
            short_text(run.stdout.as_str(), 120),
            short_text(run.stderr.as_str(), 120)
        )),
        status: if run.exit_code == 0 {
            "completed".into()
        } else {
            "failed".into()
        },
        started_at: input.occurred_at.into(),
        ended_at: Some(input.occurred_at.into()),
        duration_ms: Some(run.duration_ms),
        side_effects: vec![
            "replay_command_with_stdin".into(),
            "update_exec_session_state".into(),
        ],
        artifact_refs: vec![relative_artifact(input.context, &path)],
        error_summary: if run.exit_code == 0 {
            None
        } else {
            Some(format!(
                "replay command exited with non-zero status: {}",
                run.exit_code
            ))
        },
    });
    if let Some(receipt_ref) = receipt_ref {
        if let Some(record) = outcome.tool_records.last_mut() {
            record.artifact_refs.push(receipt_ref);
        }
    }
    outcome.events.push((
        "tool.write_stdin_completed".into(),
        json!({
            "tool_call_id": tool_call_id,
            "session_id": session_id,
            "exit_code": run.exit_code,
            "duration_ms": run.duration_ms,
            "run_count": session.run_count,
        }),
    ));
    true
}

fn run_command(cmd: &str, cwd: Option<&str>, stdin: Option<&str>) -> Result<CommandRun, String> {
    let started = Instant::now();
    let mut command = Command::new("zsh");
    command.arg("-lc").arg(cmd);
    if let Some(path) = cwd.filter(|value| !value.trim().is_empty()) {
        command.current_dir(path);
    }
    command.stdout(Stdio::piped()).stderr(Stdio::piped());
    if stdin.is_some() {
        command.stdin(Stdio::piped());
    } else {
        command.stdin(Stdio::null());
    }

    let mut child = command
        .spawn()
        .map_err(|err| format!("failed to spawn command: {err}"))?;
    if let Some(stdin_payload) = stdin {
        let mut handle = child
            .stdin
            .take()
            .ok_or_else(|| "command stdin handle unavailable".to_string())?;
        handle
            .write_all(stdin_payload.as_bytes())
            .map_err(|err| format!("failed to write stdin payload: {err}"))?;
    }
    let output = child
        .wait_with_output()
        .map_err(|err| format!("failed waiting command output: {err}"))?;

    Ok(CommandRun {
        exit_code: output.status.code().unwrap_or(-1),
        stdout: String::from_utf8_lossy(&output.stdout).trim().to_string(),
        stderr: String::from_utf8_lossy(&output.stderr).trim().to_string(),
        duration_ms: started.elapsed().as_millis().min(u128::from(u64::MAX)) as u64,
    })
}

fn cwd_suffix(cwd: &Option<String>) -> String {
    cwd.as_ref()
        .map(|value| format!(", cwd={value}"))
        .unwrap_or_default()
}

fn read_json<T: for<'de> Deserialize<'de>>(path: &Path) -> Result<Option<T>, String> {
    match fs::read_to_string(path) {
        Ok(content) => serde_json::from_str::<T>(&content)
            .map(Some)
            .map_err(|err| err.to_string()),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(err) => Err(err.to_string()),
    }
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

fn relative_artifact(context: &fin_contracts::MinimalContextView, absolute: &Path) -> String {
    runtime_home_from_context(context)
        .and_then(|home| {
            absolute
                .strip_prefix(home)
                .ok()
                .map(|value| value.to_string_lossy().to_string())
        })
        .unwrap_or_else(|| absolute.display().to_string())
}
