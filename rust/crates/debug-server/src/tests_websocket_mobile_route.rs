use crate::{ChatSendRequest, ChatSendResponse, DebugActionHandler, DebugBinding, routes};
use serde_json::json;
use std::{
    fs,
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

#[test]
fn websocket_user_input_streams_tool_item_before_final_render() {
    let frames = ws_roundtrip_frames(
        &ToolHandler,
        br#"{"type":"session.user_input","payload":"tool render hi","client_message_id":"m-tools"}"#,
        7,
    );
    let item_index = frames
        .iter()
        .position(|frame| frame.contains("\"type\":\"turn.item.completed\""))
        .expect("turn item frame");
    let provider_started_index = frames
        .iter()
        .position(|frame| {
            frame.contains("\"type\":\"turn.item.started\"")
                && frame.contains("\"label\":\"provider.call\"")
        })
        .expect("provider started item frame");
    let rendered_index = frames
        .iter()
        .position(|frame| frame.contains("\"type\":\"turn.rendered\""))
        .expect("rendered frame");
    assert!(provider_started_index < item_index);
    assert!(item_index < rendered_index);
    assert!(frames[item_index].contains("\"item_id\":\"tool-ws-item\""));
    assert!(frames[rendered_index].contains("\"tool_execution_records\""));
    assert!(frames[rendered_index].contains("\"tool_call_id\":\"tool-ws-item\""));
}

struct ToolHandler;

impl DebugActionHandler for ToolHandler {
    fn read_binding(&self, runtime_home: &Path) -> Result<DebugBinding, String> {
        Ok(binding(runtime_home))
    }

    fn send_chat_message(
        &self,
        runtime_home: &Path,
        request: ChatSendRequest,
    ) -> Result<ChatSendResponse, String> {
        write_tool_truth(runtime_home);
        Ok(ChatSendResponse {
            binding: binding(runtime_home),
            answer: format!("echo {}", request.message),
            digest_id: "digest-live".into(),
            events_count: 1,
            response_kind: "assistant_message".into(),
            freshness: None,
            control_feedback: None,
            progress: None,
            note: None,
            routing_action: None,
        })
    }
}

fn write_tool_truth(runtime_home: &Path) {
    let current_dir = runtime_home.join("runtime/current");
    let tools_dir = runtime_home.join("sessions/2026/06/session-live/tools");
    fs::create_dir_all(&current_dir).expect("current dir");
    fs::create_dir_all(&tools_dir).expect("tools dir");
    fs::write(
        current_dir.join("last_run.json"),
        json!({
            "operation_id": "op-ws-items",
            "session_recent_tool_records_path": "sessions/2026/06/session-live/tools/recent_tool_records.json"
        })
        .to_string(),
    )
    .expect("last run");
    fs::write(
        tools_dir.join("recent_tool_records.json"),
        json!([{
            "tool_call_id":"tool-ws-item",
            "operation_id":"op-ws-items",
            "trace_id":"trace-ws-items",
            "session_id":"session-live",
            "task_id":"task-live",
            "topic_thread_id":null,
            "dispatch_id":null,
            "worker_id":"worker-system",
            "tool_name":"update_plan",
            "tool_kind":"agent_tool",
            "title":"Update Plan",
            "purpose":"render mobile tool item",
            "target_kind":"plan",
            "target_ref":"runtime/current/current_plan.json",
            "input_summary":"record progress",
            "output_summary":"plan updated",
            "status":"completed",
            "started_at":"2026-06-09T12:00:00+08:00",
            "ended_at":"2026-06-09T12:00:01+08:00",
            "duration_ms":1,
            "side_effects":["plan_write"],
            "artifact_refs":["runtime/current/current_plan.json"],
            "error_summary":null
        }])
        .to_string(),
    )
    .expect("tool records");
}

fn ws_roundtrip_frames(
    handler: &(impl DebugActionHandler + Sync),
    payload: &[u8],
    frame_count: usize,
) -> Vec<String> {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind test listener");
    let addr = listener.local_addr().expect("local addr");
    let runtime_home = temp_runtime_home("fin-debug-ws-route-items");
    let server = std::thread::scope(|scope| {
        scope.spawn(|| {
            let (mut stream, _) = listener.accept().expect("accept websocket client");
            routes::handle_connection(&mut stream, &runtime_home, handler)
                .expect("websocket route should handle client");
        });
        let mut client = TcpStream::connect(addr).expect("connect websocket route");
        write_ws_upgrade_request(&mut client);
        let header = read_http_header(&mut client);
        assert!(header.starts_with("HTTP/1.1 101 Switching Protocols"));
        write_masked_text_frame(&mut client, payload);
        let frames = (0..frame_count)
            .map(|_| read_unmasked_text_frame(&mut client))
            .collect::<Vec<_>>();
        client.write_all(&[0x88, 0x00]).expect("close frame");
        frames
    });
    server
}

fn binding(runtime_home: &Path) -> DebugBinding {
    DebugBinding {
        project_id: "fin".into(),
        project_label: "fin".into(),
        runtime_home: runtime_home.display().to_string(),
        session_id: Some("session-live".into()),
        task_id: Some("task-live".into()),
        session_messages_path: None,
        recent_contexts_path: None,
        recent_digests_path: None,
    }
}

fn temp_runtime_home(prefix: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "{}-{}",
        prefix,
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time should work")
            .as_nanos()
    ))
}

fn read_http_header(stream: &mut TcpStream) -> String {
    let mut response = Vec::new();
    let mut byte = [0_u8; 1];
    while !response.ends_with(b"\r\n\r\n") {
        stream.read_exact(&mut byte).expect("read handshake byte");
        response.push(byte[0]);
    }
    String::from_utf8(response).expect("handshake utf8")
}

fn write_ws_upgrade_request(client: &mut TcpStream) {
    client
        .write_all(
            b"GET /ws HTTP/1.1\r\n\
              Host: 127.0.0.1\r\n\
              Connection: Upgrade\r\n\
              Upgrade: websocket\r\n\
              Sec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\n\
              Sec-WebSocket-Version: 13\r\n\r\n",
        )
        .expect("write handshake request");
}

fn write_masked_text_frame(stream: &mut TcpStream, payload: &[u8]) {
    assert!(payload.len() < 126);
    let mask = [1_u8, 2, 3, 4];
    let mut frame = vec![0x81, 0x80 | payload.len() as u8];
    frame.extend_from_slice(&mask);
    for (idx, byte) in payload.iter().enumerate() {
        frame.push(byte ^ mask[idx % 4]);
    }
    stream.write_all(&frame).expect("write masked frame");
}

fn read_unmasked_text_frame(stream: &mut TcpStream) -> String {
    let mut header = [0_u8; 2];
    stream.read_exact(&mut header).expect("read frame header");
    assert_eq!(header[0] & 0x0f, 0x1);
    assert_eq!(header[1] & 0x80, 0);
    let marker = header[1] & 0x7f;
    let len = if marker < 126 {
        usize::from(marker)
    } else if marker == 126 {
        let mut ext = [0_u8; 2];
        stream.read_exact(&mut ext).expect("read len16");
        usize::from(u16::from_be_bytes(ext))
    } else {
        let mut ext = [0_u8; 8];
        stream.read_exact(&mut ext).expect("read len64");
        u64::from_be_bytes(ext)
            .try_into()
            .expect("test frame length should fit usize")
    };
    let mut payload = vec![0_u8; len];
    stream.read_exact(&mut payload).expect("read frame payload");
    String::from_utf8(payload).expect("frame utf8")
}
