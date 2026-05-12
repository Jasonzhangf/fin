use super::*;
use std::{
    fs,
    path::Path,
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

static TEMP_HOME_SEQ: AtomicU64 = AtomicU64::new(1);

fn temp_runtime_home() -> std::path::PathBuf {
    let seq = TEMP_HOME_SEQ.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!(
        "fin-qqbot-command-{}-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos(),
        seq,
    ))
}

fn binding(home: &Path) -> DebugBinding {
    let session_rel = "sessions/2026/05/session-qq/conversation/messages.json".to_string();
    let full = home.join(&session_rel);
    fs::create_dir_all(full.parent().expect("parent")).expect("mkdir");
    fs::write(&full, b"[]").expect("messages");
    DebugBinding {
        project_id: "fin".into(),
        project_label: "fin".into(),
        runtime_home: home.display().to_string(),
        session_id: Some("session-qq".into()),
        task_id: Some("task-qq".into()),
        session_messages_path: Some(session_rel),
        recent_contexts_path: None,
        recent_digests_path: None,
    }
}

#[test]
fn qqbot_pair_command_updates_state_and_writes_session_notice() {
    let home = temp_runtime_home();
    fs::create_dir_all(&home).expect("home");
    let result = try_handle_channel_peer_command(
        &home,
        None,
        &ChatSendRequest {
            message: "/qqbot pair".into(),
            input_kind: None,
            attachments: Vec::new(),
        },
        &binding(&home),
    )
    .expect("command ok")
    .expect("handled");
    assert!(result.answer.contains("qqbot paired"));
    assert!(result.answer.contains("persistent"));
    let messages =
        fs::read_to_string(home.join("sessions/2026/05/session-qq/conversation/messages.json"))
            .expect("messages");
    assert!(messages.contains("/qqbot pair"));
    assert!(messages.contains("qqbot paired with session session-qq"));
    assert!(messages.contains("persistent"));
}

#[test]
fn qqbot_expire_command_releases_binding_without_pairing_required() {
    let home = temp_runtime_home();
    fs::create_dir_all(&home).expect("home");
    let bind = binding(&home);
    let _ = try_handle_channel_peer_command(
        &home,
        None,
        &ChatSendRequest {
            message: "/qqbot pair".into(),
            input_kind: None,
            attachments: Vec::new(),
        },
        &bind,
    )
    .expect("pair ok");
    let result = try_handle_channel_peer_command(
        &home,
        None,
        &ChatSendRequest {
            message: "/qqbot expire test-reason".into(),
            input_kind: None,
            attachments: Vec::new(),
        },
        &bind,
    )
    .expect("expire ok")
    .expect("handled");
    assert!(result.answer.contains("binding=unbound"));
    assert!(result.answer.contains("pairing_required=false"));
}

#[test]
fn qqbot_debug_command_toggles_and_reports_state() {
    let home = temp_runtime_home();
    fs::create_dir_all(&home).expect("home");
    let bind = binding(&home);

    let enable = try_handle_channel_peer_command(
        &home,
        None,
        &ChatSendRequest {
            message: "/qqbot debug on".into(),
            input_kind: None,
            attachments: Vec::new(),
        },
        &bind,
    )
    .expect("debug on ok")
    .expect("handled");
    assert!(enable.answer.contains("enabled"));
    assert!(qqbot_debug_enabled(&home).expect("debug enabled"));

    let status = try_handle_channel_peer_command(
        &home,
        None,
        &ChatSendRequest {
            message: "/qqbot debug status".into(),
            input_kind: None,
            attachments: Vec::new(),
        },
        &bind,
    )
    .expect("debug status ok")
    .expect("handled");
    assert!(status.answer.contains("enabled"));

    let disable = try_handle_channel_peer_command(
        &home,
        None,
        &ChatSendRequest {
            message: "/qqbot debug off".into(),
            input_kind: None,
            attachments: Vec::new(),
        },
        &bind,
    )
    .expect("debug off ok")
    .expect("handled");
    assert!(disable.answer.contains("disabled"));
    assert!(!qqbot_debug_enabled(&home).expect("debug disabled"));
}

#[test]
fn qqbot_progress_command_updates_policy() {
    let home = temp_runtime_home();
    fs::create_dir_all(&home).expect("home");
    let bind = binding(&home);

    let compact = try_handle_channel_peer_command(
        &home,
        None,
        &ChatSendRequest {
            message: "/qqbot progress compact".into(),
            input_kind: None,
            attachments: Vec::new(),
        },
        &bind,
    )
    .expect("progress compact ok")
    .expect("handled");
    assert!(compact.answer.contains("detail=compact"));

    let tools = try_handle_channel_peer_command(
        &home,
        None,
        &ChatSendRequest {
            message: "/qqbot progress tools off".into(),
            input_kind: None,
            attachments: Vec::new(),
        },
        &bind,
    )
    .expect("progress tools ok")
    .expect("handled");
    assert!(tools.answer.contains("tool_summary=false"));

    let status = try_handle_channel_peer_command(
        &home,
        None,
        &ChatSendRequest {
            message: "/qqbot progress status".into(),
            input_kind: None,
            attachments: Vec::new(),
        },
        &bind,
    )
    .expect("progress status ok")
    .expect("handled");
    assert!(status.answer.contains("detail=compact"));
    assert!(status.answer.contains("tool_summary=false"));
}
