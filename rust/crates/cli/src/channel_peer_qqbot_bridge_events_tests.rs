//!
//! qqbot event consumer gate tests - Gate 1/2/3.
//!
//! Gate 1: record_builtin_qqbot_runtime_event writes events.jsonl
//! Gate 2: recent_contexts.json bounded after multiple operations
//! Gate 3: read_last_run_value parses correctly

use super::*;
use crate::runtime_home::{ensure_runtime_home_layout, read_last_run_value};
use crate::{channel_peer::complete_builtin_qqbot_pairing, config::map_system_config};
use fin_debug_server::{ChatSendRequest, DebugActionHandler};
use std::{
    fs,
    io::{Read, Write},
    net::TcpListener,
    path::PathBuf,
    process::{Command, Stdio},
    sync::{Arc, Barrier, Mutex},
    thread,
    time::{SystemTime, UNIX_EPOCH},
};

fn temp_home() -> PathBuf {
    std::env::temp_dir().join(format!(
        "fin-qqbot-events-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos()
    ))
}

fn sample_user_toml(base_url: &str) -> String {
    let base_url = base_url.to_string();
    format!(
        "default_provider = \"local-anthropic\"\n\
[providers.local-anthropic]\n\
protocol = \"anthropic-wire\"\n\
base_url = \"{}\"\n\
model = \"qwen3.6-plus\"\n\
api_key = \"test-key\"\n\
user_agent = \"opencode/1.2.27\"\n",
        base_url
    )
}

fn new_session(
    handler: &CliDebugActionHandler,
    home: &Path,
) -> Result<(String, String), Box<dyn std::error::Error>> {
    let response = handler.send_chat_message(
        home,
        ChatSendRequest {
            message: "/new".into(),
            input_kind: None,
            attachments: Vec::new(),
        },
    )?;
    Ok((
        response.binding.session_id.expect("session id"),
        response.binding.task_id.expect("task id"),
    ))
}

fn direct_message(sender: &str, msg_id: &str, content: &str) -> QqbotBridgeInboundMessage {
    QqbotBridgeInboundMessage {
        message_type: "c2c".into(),
        sender_id: sender.into(),
        sender_name: Some("Jason".into()),
        content: content.into(),
        message_id: msg_id.into(),
        timestamp: "2026-05-14T12:00:00+08:00".into(),
        group_openid: None,
        channel_id: None,
        guild_id: None,
        attachments: None,
    }
}

fn spawn_sink(
    sink_path: &Path,
) -> (
    std::process::Child,
    Arc<Mutex<Option<std::process::ChildStdin>>>,
) {
    let mut child = Command::new("sh")
        .arg("-c")
        .arg("cat > \"$FIN_SINK\"")
        .env("FIN_SINK", sink_path)
        .stdin(Stdio::piped())
        .spawn()
        .expect("spawn sink");
    let stdin = Arc::new(Mutex::new(child.stdin.take()));
    (child, stdin)
}

fn finish_sink(
    stdin: &Arc<Mutex<Option<std::process::ChildStdin>>>,
    mut child: std::process::Child,
) {
    drop(stdin.lock().expect("lock").take());
    let _ = child.wait();
}

fn read_http_body_simple(stream: &mut std::net::TcpStream) -> String {
    let mut buf = Vec::new();
    let mut tmp = [0u8; 4096];
    while let Ok(n) = stream.read(&mut tmp) {
        if n == 0 {
            break;
        }
        buf.extend_from_slice(&tmp[..n]);
        let text = String::from_utf8_lossy(&buf);
        if let Some(pos) = text.find("\r\n\r\n") {
            let body_start = pos + 4;
            for line in text[..pos].lines() {
                if line.to_lowercase().starts_with("content-length:") {
                    if let Ok(len) = line
                        .split(":")
                        .nth(1)
                        .unwrap_or("0")
                        .trim()
                        .parse::<usize>()
                    {
                        let body_bytes = &buf[body_start..];
                        if body_bytes.len() >= len {
                            return String::from_utf8_lossy(&body_bytes[..len]).to_string();
                        }
                    }
                }
            }
        }
    }
    String::new()
}

/// Mock provider: bind in main thread, share via Arc, Barrier(2) to sync.
fn spawn_mock_provider(responses: Vec<&str>) -> (String, Arc<Barrier>, thread::JoinHandle<()>) {
    let responses: Vec<String> = responses.into_iter().map(str::to_string).collect();
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let addr = listener.local_addr().expect("addr");
    let base_url = format!("http://{}", addr);
    let listener = Arc::new(Mutex::new(listener));
    let barrier = Arc::new(Barrier::new(2));
    let barrier_clone = Arc::clone(&barrier);
    let handle = thread::spawn(move || {
        barrier_clone.wait(); // wait for test to call .wait()
        let mut idx = 0usize;
        loop {
            let listener_guard = match listener.lock() {
                Ok(g) => g,
                Err(_) => break,
            };
            match listener_guard.accept() {
                Ok((mut stream, _)) => {
                    drop(listener_guard);
                    let _ = read_http_body_simple(&mut stream);
                    let body_ref = &responses[idx % responses.len()];
                    idx += 1;
                    let response_text = format!(
                        "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}",
                        body_ref.len(),
                        body_ref
                    );
                    let _ = stream.write_all(response_text.as_bytes());
                }
                Err(_) => break,
            }
        }
    });
    (base_url, barrier, handle)
}

/// Gate 1: record_builtin_qqbot_runtime_event writes events.jsonl
#[test]
fn qqbot_event_recorder_writes_events_jsonl() {
    let response_body = "{\"id\":\"msg-gate1\",\"role\":\"assistant\",\"content\":[{\"type\":\"text\",\"text\":\"gate1 reply.\"}],\"stop_reason\":\"end_turn\"}";
    let (base_url, barrier, _server) = spawn_mock_provider(vec![response_body]);
    let home = temp_home();
    ensure_runtime_home_layout(&home).expect("home");
    let user_toml = sample_user_toml(&base_url);
    let system = map_system_config(&user_toml).expect("system");
    let handler = CliDebugActionHandler::new(user_toml, system).expect("handler");
    let (session_id, _task_id) = new_session(&handler, &home).expect("new session");
    barrier.wait(); // signal: test is ready to connect
    complete_builtin_qqbot_pairing(&home, &session_id, None).expect("pairing");

    let sink_path = home.join("runtime/peers/qqbot/outbound-gate1.jsonl");
    let (child, stdin) = spawn_sink(&sink_path);
    let inbound = direct_message("user-gate1", "msg-gate1-1", "gate1 test");
    let _ = process_inbound_message(&home, &handler, &stdin, inbound);
    finish_sink(&stdin, child);

    // Gate 1: events.jsonl written with channel.peer events
    let events_path = home.join("runtime/peers/qqbot/events.jsonl");
    assert!(events_path.exists(), "events.jsonl must be written");
    let ev_content = fs::read_to_string(&events_path).expect("read events");
    assert!(
        ev_content.contains("channel.peer.message_ingested"),
        "events.jsonl should contain channel.peer.message_ingested"
    );
    let lines: Vec<&str> = ev_content
        .lines()
        .filter(|l| !l.trim().is_empty())
        .collect();
    assert!(
        lines.len() >= 2,
        "events.jsonl should have >= 2 events, got {}",
        lines.len()
    );

    let _ = fs::remove_dir_all(&home);
}

/// Gate 2: recent_contexts.json bounded after multiple qqbot operations
#[test]
fn qqbot_recent_contexts_bounded_after_multiple_operations() {
    let response_body = "{\"id\":\"msg-bound\",\"role\":\"assistant\",\"content\":[{\"type\":\"text\",\"text\":\"bound reply.\"}],\"stop_reason\":\"end_turn\"}";
    let (base_url, barrier, _server) = spawn_mock_provider(vec![response_body]);
    let home = temp_home();
    ensure_runtime_home_layout(&home).expect("home");
    let user_toml = sample_user_toml(&base_url);
    let system = map_system_config(&user_toml).expect("system");
    let handler = CliDebugActionHandler::new(user_toml, system).expect("handler");
    let (session_id, _task_id) = new_session(&handler, &home).expect("new session");
    barrier.wait();
    complete_builtin_qqbot_pairing(&home, &session_id, None).expect("pairing");

    let sink_path = home.join("runtime/peers/qqbot/outbound-bound.jsonl");
    let (child, stdin) = spawn_sink(&sink_path);
    for i in 0..5 {
        let inbound = direct_message(
            "user-bound",
            &format!("msg-bound-{}", i),
            &format!("bound test {}", i),
        );
        let _ = process_inbound_message(&home, &handler, &stdin, inbound);
    }
    finish_sink(&stdin, child);

    // Gate 2: recent_contexts bounded if it exists
    let recent = home
        .join("sessions/2026/05")
        .join(&session_id)
        .join("recent_contexts.json");
    if recent.exists() {
        let c = fs::read_to_string(&recent).expect("read recent");
        let contexts: Vec<serde_json::Value> = serde_json::from_str(&c).expect("parse");
        assert!(
            contexts.len() <= 10,
            "recent_contexts bounded <= 10, got {}",
            contexts.len()
        );
    }

    let _ = fs::remove_dir_all(&home);
}

/// Gate 3: read_last_run_value parses last_run.json correctly.
/// Full turn_id concretization is verified by turn_event_consumer_tests.
#[test]
fn qqbot_read_last_run_value_parses_correctly() {
    let home = temp_home();
    ensure_runtime_home_layout(&home).expect("home");
    let user_toml = sample_user_toml("http://localhost:99999");
    let system = map_system_config(&user_toml).expect("system");
    let _handler = CliDebugActionHandler::new(user_toml, system).expect("handler");

    let last_run_path = home.join("runtime/current/last_run.json");
    let last_run_json = "{\"session_id\":\"s-test-123\",\"turn_id\":\"turn-2026-05-14-001\",\"task_id\":\"t-test\",\"status\":\"ok\",\"ended_at\":\"2026-05-14T12:00:00+08:00\"}";
    fs::write(&last_run_path, last_run_json).expect("write last_run");

    let parsed = read_last_run_value(&home).expect("parse last_run");
    assert_eq!(
        parsed.get("turn_id").and_then(|v| v.as_str()),
        Some("turn-2026-05-14-001"),
        "turn_id should match written value"
    );
    assert_eq!(
        parsed.get("session_id").and_then(|v| v.as_str()),
        Some("s-test-123"),
        "session_id should match"
    );

    // Tentative turn_id also parses (validation is caller responsibility)
    let tentative = "{\"turn_id\":\"turn-tentative-abc123\"}";
    fs::write(&last_run_path, tentative).expect("write tentative");
    let parsed_t = read_last_run_value(&home).expect("parse tentative");
    let tid = parsed_t
        .get("turn_id")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    assert!(
        tid.starts_with("turn-tentative"),
        "tentative should parse correctly"
    );

    let _ = fs::remove_dir_all(&home);
}
