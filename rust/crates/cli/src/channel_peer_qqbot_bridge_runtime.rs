use super::{CliError, QQBOT_CHANNEL_ID, handle_stdout_line, record_builtin_qqbot_runtime_event};
use crate::{
    channel_peer_connectivity::QqbotCredentials, process_utils::append_log,
    web_debug::CliDebugActionHandler,
};
use serde_json::json;
use std::{
    fs,
    io::BufRead,
    path::{Path, PathBuf},
    process::{Child, ChildStderr, ChildStdout, Command, Stdio},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread::{self, JoinHandle},
    time::Duration,
};

use super::support::write_bridge_request;

const BRIDGE_RESTART_BACKOFF_MS: u64 = 3000;

pub(super) fn ensure_runner_entry(
    runtime_home: &Path,
    runner_source: &str,
) -> Result<PathBuf, CliError> {
    let runner_dir = runtime_home.join("runtime/peers/qqbot/bin");
    fs::create_dir_all(&runner_dir).map_err(|source| CliError::WriteFile {
        path: runner_dir.display().to_string(),
        source,
    })?;
    let runner_path = runner_dir.join("qqbot-peer-runner.mjs");
    let needs_write = fs::read_to_string(&runner_path)
        .map(|content| content != runner_source)
        .unwrap_or(true);
    if needs_write {
        fs::write(&runner_path, runner_source).map_err(|source| CliError::WriteFile {
            path: runner_path.display().to_string(),
            source,
        })?;
    }
    Ok(runner_path)
}

pub(super) fn spawn_bridge_supervisor(
    runtime_home: PathBuf,
    handler: CliDebugActionHandler,
    runner_entry: PathBuf,
    credentials: QqbotCredentials,
    stdin: Arc<Mutex<Option<std::process::ChildStdin>>>,
    stop_signal: Arc<AtomicBool>,
) -> JoinHandle<()> {
    thread::spawn(move || {
        let mut restart_count = 0_u64;
        while !stop_signal.load(Ordering::SeqCst) {
            if let Err(err) = run_bridge_once(
                &runtime_home,
                &handler,
                &runner_entry,
                &credentials,
                &stdin,
                &stop_signal,
            ) {
                let _ = record_builtin_qqbot_runtime_event(
                    &runtime_home,
                    "channel.peer.bridge_supervisor_error",
                    Some("bridge_error"),
                    None,
                    None,
                    json!({ "error": err.to_string(), "restart_count": restart_count }),
                );
            }
            if stop_signal.load(Ordering::SeqCst) {
                break;
            }
            restart_count = restart_count.saturating_add(1);
            let _ = record_builtin_qqbot_runtime_event(
                &runtime_home,
                "channel.peer.bridge_restart_scheduled",
                Some("bridge_restarting"),
                None,
                None,
                json!({
                    "restart_count": restart_count,
                    "backoff_ms": BRIDGE_RESTART_BACKOFF_MS,
                    "channel_id": QQBOT_CHANNEL_ID,
                }),
            );
            thread::sleep(Duration::from_millis(BRIDGE_RESTART_BACKOFF_MS));
        }
    })
}

fn run_bridge_once(
    runtime_home: &Path,
    handler: &CliDebugActionHandler,
    runner_entry: &Path,
    credentials: &QqbotCredentials,
    stdin_slot: &Arc<Mutex<Option<std::process::ChildStdin>>>,
    stop_signal: &Arc<AtomicBool>,
) -> Result<(), CliError> {
    let mut child = spawn_runner_process(runner_entry)?;
    let child_pid = child.id();
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| CliError::ChannelConnectivity("qqbot runner stdout unavailable".into()))?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| CliError::ChannelConnectivity("qqbot runner stderr unavailable".into()))?;
    let child_stdin = child
        .stdin
        .take()
        .ok_or_else(|| CliError::ChannelConnectivity("qqbot runner stdin unavailable".into()))?;
    {
        let mut slot = stdin_slot.lock().map_err(|_| {
            CliError::ChannelConnectivity("qqbot bridge stdin lock poisoned".into())
        })?;
        *slot = Some(child_stdin);
    }
    record_builtin_qqbot_runtime_event(
        runtime_home,
        "channel.peer.bridge_spawned",
        Some("bridge_spawned"),
        None,
        None,
        json!({
            "runner_entry": runner_entry.display().to_string(),
            "channel_id": QQBOT_CHANNEL_ID,
            "bridge_impl": "fin_builtin_runner",
            "pid": child_pid,
        }),
    )?;
    write_bridge_request(
        stdin_slot,
        "start",
        Some(json!({ "appId": credentials.app_id, "clientSecret": credentials.client_secret })),
    )?;
    record_builtin_qqbot_runtime_event(
        runtime_home,
        "channel.peer.bridge_start_requested",
        Some("bridge_starting"),
        Some("connecting"),
        None,
        json!({
            "credential_source": credentials.source,
            "channel_id": QQBOT_CHANNEL_ID,
            "bridge_impl": "fin_builtin_runner",
            "pid": child_pid,
        }),
    )?;
    let stderr_thread = spawn_stderr_loop(runtime_home.to_path_buf(), stderr);
    run_stdout_loop(runtime_home, handler, stdin_slot, stdout, stop_signal);
    let _ = stderr_thread.join();
    if let Ok(mut slot) = stdin_slot.lock() {
        *slot = None;
    }
    let status = child.wait().map_err(|source| CliError::ReadFile {
        path: runner_entry.display().to_string(),
        source,
    })?;
    let _ = record_builtin_qqbot_runtime_event(
        runtime_home,
        "channel.peer.bridge_process_exited",
        Some(if stop_signal.load(Ordering::SeqCst) {
            "bridge_stopped"
        } else {
            "bridge_exited"
        }),
        None,
        None,
        json!({ "pid": child_pid, "exit_code": status.code() }),
    );
    Ok(())
}

fn spawn_runner_process(runner_entry: &Path) -> Result<Child, CliError> {
    Command::new("node")
        .arg(runner_entry)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|source| CliError::ReadFile {
            path: runner_entry.display().to_string(),
            source,
        })
}

fn run_stdout_loop(
    runtime_home: &Path,
    handler: &CliDebugActionHandler,
    stdin: &Arc<Mutex<Option<std::process::ChildStdin>>>,
    stdout: ChildStdout,
    stop_signal: &Arc<AtomicBool>,
) {
    let reader = std::io::BufReader::new(stdout);
    for line in reader.lines().map_while(Result::ok) {
        if stop_signal.load(Ordering::SeqCst) {
            break;
        }
        if line.trim().is_empty() {
            continue;
        }
        if let Err(err) = handle_stdout_line(runtime_home, handler, stdin, &line) {
            let _ = record_builtin_qqbot_runtime_event(
                runtime_home,
                "channel.peer.bridge_line_error",
                Some("bridge_degraded"),
                None,
                None,
                json!({ "error": err.to_string(), "line_preview": super::support::shorten(&line, 240) }),
            );
        }
    }
}

fn spawn_stderr_loop(runtime_home: PathBuf, stderr: ChildStderr) -> JoinHandle<()> {
    thread::spawn(move || {
        let reader = std::io::BufReader::new(stderr);
        let log_path = runtime_home.join("runtime/peers/qqbot/bridge.stderr.log");
        for line in reader.lines().map_while(Result::ok) {
            let _ = append_log(&log_path, &(line + "\n"));
        }
    })
}
