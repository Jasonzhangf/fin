use super::*;
use crate::channel_peer_conversations::load_conversation_by_target;
use crate::runtime_home::{ensure_runtime_home_layout, read_last_run_value, read_session_messages};
use crate::session_binding::find_session_dir;
use crate::{channel_peer::complete_builtin_qqbot_pairing, config::map_system_config};
use fin_debug_server::{ChatSendRequest, DebugActionHandler};
use serde_json::Value;
use std::{
    fs,
    io::{Read, Write},
    net::TcpListener,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{Arc, Mutex},
    thread::{self, JoinHandle},
    time::{SystemTime, UNIX_EPOCH},
};

fn temp_runtime_home() -> PathBuf {
    std::env::temp_dir().join(format!(
        "fin-qqbot-bridge-e2e-tests-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos()
    ))
}

fn sample_user_toml(base_url: &str) -> String {
    format!(
        r#"
default_provider = "local-anthropic"

[providers.local-anthropic]
protocol = "anthropic-wire"
base_url = "{base_url}"
model = "qwen3.6-plus"
api_key = "test-key"
user_agent = "opencode/1.2.27"
"#
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

fn new_distinct_session(
    handler: &CliDebugActionHandler,
    home: &Path,
    previous_session_id: &str,
) -> Result<(String, String), Box<dyn std::error::Error>> {
    thread::sleep(std::time::Duration::from_millis(1100));
    for _ in 0..3 {
        let created = new_session(handler, home)?;
        if created.0 != previous_session_id {
            return Ok(created);
        }
        thread::sleep(std::time::Duration::from_millis(1100));
    }
    Err("failed to create distinct session id".into())
}

fn spawn_bridge_sink(
    sink_path: &Path,
) -> (
    std::process::Child,
    Arc<Mutex<Option<std::process::ChildStdin>>>,
) {
    let mut child = Command::new("sh")
        .arg("-c")
        .arg("cat > \"$FIN_QQBOT_SINK\"")
        .env("FIN_QQBOT_SINK", sink_path)
        .stdin(Stdio::piped())
        .spawn()
        .expect("spawn sink");
    let stdin = Arc::new(Mutex::new(child.stdin.take()));
    (child, stdin)
}

fn finish_bridge_sink(
    stdin: &Arc<Mutex<Option<std::process::ChildStdin>>>,
    mut child: std::process::Child,
    sink_path: &Path,
) -> Vec<Value> {
    *stdin.lock().expect("sink lock") = None;
    child.wait().expect("sink wait");
    fs::read_to_string(sink_path)
        .expect("sink content")
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| serde_json::from_str(line).expect("bridge request json"))
        .collect()
}

fn direct_message(target_user: &str, message_id: &str, content: &str) -> QqbotBridgeInboundMessage {
    QqbotBridgeInboundMessage {
        message_type: "c2c".into(),
        sender_id: target_user.into(),
        sender_name: Some("Jason".into()),
        content: content.into(),
        message_id: message_id.into(),
        timestamp: "2026-04-20T21:00:00+08:00".into(),
        group_openid: None,
        channel_id: None,
        guild_id: None,
        attachments: None,
    }
}

fn read_http_body(stream: &mut std::net::TcpStream) -> String {
    stream
        .set_read_timeout(Some(std::time::Duration::from_secs(2)))
        .expect("set timeout");
    let mut buffer = Vec::new();
    let mut chunk = [0_u8; 4096];
    let mut header_end = None;
    let mut content_length = 0_usize;
    loop {
        let read = stream.read(&mut chunk).expect("read request");
        if read == 0 {
            break;
        }
        buffer.extend_from_slice(&chunk[..read]);
        if header_end.is_none() {
            header_end = buffer.windows(4).position(|window| window == b"\r\n\r\n");
            if let Some(end) = header_end {
                let header_text = String::from_utf8_lossy(&buffer[..end + 4]);
                for line in header_text.lines() {
                    let lower = line.to_ascii_lowercase();
                    if let Some((_, value)) = lower.split_once("content-length:") {
                        content_length = value.trim().parse::<usize>().expect("content length");
                    }
                }
            }
        }
        if let Some(end) = header_end {
            let body_start = end + 4;
            if buffer.len() >= body_start + content_length {
                return String::from_utf8(buffer[body_start..body_start + content_length].to_vec())
                    .expect("utf8 body");
            }
        }
    }
    String::new()
}

fn spawn_anthropic_server(
    responses: Vec<&str>,
) -> (String, Arc<Mutex<Vec<String>>>, JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("listener");
    let address = listener.local_addr().expect("local addr");
    let requests = Arc::new(Mutex::new(Vec::new()));
    let requests_for_thread = Arc::clone(&requests);
    let response_bodies = responses
        .into_iter()
        .map(str::to_string)
        .collect::<Vec<_>>();
    let handle = thread::spawn(move || {
        for body in response_bodies {
            let (mut stream, _) = listener.accept().expect("accept");
            let request_body = read_http_body(&mut stream);
            requests_for_thread
                .lock()
                .expect("requests lock")
                .push(request_body);
            let response = format!(
                "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}",
                body.len(),
                body
            );
            stream
                .write_all(response.as_bytes())
                .expect("write response");
        }
    });
    (format!("http://{address}"), requests, handle)
}

#[test]
fn qqbot_inbound_message_runs_end_to_end_and_emits_reply_from_session_truth() {
    let response_body = r#"{"id":"msg-qqbot-1","content":[{"type":"text","text":"<fin_user_response>闭环回复：收到 Jason。</fin_user_response>\n<fin_control_feedback>{\"origin\":\"model_output_contract_v1\",\"is_continuation\":true,\"is_simple_query\":false,\"candidate_task_id\":\"task-qqbot\",\"candidate_topic_thread_id\":\"topic-qqbot\",\"continuity_confidence\":92,\"topic_shift_confidence\":8,\"simple_query_confidence\":4,\"previous_topic_summary\":\"qqbot e2e\",\"current_topic_summary\":\"qqbot e2e\",\"note_candidate\":\"qqbot e2e done\",\"digest_candidate\":\"qqbot e2e done\",\"reason\":\"reply ready\"}</fin_control_feedback>\n<fin_tool_calls>[{\"tool_name\":\"reasoning.stop\",\"arguments\":{\"summary\":\"qqbot e2e done\"}}]</fin_tool_calls>"}],"stop_reason":"end_turn"}"#;
    let (base_url, requests, server) = spawn_anthropic_server(vec![response_body]);
    let home = temp_runtime_home();
    ensure_runtime_home_layout(&home).expect("runtime home");
    let user_toml = sample_user_toml(&base_url);
    let system = map_system_config(&user_toml).expect("system config");
    let handler = CliDebugActionHandler::new(user_toml, system).expect("handler");
    let (session_id, task_id) = new_session(&handler, &home).expect("new session");
    complete_builtin_qqbot_pairing(&home, &session_id, None).expect("pairing");

    let sink_path = home.join("runtime/peers/qqbot/outbound.jsonl");
    let (child, stdin) = spawn_bridge_sink(&sink_path);
    process_inbound_message(
        &home,
        &handler,
        &stdin,
        direct_message("user-e2e", "msg-inbound-1", "请做一次 qqbot 真闭环测试"),
    )
    .expect("process inbound");
    let outbound = finish_bridge_sink(&stdin, child, &sink_path);
    server.join().expect("server join");

    assert_eq!(outbound.len(), 2, "expected ack + final reply");
    assert_eq!(
        outbound[0]["payload"]["text"].as_str(),
        Some("已收到，正在处理。")
    );
    assert_eq!(
        outbound[0]["payload"]["to"].as_str(),
        Some("qqbot:c2c:user-e2e")
    );
    assert_eq!(
        outbound[1]["payload"]["text"].as_str(),
        Some("闭环回复：收到 Jason。")
    );
    assert_eq!(
        outbound[1]["payload"]["replyToId"].as_str(),
        Some("msg-inbound-1")
    );

    let conversation = load_conversation_by_target(&home, "qqbot:c2c:user-e2e")
        .expect("conversation")
        .expect("present");
    assert_eq!(
        conversation.session_id.as_deref(),
        Some(session_id.as_str())
    );

    let last_run = read_last_run_value(&home).expect("last run");
    assert_eq!(last_run["session_id"].as_str(), Some(session_id.as_str()));
    assert_eq!(last_run["task_id"].as_str(), Some(task_id.as_str()));

    let (_, _, session_dir) = find_session_dir(&home, &session_id).expect("session dir");
    let messages = read_session_messages(&session_dir.join("conversation/messages.json"))
        .expect("session messages");
    assert!(
        messages
            .iter()
            .any(|message| message.role == "user" && message.content.contains("qqbot 真闭环测试"))
    );
    let assistant_message = messages.iter().find(|message| {
        message.role == "assistant" && message.content.contains("闭环回复：收到 Jason。")
    });
    assert!(
        assistant_message.is_some(),
        "assistant answer should persist into session truth"
    );

    let provider_requests =
        fs::read_to_string(session_dir.join("provider/recent_provider_requests.json"))
            .expect("provider requests");
    assert!(provider_requests.contains("channel.qqbot"));
    assert!(provider_requests.contains("请做一次 qqbot 真闭环测试"));

    let peer_events =
        fs::read_to_string(home.join("runtime/peers/qqbot/events.jsonl")).expect("peer events");
    assert!(peer_events.contains("channel.peer.message_ingested"));
    assert!(peer_events.contains("channel.peer.session_message_send_requested"));
    assert_eq!(
        conversation.last_delivered_message_id.as_deref(),
        assistant_message.map(|message| message.message_id.as_str())
    );

    let captured_requests = requests.lock().expect("requests").clone();
    assert_eq!(captured_requests.len(), 1);
    assert!(captured_requests[0].contains("请做一次 qqbot 真闭环测试"));
}

#[test]
fn qqbot_inbound_restores_existing_target_session_and_continues_same_session() {
    let first_response = r#"{"id":"msg-qqbot-2","content":[{"type":"text","text":"<fin_user_response>第一次回复。</fin_user_response>\n<fin_control_feedback>{\"origin\":\"model_output_contract_v1\",\"is_continuation\":true,\"is_simple_query\":false,\"task_completed\":true,\"is_simple_chat\":false,\"blocked\":false,\"needs_user_involve\":false,\"candidate_task_id\":\"task-restore\",\"candidate_topic_thread_id\":\"topic-restore\",\"continuity_confidence\":90,\"topic_shift_confidence\":10,\"simple_query_confidence\":5,\"previous_topic_summary\":\"restore\",\"current_topic_summary\":\"restore\",\"completion_evidence\":[\"first inbound reply generated\",\"target binding persisted\"],\"final_conclusions\":[\"first qqbot closure completed\",\"target session remains active\"],\"blocked_reason\":null,\"what_needs_to_be_done_by_user\":null,\"note_candidate\":\"first reply\",\"digest_candidate\":\"first reply\",\"reason\":\"first turn\"}</fin_control_feedback>\n<fin_tool_calls>[{\"tool_name\":\"reasoning.stop\",\"arguments\":{\"summary\":\"first turn done\"}}]</fin_tool_calls>"}],"stop_reason":"end_turn"}"#;
    let second_response = r#"{"id":"msg-qqbot-3","content":[{"type":"text","text":"<fin_user_response>第二次回复，沿用旧会话。</fin_user_response>\n<fin_control_feedback>{\"origin\":\"model_output_contract_v1\",\"is_continuation\":true,\"is_simple_query\":false,\"task_completed\":true,\"is_simple_chat\":false,\"blocked\":false,\"needs_user_involve\":false,\"candidate_task_id\":\"task-restore\",\"candidate_topic_thread_id\":\"topic-restore\",\"continuity_confidence\":94,\"topic_shift_confidence\":6,\"simple_query_confidence\":4,\"previous_topic_summary\":\"restore\",\"current_topic_summary\":\"restore\",\"completion_evidence\":[\"existing target session restored\",\"second inbound reply persisted\"],\"final_conclusions\":[\"restore closure completed\",\"existing target binding wins\"],\"blocked_reason\":null,\"what_needs_to_be_done_by_user\":null,\"note_candidate\":\"restored existing target session\",\"digest_candidate\":\"restored existing target session\",\"reason\":\"existing target binding wins\"}</fin_control_feedback>\n<fin_tool_calls>[{\"tool_name\":\"reasoning.stop\",\"arguments\":{\"summary\":\"second turn done\"}}]</fin_tool_calls>"}],"stop_reason":"end_turn"}"#;
    let (base_url, requests, server) =
        spawn_anthropic_server(vec![first_response, second_response]);
    let home = temp_runtime_home();
    ensure_runtime_home_layout(&home).expect("runtime home");
    let user_toml = sample_user_toml(&base_url);
    let system = map_system_config(&user_toml).expect("system config");
    let handler = CliDebugActionHandler::new(user_toml, system).expect("handler");
    let (session_a, task_a) = new_session(&handler, &home).expect("session a");
    complete_builtin_qqbot_pairing(&home, &session_a, None).expect("pair session a");

    let sink_path = home.join("runtime/peers/qqbot/outbound-restore.jsonl");
    let (child, stdin) = spawn_bridge_sink(&sink_path);
    process_inbound_message(
        &home,
        &handler,
        &stdin,
        direct_message("user-restore", "msg-restore-1", "第一条消息，创建目标绑定"),
    )
    .expect("first inbound");

    let (session_b, _) = new_distinct_session(&handler, &home, &session_a).expect("session b");
    complete_builtin_qqbot_pairing(&home, &session_b, None).expect("pair session b");

    process_inbound_message(
        &home,
        &handler,
        &stdin,
        direct_message("user-restore", "msg-restore-2", "第二条消息，应恢复旧会话"),
    )
    .expect("second inbound");
    let outbound = finish_bridge_sink(&stdin, child, &sink_path);
    server.join().expect("server join");

    assert_eq!(outbound.len(), 4, "two ack + two replies");
    assert!(
        outbound[3]["payload"]["text"]
            .as_str()
            .expect("restored reply")
            .contains("第二次回复，沿用旧会话。")
    );

    let conversation = load_conversation_by_target(&home, "qqbot:c2c:user-restore")
        .expect("conversation")
        .expect("present");
    assert_eq!(conversation.session_id.as_deref(), Some(session_a.as_str()));
    assert_eq!(
        conversation.last_inbound_message_id.as_deref(),
        Some("msg-restore-2")
    );

    let last_run = read_last_run_value(&home).expect("last run");
    assert_eq!(last_run["session_id"].as_str(), Some(session_a.as_str()));
    assert_eq!(last_run["task_id"].as_str(), Some(task_a.as_str()));

    let (_, _, session_a_dir) = find_session_dir(&home, &session_a).expect("session a dir");
    let session_a_messages =
        read_session_messages(&session_a_dir.join("conversation/messages.json"))
            .expect("session a messages");
    assert!(
        session_a_messages
            .iter()
            .filter(|message| message.role == "user")
            .any(|message| message.content.contains("第一条消息"))
    );
    assert!(
        session_a_messages
            .iter()
            .filter(|message| message.role == "user")
            .any(|message| message.content.contains("第二条消息"))
    );

    let (_, _, session_b_dir) = find_session_dir(&home, &session_b).expect("session b dir");
    let session_b_messages =
        read_session_messages(&session_b_dir.join("conversation/messages.json"))
            .expect("session b messages");
    assert!(
        session_b_messages
            .iter()
            .all(|message| !message.content.contains("第二条消息"))
    );

    let peer_events =
        fs::read_to_string(home.join("runtime/peers/qqbot/events.jsonl")).expect("peer events");
    assert!(peer_events.contains("channel.peer.session_restored"));
    assert!(peer_events.contains("msg-restore-2"));

    let captured_requests = requests.lock().expect("requests").clone();
    assert_eq!(captured_requests.len(), 2);
    assert!(captured_requests[1].contains("第一条消息，创建目标绑定"));
    assert!(captured_requests[1].contains("第二条消息，应恢复旧会话"));
}
