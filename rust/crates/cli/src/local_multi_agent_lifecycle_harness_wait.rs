use crate::CliError;
use fin_runtime::AgentControlStore;
use std::path::Path;
use std::{
    thread,
    time::{Duration, Instant},
};

pub(super) fn standard_headless_command_set() -> Vec<String> {
    [
        "project-agent configure --cwd <path> --session <session-id> --port auto-persist",
        "project-agent session new --cwd <path>",
        "project-agent session resume --session <session-id>",
        "project-agent slash <command>",
        "project-agent control status",
        "project-agent control dispatch --task <task-id>",
        "project-agent control stop --pid <pid>",
        "project-agent ledger check --strict",
    ]
    .into_iter()
    .map(str::to_string)
    .collect()
}

pub(super) fn wait_for_path(path: &Path, timeout: Duration) -> Result<(), CliError> {
    let start = Instant::now();
    loop {
        if path.exists() {
            return Ok(());
        }
        if start.elapsed() >= timeout {
            return Err(CliError::Runtime(fin_runtime::RuntimeError::State(
                format!("timed out waiting path {}", path.display()),
            )));
        }
        thread::sleep(Duration::from_millis(50));
    }
}

pub(super) fn headless_command_set_covers_headed_controls(commands: &[String]) -> bool {
    [
        "--cwd",
        "--session",
        "slash",
        "control status",
        "control stop",
        "ledger check",
    ]
    .iter()
    .all(|needle| commands.iter().any(|command| command.contains(needle)))
}

pub(super) fn wait_for_mailbox_kind(
    control: &AgentControlStore,
    agent_id: &str,
    kind: &str,
    timeout: Duration,
) -> Result<serde_json::Value, CliError> {
    let start = Instant::now();
    loop {
        let inbox = control
            .read_mailbox(agent_id)
            .map_err(fin_runtime::RuntimeError::State)
            .map_err(CliError::Runtime)?;
        if let Some(message) = inbox.iter().find(|message| {
            message.payload.get("kind").and_then(|value| value.as_str()) == Some(kind)
        }) {
            return Ok(message.payload.clone());
        }
        if start.elapsed() >= timeout {
            return Err(CliError::Runtime(fin_runtime::RuntimeError::State(
                format!(
                    "timed out waiting mailbox kind {kind}; inbox={}",
                    serde_json::to_string(&inbox).unwrap_or_else(|_| "serialize_failed".into())
                ),
            )));
        }
        thread::sleep(Duration::from_millis(50));
    }
}
