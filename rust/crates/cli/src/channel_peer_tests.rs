use crate::channel_peer::{
    complete_builtin_qqbot_pairing, ensure_builtin_qqbot_binding, ensure_builtin_qqbot_peer,
    force_expire_builtin_qqbot_session, probe_builtin_qqbot_connectivity,
    record_builtin_qqbot_heartbeat,
};
use serde_json::Value;
use std::{
    fs,
    io::{Read, Write},
    net::TcpListener,
    sync::{Mutex, OnceLock},
    thread,
    time::{SystemTime, UNIX_EPOCH},
};

fn env_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

fn temp_runtime_home() -> std::path::PathBuf {
    std::env::temp_dir().join(format!(
        "fin-qqbot-peer-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos()
    ))
}

#[test]
fn ensure_builtin_qqbot_peer_writes_state_registry_and_pairing_required_event() {
    let home = temp_runtime_home();
    fs::create_dir_all(&home).expect("home");
    let state = ensure_builtin_qqbot_peer(&home).expect("ensure peer");
    assert_eq!(state.peer_kind, "channel_gateway.qqbot");
    assert_eq!(state.runtime_state, "ready_local");
    assert_eq!(state.connectivity_state, "local_only");
    assert_eq!(state.binding_state, "pairing_required");
    assert!(state.pairing_required);
    assert!(!state.session_valid);

    let registry_text =
        fs::read_to_string(home.join("runtime/peers/registry.json")).expect("registry");
    assert!(registry_text.contains("peer-channel-gateway-qqbot-local"));
    assert!(registry_text.contains("pairing_required"));

    let events_path = home.join("runtime/peers/qqbot/events.jsonl");
    let events_text = fs::read_to_string(events_path).expect("events");
    assert!(events_text.contains("channel.peer.pairing_required"));
}

#[test]
fn pairing_then_force_expire_requires_repairing() {
    let home = temp_runtime_home();
    fs::create_dir_all(&home).expect("home");
    ensure_builtin_qqbot_peer(&home).expect("ensure");

    let paired = complete_builtin_qqbot_pairing(&home, "session-qq-1", Some(1))
        .expect("pairing should complete");
    assert_eq!(paired.runtime_state, "ready_local");
    assert_eq!(paired.connectivity_state, "local_only");
    assert_eq!(paired.binding_state, "bound");
    assert_eq!(paired.lifecycle_state, "paired_active");
    assert!(paired.session_valid);
    assert!(!paired.pairing_required);
    assert_eq!(paired.session_id.as_deref(), Some("session-qq-1"));

    let expired =
        force_expire_builtin_qqbot_session(&home, "manual-test").expect("force expire should work");
    assert_eq!(expired.binding_state, "pairing_required");
    assert_eq!(expired.lifecycle_state, "idle_unpaired");
    assert!(!expired.session_valid);
    assert!(expired.pairing_required);
    assert!(expired.reconnect_count >= 1);

    let events_path = home.join("runtime/peers/qqbot/events.jsonl");
    let events_text = fs::read_to_string(events_path).expect("events");
    assert!(events_text.contains("channel.peer.pairing_completed"));
    assert!(events_text.contains("channel.peer.session_expired"));
}

#[test]
fn heartbeat_records_event_and_last_heartbeat() {
    let home = temp_runtime_home();
    fs::create_dir_all(&home).expect("home");
    ensure_builtin_qqbot_peer(&home).expect("ensure");

    let state = record_builtin_qqbot_heartbeat(&home).expect("heartbeat");
    assert!(state.last_heartbeat_at.is_some());

    let events_path = home.join("runtime/peers/qqbot/events.jsonl");
    let events_text = fs::read_to_string(events_path).expect("events");
    assert!(events_text.contains("channel.peer.heartbeat_recorded"));
}

#[test]
fn binding_mismatch_invalidates_existing_paired_session() {
    let home = temp_runtime_home();
    fs::create_dir_all(&home).expect("home");
    ensure_builtin_qqbot_peer(&home).expect("ensure");
    let _ = complete_builtin_qqbot_pairing(&home, "session-a", Some(5)).expect("pair");

    let state = ensure_builtin_qqbot_binding(&home, Some("session-b")).expect("binding check");
    assert_eq!(state.binding_state, "pairing_required");
    assert!(!state.session_valid);
    assert!(state.pairing_required);
    assert_eq!(state.lifecycle_state, "idle_unpaired");

    let lines = fs::read_to_string(home.join("runtime/peers/qqbot/events.jsonl")).expect("events");
    assert!(lines.contains("channel.peer.session_invalidated"));
    let last = lines.lines().last().expect("last line");
    let parsed: Value = serde_json::from_str(last).expect("json");
    assert_eq!(parsed["event_type"], "channel.peer.pairing_required");
}

#[test]
fn connectivity_probe_marks_connected_when_upstream_returns_token() {
    let _guard = env_lock().lock().expect("env lock");
    let original_home = std::env::var("HOME").ok();
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let addr = listener.local_addr().expect("addr");
    let server = thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept");
        let mut buffer = [0_u8; 2048];
        let _ = stream.read(&mut buffer).expect("read");
        let body = r#"{"access_token":"abc","expires_in":7200}"#;
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            body.len(),
            body
        );
        stream.write_all(response.as_bytes()).expect("write");
    });

    unsafe {
        std::env::set_var("FIN_QQBOT_TOKEN_URL", format!("http://{}", addr));
        std::env::set_var("FIN_QQBOT_APP_ID", "1903323793");
        std::env::set_var("FIN_QQBOT_CLIENT_SECRET", "secret");
    }

    let home = temp_runtime_home();
    fs::create_dir_all(&home).expect("home");
    let state = probe_builtin_qqbot_connectivity(&home).expect("probe");
    assert_eq!(state.connectivity_state, "connected");
    assert_eq!(state.credential_source.as_deref(), Some("env"));
    assert!(state.upstream_authenticated_at.is_some());
    assert!(state.upstream_expires_at.is_some());

    let events = fs::read_to_string(home.join("runtime/peers/qqbot/events.jsonl")).expect("events");
    assert!(events.contains("channel.peer.upstream_authenticated"));

    unsafe {
        std::env::remove_var("FIN_QQBOT_TOKEN_URL");
        std::env::remove_var("FIN_QQBOT_APP_ID");
        std::env::remove_var("FIN_QQBOT_CLIENT_SECRET");
        if let Some(home) = original_home {
            std::env::set_var("HOME", home);
        }
    }
    server.join().expect("server");
}

#[test]
fn connectivity_probe_marks_auth_required_when_credentials_missing() {
    let _guard = env_lock().lock().expect("env lock");
    let original_home = std::env::var("HOME").ok();
    let isolated_home = temp_runtime_home();
    unsafe {
        std::env::remove_var("FIN_QQBOT_TOKEN_URL");
        std::env::remove_var("FIN_QQBOT_APP_ID");
        std::env::remove_var("FIN_QQBOT_CLIENT_SECRET");
        std::env::set_var("HOME", &isolated_home);
    }
    let home = temp_runtime_home();
    fs::create_dir_all(&home).expect("home");
    let state = probe_builtin_qqbot_connectivity(&home).expect("probe");
    assert_eq!(state.connectivity_state, "auth_required");
    assert!(state.last_connectivity_error.as_deref().unwrap_or_default().contains("missing qqbot credentials"));
    let events = fs::read_to_string(home.join("runtime/peers/qqbot/events.jsonl")).expect("events");
    assert!(events.contains("channel.peer.connectivity_probe_failed"));
    unsafe {
        if let Some(home) = original_home {
            std::env::set_var("HOME", home);
        }
    }
}
