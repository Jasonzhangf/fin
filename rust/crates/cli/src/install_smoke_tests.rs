use super::*;
use crate::fs_utils::write_file;
use std::{
    fs,
    time::{SystemTime, UNIX_EPOCH},
};

fn temp_runtime_home() -> std::path::PathBuf {
    std::env::temp_dir().join(format!(
        "fin-install-smoke-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time should work")
            .as_nanos()
    ))
}

#[test]
fn verify_smoke_artifacts_writes_session_truth_into_summary() {
    let runtime_home = temp_runtime_home();
    fs::create_dir_all(runtime_home.join("runtime/current")).expect("runtime current dir");
    fs::create_dir_all(runtime_home.join("runtime/projections")).expect("runtime projections dir");
    fs::create_dir_all(runtime_home.join("sessions/2026/04/session-test-install/conversation"))
        .expect("conversation dir");
    fs::create_dir_all(runtime_home.join("sessions/2026/04/session-test-install/context"))
        .expect("context dir");

    for relative in [
        "runtime/current/current_context.json",
        "runtime/projections/current_snapshot.json",
        "runtime/projections/current_projection.json",
        "sessions/2026/04/session-test-install/conversation/messages.json",
        "sessions/2026/04/session-test-install/context/recent_contexts.json",
    ] {
        write_file(&runtime_home.join(relative), b"[]").expect("fixture file should write");
    }
    write_file(
        &runtime_home.join("runtime/current/last_run.json"),
        serde_json::to_vec_pretty(&json!({
            "session_id": "session-test-install",
            "task_id": "task-test-install",
            "operation_id": "op-test-install-0001",
            "session_recent_contexts_path": "sessions/2026/04/session-test-install/context/recent_contexts.json",
            "session_messages_path": "sessions/2026/04/session-test-install/conversation/messages.json"
        }))
        .expect("last run json")
        .as_slice(),
    )
    .expect("last run should write");

    verify_smoke_artifacts(
        &runtime_home,
        "0.1.0001",
        std::path::Path::new("/tmp/fake-fin"),
        std::path::Path::new("/tmp/fake-home"),
    )
    .expect("verify smoke should pass");

    let summary: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(runtime_home.join("harness/reports/0.1.0001/summary.json"))
            .expect("summary should exist"),
    )
    .expect("summary should parse");
    assert_eq!(summary["session_id"].as_str(), Some("session-test-install"));
    assert_eq!(summary["task_id"].as_str(), Some("task-test-install"));
    assert_eq!(
        summary["operation_id"].as_str(),
        Some("op-test-install-0001")
    );
    let verified = summary["verified_paths"]
        .as_array()
        .expect("verified paths should be array");
    assert!(
        verified
            .iter()
            .any(|item| item.as_str() == Some("artifacts/session_messages.json"))
    );
    assert_eq!(summary["smoke_binary"].as_str(), Some("/tmp/fake-fin"));
    assert!(
        runtime_home
            .join("harness/reports/0.1.0001/artifacts/session_messages.json")
            .exists()
    );
}

#[test]
fn finalize_smoke_report_after_promote_rewrites_binary_to_canonical_version_path() {
    let runtime_home = temp_runtime_home();
    fs::create_dir_all(runtime_home.join("harness/reports/0.1.0001")).expect("report dir");
    fs::create_dir_all(runtime_home.join("install/versions/0.1.0001/bin")).expect("version dir");
    write_file(
        &runtime_home.join("harness/reports/0.1.0001/summary.json"),
        serde_json::to_vec_pretty(&json!({
            "build_version": "0.1.0001",
            "binary": "/tmp/staged/0.1.0001/bin/fin"
        }))
        .expect("summary json")
        .as_slice(),
    )
    .expect("summary write");

    finalize_smoke_report_after_promote(&runtime_home, "0.1.0001")
        .expect("finalize summary should pass");

    let summary: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(runtime_home.join("harness/reports/0.1.0001/summary.json"))
            .expect("summary should exist"),
    )
    .expect("summary should parse");
    let expected = runtime_home
        .join("install/versions/0.1.0001/bin/fin")
        .display()
        .to_string();
    assert_eq!(summary["binary"].as_str(), Some(expected.as_str()));
    assert_eq!(summary["promoted_binary"].as_str(), Some(expected.as_str()));
    assert_eq!(
        summary["smoke_binary"].as_str(),
        Some("/tmp/staged/0.1.0001/bin/fin")
    );
}

#[test]
fn cleanup_smoke_session_artifacts_removes_session_and_scrubs_runtime_pointers() {
    let runtime_home = temp_runtime_home();
    let session_id = "session-test-install-0-1-0001";
    let session_dir = runtime_home.join("sessions/2026/04").join(session_id);
    fs::create_dir_all(session_dir.join("conversation")).expect("conversation dir");
    fs::create_dir_all(runtime_home.join("runtime/current")).expect("runtime current dir");
    fs::create_dir_all(runtime_home.join("runtime/channels/qqbot"))
        .expect("qqbot conversations dir");
    fs::create_dir_all(runtime_home.join("runtime/peers/qqbot")).expect("qqbot peer dir");

    write_file(
        &runtime_home.join("runtime/current/last_run.json"),
        serde_json::to_vec_pretty(&json!({
            "session_id": session_id,
            "task_id": "task-test-install-0-1-0001",
            "session_messages_path": format!("sessions/2026/04/{session_id}/conversation/messages.json")
        }))
        .expect("json")
        .as_slice(),
    )
    .expect("last run");
    write_file(
        &runtime_home.join("runtime/channels/qqbot/conversations.json"),
        serde_json::to_vec_pretty(&json!({
            "conversations": [
                {
                    "conversation_id": "qqconv-1",
                    "channel_id": "qqbot",
                    "target": "qqbot:c2c:user-1",
                    "session_id": session_id,
                    "status": "bound"
                }
            ]
        }))
        .expect("json")
        .as_slice(),
    )
    .expect("conversations");
    write_file(
        &runtime_home.join("runtime/peers/qqbot/state.json"),
        serde_json::to_vec_pretty(&json!({
            "binding_state": "bound",
            "pairing_required": false,
            "session_valid": true,
            "paired_at": "2026-04-22T00:00:00+08:00",
            "session_id": session_id,
            "session_expires_at": null,
            "session_ttl_minutes": null
        }))
        .expect("json")
        .as_slice(),
    )
    .expect("state");
    write_file(
        &runtime_home.join("runtime/peers/qqbot/activity_delivery_state.json"),
        serde_json::to_vec_pretty(&json!({
            "session_id": session_id,
            "target": "qqbot:c2c:user-1",
            "last_user_signature": "sig"
        }))
        .expect("json")
        .as_slice(),
    )
    .expect("activity state");

    cleanup_smoke_session_artifacts(&runtime_home, "test-install-0-1-0001")
        .expect("cleanup should succeed");

    assert!(!session_dir.exists());
    let last_run: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(runtime_home.join("runtime/current/last_run.json")).expect("last run"),
    )
    .expect("last run json");
    assert_eq!(last_run, json!({}));

    let conversations: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(runtime_home.join("runtime/channels/qqbot/conversations.json"))
            .expect("conversations"),
    )
    .expect("conversations json");
    assert_eq!(
        conversations["conversations"].as_array().map(|v| v.len()),
        Some(0)
    );

    let state: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(runtime_home.join("runtime/peers/qqbot/state.json")).expect("state"),
    )
    .expect("state json");
    assert_eq!(state["binding_state"].as_str(), Some("unbound"));
    assert_eq!(state["session_id"], serde_json::Value::Null);
    assert_eq!(state["session_valid"].as_bool(), Some(false));

    let activity: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(runtime_home.join("runtime/peers/qqbot/activity_delivery_state.json"))
            .expect("activity"),
    )
    .expect("activity json");
    assert_eq!(activity["session_id"], serde_json::Value::Null);
    assert_eq!(activity["target"], serde_json::Value::Null);
}
