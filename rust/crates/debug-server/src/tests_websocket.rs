use crate::{
    ChatSendRequest, ChatSendResponse, DebugActionHandler, DebugBinding, NoopDebugActionHandler,
    routes,
};
use serde_json::json;
use std::{
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, SystemTime, UNIX_EPOCH},
};

#[test]
fn websocket_route_upgrades_and_answers_mobile_handshake() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind test listener");
    let addr = listener.local_addr().expect("local addr");
    let runtime_home = std::env::temp_dir().join(format!(
        "fin-debug-ws-route-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time should work")
            .as_nanos()
    ));
    let server = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept websocket client");
        routes::handle_connection(&mut stream, &runtime_home, &NoopDebugActionHandler)
            .expect("websocket route should handle client");
    });

    let mut client = TcpStream::connect(addr).expect("connect websocket route");
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
    let header = read_http_header(&mut client);
    assert!(header.starts_with("HTTP/1.1 101 Switching Protocols"));
    assert!(header.contains("Sec-WebSocket-Accept: s3pPLMBiTxaQ9kYGzzhZRbK+xOo="));

    write_masked_text_frame(
        &mut client,
        br#"{"type":"mobile.handshake","token":"","project":"fin","scopes":[]}"#,
    );
    let payload = read_unmasked_text_frame(&mut client);
    assert!(payload.contains("\"type\":\"handshake.ok\""));
    client.write_all(&[0x88, 0x00]).expect("close frame");
    server.join().expect("server thread");
}

#[test]
fn websocket_route_keeps_idle_mobile_channel_open_after_http_parser_timeout() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind test listener");
    let addr = listener.local_addr().expect("local addr");
    let runtime_home = temp_runtime_home("fin-debug-ws-idle");
    let server = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept websocket client");
        routes::handle_connection(&mut stream, &runtime_home, &NoopDebugActionHandler)
            .expect("websocket route should keep idle client open");
    });

    let mut client = TcpStream::connect(addr).expect("connect websocket route");
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
    let header = read_http_header(&mut client);
    assert!(header.starts_with("HTTP/1.1 101 Switching Protocols"));

    write_masked_text_frame(
        &mut client,
        br#"{"type":"mobile.handshake","token":"","project":"fin","scopes":[]}"#,
    );
    let payload = read_unmasked_text_frame(&mut client);
    assert!(payload.contains("\"type\":\"handshake.ok\""));

    std::thread::sleep(Duration::from_millis(800));
    write_masked_text_frame(&mut client, br#"{"type":"mobile.subscribe"}"#);

    let session_list = read_unmasked_text_frame(&mut client);
    let runtime_health = read_unmasked_text_frame(&mut client);
    let config_snapshot = read_unmasked_text_frame(&mut client);
    let provider_health = read_unmasked_text_frame(&mut client);
    assert!(session_list.contains("\"type\":\"session.list\""));
    assert!(runtime_health.contains("\"type\":\"runtime.health\""));
    assert!(config_snapshot.contains("\"type\":\"config.snapshot\""));
    assert!(provider_health.contains("\"type\":\"provider.health\""));

    client.write_all(&[0x88, 0x00]).expect("close frame");
    server.join().expect("server thread");
}

#[test]
fn websocket_subscribe_emits_authoritative_provider_config_schema() {
    struct Handler;
    impl DebugActionHandler for Handler {
        fn read_binding(&self, runtime_home: &Path) -> Result<DebugBinding, String> {
            Ok(binding(runtime_home, Some("session-live")))
        }

        fn read_config_snapshot(&self, _: &Path) -> Result<serde_json::Value, String> {
            Ok(json!({
                "type": "config.snapshot",
                "status": "ok",
                "default_profile": "minimax",
                "profiles": [{
                    "profile_name": "minimax",
                    "provider": "minimax",
                    "protocol": "anthropic-wire",
                    "model": "MiniMax-M3",
                    "active": true
                }],
                "active_thinking_effort": null
            }))
        }

        fn send_chat_message(
            &self,
            _: &Path,
            _: ChatSendRequest,
        ) -> Result<ChatSendResponse, String> {
            unreachable!("subscribe must not send chat")
        }
    }

    let frames = ws_roundtrip_frames(&Handler, br#"{"type":"mobile.subscribe"}"#, 5);
    assert!(frames.iter().any(|frame| {
        frame.contains("\"type\":\"config.snapshot\"")
            && frame.contains("\"default_profile\":\"minimax\"")
            && frame.contains("\"provider\":\"minimax\"")
            && frame.contains("\"model\":\"MiniMax-M3\"")
    }));
    assert!(frames.iter().any(|frame| {
        frame.contains("\"type\":\"provider.health\"")
            && frame.contains("\"status\":\"ok\"")
            && frame.contains("\"provider\":\"minimax\"")
            && frame.contains("\"model\":\"MiniMax-M3\"")
    }));
}

#[test]
fn websocket_user_input_sends_progress_before_chat_handler_finishes() {
    struct BlockingHandler {
        send_started: Arc<AtomicBool>,
    }
    impl DebugActionHandler for BlockingHandler {
        fn read_binding(&self, runtime_home: &Path) -> Result<DebugBinding, String> {
            Ok(binding(runtime_home, Some("session-live")))
        }

        fn send_chat_message(
            &self,
            runtime_home: &Path,
            request: ChatSendRequest,
        ) -> Result<ChatSendResponse, String> {
            self.send_started.store(true, Ordering::SeqCst);
            std::thread::sleep(Duration::from_millis(600));
            Ok(ChatSendResponse {
                binding: binding(runtime_home, Some("session-live")),
                answer: format!("echo {}", request.message),
                digest_id: "digest-live".into(),
                events_count: 0,
                response_kind: "assistant_message".into(),
                freshness: None,
                control_feedback: None,
                progress: None,
                note: None,
                routing_action: None,
            })
        }
    }
    let send_started = Arc::new(AtomicBool::new(false));
    let handler = BlockingHandler {
        send_started: Arc::clone(&send_started),
    };
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind test listener");
    let addr = listener.local_addr().expect("local addr");
    let runtime_home = temp_runtime_home("fin-debug-ws-streaming");
    let server = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept websocket client");
        routes::handle_connection(&mut stream, &runtime_home, &handler)
            .expect("websocket route should handle streamed input");
    });

    let mut client = TcpStream::connect(addr).expect("connect websocket route");
    write_ws_upgrade_request(&mut client);
    let header = read_http_header(&mut client);
    assert!(header.starts_with("HTTP/1.1 101 Switching Protocols"));

    write_masked_text_frame(
        &mut client,
        br#"{"type":"session.user_input","payload":"hi","client_message_id":"m-stream"}"#,
    );
    let started_at = std::time::Instant::now();
    let accepted = read_unmasked_text_frame(&mut client);
    let started = read_unmasked_text_frame(&mut client);
    let progress = read_unmasked_text_frame(&mut client);
    assert!(
        started_at.elapsed() < Duration::from_millis(500),
        "initial WS frames must arrive before the blocking chat handler completes"
    );
    assert!(accepted.contains("\"type\":\"input.accepted\""));
    assert!(started.contains("\"type\":\"turn.started\""));
    assert!(progress.contains("\"type\":\"turn.progress\""));
    let provider_item = read_unmasked_text_frame(&mut client);
    assert!(provider_item.contains("\"type\":\"turn.item.started\""));
    assert!(provider_item.contains("\"label\":\"provider.call\""));
    assert!(provider_item.contains("\"status\":\"running\""));

    let mut terminal = Vec::new();
    for _ in 0..8 {
        let frame = read_unmasked_text_frame(&mut client);
        terminal.push(frame.clone());
        if frame.contains("\"type\":\"turn.rendered\"") {
            break;
        }
    }
    assert!(terminal.iter().any(|frame| {
        frame.contains("\"type\":\"turn.completed\"") && frame.contains("\"status\":\"completed\"")
    }));
    assert!(terminal.iter().any(|frame| {
        frame.contains("\"type\":\"turn.rendered\"") && frame.contains("echo hi")
    }));
    assert!(send_started.load(Ordering::SeqCst));

    client.write_all(&[0x88, 0x00]).expect("close frame");
    server.join().expect("server thread");
}

#[test]
fn websocket_user_input_accepts_android_content_field() {
    struct EchoHandler;
    impl DebugActionHandler for EchoHandler {
        fn read_binding(&self, runtime_home: &Path) -> Result<DebugBinding, String> {
            Ok(binding(runtime_home, Some("session-live")))
        }

        fn send_chat_message(
            &self,
            runtime_home: &Path,
            request: ChatSendRequest,
        ) -> Result<ChatSendResponse, String> {
            Ok(ChatSendResponse {
                binding: binding(runtime_home, Some("session-live")),
                answer: format!("echo {}", request.message),
                digest_id: "digest-live".into(),
                events_count: 0,
                response_kind: "assistant_message".into(),
                freshness: None,
                control_feedback: None,
                progress: None,
                note: None,
                routing_action: None,
            })
        }
    }

    let frames = ws_roundtrip_frames(
        &EchoHandler,
        br#"{"type":"session.user_input","content":"android hi","client_message_id":"m-content"}"#,
        6,
    );
    assert!(
        frames
            .iter()
            .any(|frame| frame.contains("\"type\":\"input.accepted\""))
    );
    assert!(
        frames
            .iter()
            .any(|frame| frame.contains("\"type\":\"turn.started\""))
    );
    assert!(
        frames
            .iter()
            .any(|frame| frame.contains("\"type\":\"turn.progress\""))
    );
    assert!(frames.iter().any(|frame| {
        frame.contains("\"type\":\"turn.completed\"") && frame.contains("\"status\":\"completed\"")
    }));
    assert!(frames.iter().any(|frame| {
        frame.contains("\"type\":\"turn.rendered\"") && frame.contains("echo android hi")
    }));
}

#[test]
fn serve_debug_writes_port_scoped_pid_file_after_bind() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind test listener");
    let port = listener.local_addr().expect("listener addr").port();
    let runtime_home = temp_runtime_home("fin-debug-ws-pid");

    crate::persist_web_debug_pid(&runtime_home, &listener).expect("pid file write");

    let pid =
        std::fs::read_to_string(runtime_home.join(format!("runtime/pids/web-debug-{port}.pid")))
            .expect("pid file should exist");
    assert_eq!(pid, std::process::id().to_string());
}

fn ws_roundtrip_frames(
    handler: &(impl DebugActionHandler + Sync),
    payload: &[u8],
    frame_count: usize,
) -> Vec<String> {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind test listener");
    let addr = listener.local_addr().expect("local addr");
    let runtime_home = temp_runtime_home("fin-debug-ws-roundtrip");
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

fn binding(runtime_home: &Path, session_id: Option<&str>) -> DebugBinding {
    DebugBinding {
        project_id: "fin".into(),
        project_label: "fin".into(),
        runtime_home: runtime_home.display().to_string(),
        session_id: session_id.map(str::to_string),
        task_id: session_id.map(|_| "task-live".to_string()),
        session_messages_path: None,
        recent_contexts_path: None,
        recent_digests_path: None,
    }
}

fn temp_runtime_home(prefix: &str) -> std::path::PathBuf {
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
