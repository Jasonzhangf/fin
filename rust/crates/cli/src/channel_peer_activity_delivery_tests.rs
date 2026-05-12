use super::*;
use crate::channel_peer::complete_builtin_qqbot_pairing;
use crate::channel_peer_activity_delivery::channel_peer_activity_delivery_render::render_recent_action;
use fin_contracts::{
    ActivitySourceSummary, SourceActivityCardView, ToolSemanticView, UserActivityCardView,
};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

static TEMP_SEQ: AtomicU64 = AtomicU64::new(1);

fn temp_runtime_home() -> PathBuf {
    std::env::temp_dir().join(format!(
        "fin-qqbot-activity-delivery-{}-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos(),
        TEMP_SEQ.fetch_add(1, Ordering::Relaxed),
    ))
}

fn write_json(path: &Path, value: &impl serde::Serialize) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("create parent");
    }
    fs::write(path, serde_json::to_vec_pretty(value).expect("json")).expect("write");
}

#[test]
fn render_compact_text_uses_compact_multiline_layout() {
    let snapshot = ActivityCardsSnapshot {
        session_id: Some("session-1".into()),
        task_id: Some("task-1".into()),
        generated_at: "2026-04-19T12:00:00+08:00".into(),
        user_card: Some(UserActivityCardView {
            owner_source_id: "system-agent".into(),
            header: "frontstage".into(),
            state: "running".into(),
            focus_source_id: Some("system-agent".into()),
            focus_summary: Some("working on runtime closeout".into()),
            stage: Some("running regression".into()),
            recent_items: vec!["Ran cargo test".into()],
            active_sources: vec![ActivitySourceSummary {
                source_id: "system-agent".into(),
                title: "System Agent".into(),
                state: "running".into(),
                summary: "runtime".into(),
                visibility: "detailed".into(),
                auto_promoted: true,
            }],
            waiting_detail: None,
            failure_detail: None,
            updated_at: "2026-04-19T12:00:00+08:00".into(),
        }),
        source_cards: vec![SourceActivityCardView {
            source_id: "system-agent".into(),
            source_kind: "system_agent".into(),
            title: "System Agent".into(),
            visibility: "detailed".into(),
            state: "running".into(),
            summary: "running".into(),
            focus_label: None,
            auto_promoted: true,
            current_activity: Some("running regression".into()),
            recent_actions: vec![ToolSemanticView {
                category: "command".into(),
                detail: Some("cmd=cargo test → exit_code=0".into()),
                summary: "Ran cargo test".into(),
                ..ToolSemanticView::default()
            }],
            waiting_detail: None,
            failure_detail: None,
            session_id: Some("session-1".into()),
            task_id: Some("task-1".into()),
            updated_at: "2026-04-19T12:00:00+08:00".into(),
        }],
        tool_semantics: Vec::new(),
    };
    let rendered = render_compact_text(&snapshot);
    assert!(rendered.contains("🌐 System Agent"));
    assert!(!rendered.contains("Global status"));
    assert!(rendered.contains("🧩 System Agent"));
    assert!(rendered.contains("✅ 命令:"));
}

#[test]
fn render_compact_text_deduplicates_waiting_notice_lines() {
    let snapshot = ActivityCardsSnapshot {
        session_id: Some("session-1".into()),
        task_id: Some("task-1".into()),
        generated_at: "2026-04-20T15:12:00+08:00".into(),
        user_card: Some(UserActivityCardView {
            owner_source_id: "system-agent".into(),
            header: "system frontstage · 已收到，正在处理".into(),
            state: "waiting".into(),
            focus_source_id: Some("system-agent".into()),
            focus_summary: Some("已收到，正在处理".into()),
            stage: Some("已收到，正在处理".into()),
            recent_items: vec!["已收到，正在处理".into(), "Stopped current_turn".into()],
            active_sources: vec![
                ActivitySourceSummary {
                    source_id: "system-agent".into(),
                    title: "System Agent".into(),
                    state: "waiting".into(),
                    summary: "已收到，正在处理".into(),
                    visibility: "compact".into(),
                    auto_promoted: true,
                },
                ActivitySourceSummary {
                    source_id: "peer-channel-gateway-qqbot-local".into(),
                    title: "QQ Channel Peer".into(),
                    state: "ready".into(),
                    summary: "presence online".into(),
                    visibility: "compact".into(),
                    auto_promoted: false,
                },
            ],
            waiting_detail: Some("已收到，正在处理".into()),
            failure_detail: None,
            updated_at: "2026-04-20T15:12:00+08:00".into(),
        }),
        source_cards: vec![
            SourceActivityCardView {
                source_id: "system-agent".into(),
                source_kind: "system_agent".into(),
                title: "System Agent".into(),
                visibility: "compact".into(),
                state: "waiting".into(),
                summary: "已收到，正在处理".into(),
                focus_label: None,
                auto_promoted: true,
                current_activity: Some("已收到，正在处理".into()),
                recent_actions: Vec::new(),
                waiting_detail: Some("已收到，正在处理".into()),
                failure_detail: None,
                session_id: Some("session-1".into()),
                task_id: Some("task-1".into()),
                updated_at: "2026-04-20T15:12:00+08:00".into(),
            },
            SourceActivityCardView {
                source_id: "peer-channel-gateway-qqbot-local".into(),
                source_kind: "channel_gateway.qqbot".into(),
                title: "QQ Channel Peer".into(),
                visibility: "compact".into(),
                state: "ready".into(),
                summary: "presence online".into(),
                focus_label: None,
                auto_promoted: false,
                current_activity: Some("bound to session session-1".into()),
                recent_actions: Vec::new(),
                waiting_detail: None,
                failure_detail: None,
                session_id: Some("session-1".into()),
                task_id: Some("task-1".into()),
                updated_at: "2026-04-20T15:12:00+08:00".into(),
            },
        ],
        tool_semantics: Vec::new(),
    };
    let rendered = render_compact_text(&snapshot);
    assert_eq!(rendered.matches("已收到，正在处理").count(), 2);
    assert!(rendered.contains("👥 QQ Channel"));
    assert!(!rendered.contains("🧩 System Agent"));
    assert!(!rendered.contains("⏳ 已收到，正在处理\n⏳ 已收到，正在处理"));
}

#[test]
fn render_recent_action_for_provider_uses_model_only() {
    let action = ToolSemanticView {
        tool_name: "provider.call".into(),
        category: "model".into(),
        object_label: "ali-coding-plan.qwen3.6-plus".into(),
        detail: Some("模型响应已返回".into()),
        summary: "调用模型 ali-coding-plan.qwen3.6-plus".into(),
        ..ToolSemanticView::default()
    };
    let rendered = render_recent_action(&action);
    assert_eq!(rendered, "模型: ali-coding-plan.qwen3.6-plus");
}

#[test]
fn periodic_delivery_emits_once_for_changed_snapshot() {
    let runtime_home = temp_runtime_home();
    let session_rel = "sessions/2026/05/session-1";
    write_json(
        &runtime_home.join("runtime/current/last_run.json"),
        &serde_json::json!({
            "session_id": "session-1",
            "task_id": "task-1",
            "submitted_at": "2026-04-19T12:00:00+08:00",
            "session_recent_tool_records_path": format!("{session_rel}/tools/recent_tool_records.json"),
            "session_recent_turns_path": format!("{session_rel}/turns/recent_turns.json"),
            "current_execution_state_path": "runtime/current/current_execution_state.json"
        }),
    );
    write_json(
        &runtime_home.join("runtime/current/current_execution_state.json"),
        &serde_json::json!({
            "state_id": "state-1",
            "session_id": "session-1",
            "task_id": "task-1",
            "status": "running",
            "pending_input_count": 0,
            "accepts_user_input": true,
            "updated_at": "2026-04-19T12:00:04+08:00"
        }),
    );
    write_json(
        &runtime_home.join(format!("{session_rel}/tools/recent_tool_records.json")),
        &Vec::<serde_json::Value>::new(),
    );
    write_json(
        &runtime_home.join(format!("{session_rel}/turns/recent_turns.json")),
        &Vec::<serde_json::Value>::new(),
    );
    write_json(
        &runtime_home.join("runtime/peers/registry.json"),
        &serde_json::json!({"peers":[]}),
    );

    complete_builtin_qqbot_pairing(&runtime_home, "session-1", Some(5)).expect("pair");
    bind_target(&runtime_home, "session-1", "qqbot:c2c:user-1").expect("bind");

    let first = prepare_periodic_delivery(&runtime_home)
        .expect("prepare")
        .expect("first delivery");
    assert_eq!(first.reason, "diff");
    assert!(first.text.contains("🌐 System Agent"));

    mark_delivered(&runtime_home, &first.signature, &first.text, &first.reason)
        .expect("mark delivered");
    let second = prepare_periodic_delivery(&runtime_home).expect("prepare second");
    assert!(second.is_none());
}

#[test]
fn periodic_delivery_does_not_emit_heartbeat_for_unchanged_waiting_snapshot() {
    let runtime_home = temp_runtime_home();
    let session_rel = "sessions/2026/05/session-1";
    write_json(
        &runtime_home.join("runtime/current/last_run.json"),
        &serde_json::json!({
            "session_id": "session-1",
            "task_id": "task-1",
            "submitted_at": "2026-04-19T12:00:00+08:00",
            "session_recent_tool_records_path": format!("{session_rel}/tools/recent_tool_records.json"),
            "session_recent_turns_path": format!("{session_rel}/turns/recent_turns.json"),
            "current_execution_state_path": "runtime/current/current_execution_state.json"
        }),
    );
    write_json(
        &runtime_home.join("runtime/current/current_execution_state.json"),
        &serde_json::json!({
            "state_id": "state-1",
            "session_id": "session-1",
            "task_id": "task-1",
            "status": "waiting",
            "pending_input_count": 0,
            "accepts_user_input": false,
            "reason": "已收到，正在处理",
            "updated_at": "2026-04-19T12:00:04+08:00"
        }),
    );
    write_json(
        &runtime_home.join(format!("{session_rel}/tools/recent_tool_records.json")),
        &Vec::<serde_json::Value>::new(),
    );
    write_json(
        &runtime_home.join(format!("{session_rel}/turns/recent_turns.json")),
        &Vec::<serde_json::Value>::new(),
    );
    write_json(
        &runtime_home.join("runtime/peers/registry.json"),
        &serde_json::json!({"peers":[]}),
    );

    complete_builtin_qqbot_pairing(&runtime_home, "session-1", Some(5)).expect("pair");
    bind_target(&runtime_home, "session-1", "qqbot:c2c:user-1").expect("bind");
    let first = prepare_periodic_delivery(&runtime_home)
        .expect("prepare")
        .expect("first delivery");
    mark_delivered(&runtime_home, &first.signature, &first.text, &first.reason)
        .expect("mark delivered");
    let second = prepare_periodic_delivery(&runtime_home).expect("prepare second");
    assert!(second.is_none());
}

#[test]
fn periodic_delivery_skips_idle_peer_only_changes() {
    let runtime_home = temp_runtime_home();
    let session_rel = "sessions/2026/05/session-1";
    write_json(
        &runtime_home.join("runtime/current/last_run.json"),
        &serde_json::json!({
            "session_id": "session-1",
            "task_id": "task-1",
            "submitted_at": "2026-04-19T12:00:00+08:00",
            "session_recent_tool_records_path": format!("{session_rel}/tools/recent_tool_records.json"),
            "session_recent_turns_path": format!("{session_rel}/turns/recent_turns.json"),
            "current_execution_state_path": "runtime/current/current_execution_state.json"
        }),
    );
    write_json(
        &runtime_home.join("runtime/current/current_execution_state.json"),
        &serde_json::json!({
            "state_id": "state-1",
            "session_id": "session-1",
            "task_id": "task-1",
            "status": "idle",
            "pending_input_count": 0,
            "accepts_user_input": true,
            "reason": "closure completed",
            "updated_at": "2026-04-19T12:00:04+08:00"
        }),
    );
    write_json(
        &runtime_home.join(format!("{session_rel}/tools/recent_tool_records.json")),
        &Vec::<serde_json::Value>::new(),
    );
    write_json(
        &runtime_home.join(format!("{session_rel}/turns/recent_turns.json")),
        &Vec::<serde_json::Value>::new(),
    );
    write_json(
        &runtime_home.join("runtime/peers/registry.json"),
        &serde_json::json!({
            "peers": [{
                "peer_id": "peer-channel-gateway-qqbot-local",
                "peer_kind": "channel_gateway.qqbot",
                "presence_state": "online",
                "runtime_state": "bridge_ready",
                "connectivity_state": "connected",
                "binding_state": "bound",
                "lifecycle_state": "paired_active",
                "updated_at": "2026-04-19T12:00:05+08:00",
                "pairing_required": false,
                "session_valid": true,
                "session_id": "session-1"
            }]
        }),
    );

    complete_builtin_qqbot_pairing(&runtime_home, "session-1", Some(5)).expect("pair");
    bind_target(&runtime_home, "session-1", "qqbot:c2c:user-1").expect("bind");

    let prepared = prepare_periodic_delivery(&runtime_home).expect("prepare");
    assert!(prepared.is_none());
}

#[test]
fn current_delivery_signature_only_exists_for_deliverable_snapshot() {
    let runtime_home = temp_runtime_home();
    let session_rel = "sessions/2026/05/session-1";
    write_json(
        &runtime_home.join("runtime/current/last_run.json"),
        &serde_json::json!({
            "session_id": "session-1",
            "task_id": "task-1",
            "submitted_at": "2026-04-19T12:00:00+08:00",
            "session_recent_tool_records_path": format!("{session_rel}/tools/recent_tool_records.json"),
            "session_recent_turns_path": format!("{session_rel}/turns/recent_turns.json"),
            "current_execution_state_path": "runtime/current/current_execution_state.json"
        }),
    );
    write_json(
        &runtime_home.join("runtime/current/current_execution_state.json"),
        &serde_json::json!({
            "state_id": "state-1",
            "session_id": "session-1",
            "task_id": "task-1",
            "status": "running",
            "pending_input_count": 0,
            "accepts_user_input": false,
            "reason": "active closure running",
            "updated_at": "2026-04-19T12:00:04+08:00"
        }),
    );
    write_json(
        &runtime_home.join(format!("{session_rel}/tools/recent_tool_records.json")),
        &Vec::<serde_json::Value>::new(),
    );
    write_json(
        &runtime_home.join(format!("{session_rel}/turns/recent_turns.json")),
        &Vec::<serde_json::Value>::new(),
    );
    write_json(
        &runtime_home.join("runtime/peers/registry.json"),
        &serde_json::json!({"peers":[]}),
    );

    let signature = current_delivery_signature_if_deliverable(&runtime_home).expect("signature");
    assert!(signature.is_some());
}
