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
            total_sources: 1,
            running_sources: 1,
            waiting_sources: 0,
            failed_sources: 0,
            idle_sources: 0,
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
    assert!(rendered.contains("📡 状态卡 · system"));
    assert!(rendered.contains("🧮 资源 · 总1 · 运行1 · 等待0 · 空闲0"));
    assert!(rendered.contains("✅ 命令:"));
}

#[test]
fn render_compact_text_uses_failure_icon_for_failed_tool_line() {
    let snapshot = ActivityCardsSnapshot {
        session_id: Some("session-1".into()),
        task_id: Some("task-1".into()),
        generated_at: "2026-04-24T12:00:00+08:00".into(),
        user_card: Some(UserActivityCardView {
            owner_source_id: "system-agent".into(),
            header: "frontstage".into(),
            state: "waiting".into(),
            focus_source_id: Some("system-agent".into()),
            focus_summary: Some("等待 worker 回报".into()),
            stage: Some("等待 worker 回报".into()),
            recent_items: Vec::new(),
            active_sources: vec![],
            total_sources: 1,
            running_sources: 0,
            waiting_sources: 1,
            failed_sources: 0,
            idle_sources: 0,
            waiting_detail: Some("等待 worker 回报".into()),
            failure_detail: None,
            updated_at: "2026-04-24T12:00:00+08:00".into(),
        }),
        source_cards: vec![SourceActivityCardView {
            source_id: "system-agent".into(),
            source_kind: "system_agent".into(),
            title: "System Agent".into(),
            visibility: "detailed".into(),
            state: "waiting".into(),
            summary: "等待 worker 回报".into(),
            focus_label: None,
            auto_promoted: true,
            current_activity: Some("等待 worker 回报".into()),
            recent_actions: vec![ToolSemanticView {
                tool_name: "update_plan".into(),
                category: "plan".into(),
                status: "failed".into(),
                detail: Some("补全 steps 数组；重试：补充 steps[{step,status}]".into()),
                started_at: "2026-04-24T12:00:00+08:00".into(),
                ..ToolSemanticView::default()
            }],
            waiting_detail: Some("等待 worker 回报".into()),
            failure_detail: None,
            session_id: Some("session-1".into()),
            task_id: Some("task-1".into()),
            updated_at: "2026-04-24T12:00:00+08:00".into(),
        }],
        tool_semantics: Vec::new(),
    };
    let rendered = render_compact_text(&snapshot);
    assert!(rendered.contains("❌ 失败:"));
    assert!(!rendered.contains("✅ 失败:"));
}

#[test]
fn render_compact_text_treats_system_entry_title_as_system_name() {
    let snapshot = ActivityCardsSnapshot {
        session_id: Some("session-1".into()),
        task_id: Some("task-1".into()),
        generated_at: "2026-04-24T12:00:00+08:00".into(),
        user_card: Some(UserActivityCardView {
            owner_source_id: "system-agent".into(),
            header: "frontstage".into(),
            state: "waiting".into(),
            focus_source_id: Some("mac.system".into()),
            focus_summary: Some("已收到，正在处理".into()),
            stage: Some("已收到，正在处理".into()),
            recent_items: Vec::new(),
            active_sources: Vec::new(),
            total_sources: 2,
            running_sources: 0,
            waiting_sources: 1,
            failed_sources: 0,
            idle_sources: 1,
            waiting_detail: Some("已收到，正在处理".into()),
            failure_detail: None,
            updated_at: "2026-04-24T12:00:00+08:00".into(),
        }),
        source_cards: vec![
            SourceActivityCardView {
                source_id: "mac.system".into(),
                source_kind: "system_entry".into(),
                title: "System Agent".into(),
                visibility: "detailed".into(),
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
                updated_at: "2026-04-24T12:00:00+08:00".into(),
            },
            SourceActivityCardView {
                source_id: "mac.system-worker-01".into(),
                source_kind: "system_worker".into(),
                title: "System Worker system-worker-01".into(),
                visibility: "compact".into(),
                state: "idle".into(),
                summary: "worker ready".into(),
                focus_label: None,
                auto_promoted: false,
                current_activity: Some("worker ready".into()),
                recent_actions: Vec::new(),
                waiting_detail: None,
                failure_detail: None,
                session_id: Some("session-1".into()),
                task_id: Some("task-1".into()),
                updated_at: "2026-04-24T12:00:00+08:00".into(),
            },
        ],
        tool_semantics: Vec::new(),
    };
    let rendered = render_compact_text(&snapshot);
    assert!(rendered.contains("📡 状态卡 · system"));
    assert!(rendered.contains("👤 等待: system"));
    assert!(rendered.contains("空闲: worker-01"));
    assert!(!rendered.contains("📡 状态卡 · System Agent"));
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
            total_sources: 2,
            running_sources: 0,
            waiting_sources: 1,
            failed_sources: 0,
            idle_sources: 1,
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
    assert!(rendered.contains("👥 QQ 就绪 · 已绑定会话 session-1"));
    assert!(!rendered.contains("QQ ready"));
    assert!(rendered.contains("🧮 资源 · 总1 · 运行0 · 等待1 · 空闲0"));
    assert!(rendered.contains("👤 等待: system"));
    assert!(!rendered.contains("QQ Channel Peer,"));
    assert!(!rendered.contains("🧩 System Agent"));
    assert!(!rendered.contains("⏳ 已收到，正在处理\n⏳ 已收到，正在处理"));
}

#[test]
fn render_compact_text_hides_zero_result_mailbox_poll_lines() {
    let snapshot = ActivityCardsSnapshot {
        session_id: Some("session-1".into()),
        task_id: Some("task-1".into()),
        generated_at: "2026-04-24T19:00:00+08:00".into(),
        user_card: Some(UserActivityCardView {
            owner_source_id: "system-agent".into(),
            header: "frontstage".into(),
            state: "waiting".into(),
            focus_source_id: Some("system-agent".into()),
            focus_summary: Some("等待 worker 回报".into()),
            stage: Some("等待 worker 回报".into()),
            recent_items: Vec::new(),
            active_sources: Vec::new(),
            total_sources: 1,
            running_sources: 0,
            waiting_sources: 1,
            failed_sources: 0,
            idle_sources: 0,
            waiting_detail: Some("等待 worker 回报".into()),
            failure_detail: None,
            updated_at: "2026-04-24T19:00:00+08:00".into(),
        }),
        source_cards: vec![SourceActivityCardView {
            source_id: "system-agent".into(),
            source_kind: "system_agent".into(),
            title: "System Agent".into(),
            visibility: "detailed".into(),
            state: "waiting".into(),
            summary: "等待 worker 回报".into(),
            focus_label: None,
            auto_promoted: true,
            current_activity: Some("等待 worker 回报".into()),
            recent_actions: vec![
                ToolSemanticView {
                    tool_name: "mailbox.poll".into(),
                    category: "mailbox".into(),
                    status: "completed".into(),
                    detail: Some(
                        "agent_name=worker-01, peer_id=worker-01, limit=10, consume=false → mailbox worker-01: messages=0, remaining=0, ids="
                            .into(),
                    ),
                    started_at: "2026-04-24T19:00:00+08:00".into(),
                    ..ToolSemanticView::default()
                },
                ToolSemanticView {
                    tool_name: "project.task.list".into(),
                    category: "read".into(),
                    status: "completed".into(),
                    detail: Some("tasks=2 [task-alpha@session-a, task-beta@session-b]".into()),
                    started_at: "2026-04-24T18:59:59+08:00".into(),
                    ..ToolSemanticView::default()
                },
            ],
            waiting_detail: Some("等待 worker 回报".into()),
            failure_detail: None,
            session_id: Some("session-1".into()),
            task_id: Some("task-1".into()),
            updated_at: "2026-04-24T19:00:00+08:00".into(),
        }],
        tool_semantics: Vec::new(),
    };

    let rendered = render_compact_text(&snapshot);
    assert!(rendered.contains("✅ 查看: 任务 2 条"));
    assert!(!rendered.contains("收取 worker-01：0 条，余 0"));
}

#[test]
fn render_compact_text_does_not_use_global_tool_failures_when_focus_source_has_no_recent_actions()
 {
    let snapshot = ActivityCardsSnapshot {
        session_id: Some("session-1".into()),
        task_id: Some("task-1".into()),
        generated_at: "2026-04-24T21:15:00+08:00".into(),
        user_card: Some(UserActivityCardView {
            owner_source_id: "system-agent".into(),
            header: "frontstage".into(),
            state: "waiting".into(),
            focus_source_id: Some("system-agent".into()),
            focus_summary: Some("已派发任务，等待 worker 回报".into()),
            stage: Some("dispatch_ready_tasks count=1 [task-1]".into()),
            recent_items: Vec::new(),
            active_sources: vec![ActivitySourceSummary {
                source_id: "system-agent".into(),
                title: "System Agent".into(),
                state: "waiting".into(),
                summary: "已派发任务，等待 worker 回报".into(),
                visibility: "compact".into(),
                auto_promoted: true,
            }],
            total_sources: 1,
            running_sources: 0,
            waiting_sources: 1,
            failed_sources: 0,
            idle_sources: 0,
            waiting_detail: Some("最近已派发 1 个任务；当前关注 task-1；等待 worker 启动或回报".into()),
            failure_detail: None,
            updated_at: "2026-04-24T21:15:00+08:00".into(),
        }),
        source_cards: vec![SourceActivityCardView {
            source_id: "system-agent".into(),
            source_kind: "system_agent".into(),
            title: "System Agent".into(),
            visibility: "compact".into(),
            state: "waiting".into(),
            summary: "已派发任务，等待 worker 回报".into(),
            focus_label: None,
            auto_promoted: true,
            current_activity: Some("dispatch_ready_tasks count=1 [task-1]".into()),
            recent_actions: Vec::new(),
            waiting_detail: Some("最近已派发 1 个任务；当前关注 task-1；等待 worker 启动或回报".into()),
            failure_detail: None,
            session_id: Some("session-1".into()),
            task_id: Some("task-1".into()),
            updated_at: "2026-04-24T21:15:00+08:00".into(),
        }],
        tool_semantics: vec![ToolSemanticView {
            tool_name: "project.task.status".into(),
            status: "failed".into(),
            category: "other".into(),
            detail: Some(
                "referenced runtime target does not exist；重试：inspect current task/session truth first"
                    .into(),
            ),
            started_at: "2026-04-24T12:25:08+08:00".into(),
            ..ToolSemanticView::default()
        }],
    };

    let rendered = render_compact_text(&snapshot);
    assert!(!rendered.contains("referenced runtime target does not exist"));
    assert!(!rendered.contains("❌ 失败:"));
}

#[test]
fn render_compact_text_preserves_full_session_task_and_worker_identifiers() {
    let snapshot = ActivityCardsSnapshot {
        session_id: Some("session-system-entry".into()),
        task_id: Some("task-codex-computer-use-mcp-toolu_3b67674b4f2d4".into()),
        generated_at: "2026-04-24T21:20:00+08:00".into(),
        user_card: Some(UserActivityCardView {
            owner_source_id: "system-agent".into(),
            header: "frontstage".into(),
            state: "waiting".into(),
            focus_source_id: Some("system-agent".into()),
            focus_summary: Some("已派发任务，等待 worker 回报".into()),
            stage: Some(
                "dispatch_ready_tasks count=4 [task-codex-hermes-fin-runtime_11111111, task-codex-computer-use-mcp-toolu_3b67674b4f2d4, task-fin-other-03, task-fin-other-04]"
                    .into(),
            ),
            recent_items: Vec::new(),
            active_sources: vec![ActivitySourceSummary {
                source_id: "system-agent".into(),
                title: "System Agent".into(),
                state: "waiting".into(),
                summary: "已派发任务，等待 worker 回报".into(),
                visibility: "detailed".into(),
                auto_promoted: true,
            }],
            total_sources: 5,
            running_sources: 0,
            waiting_sources: 1,
            failed_sources: 0,
            idle_sources: 4,
            waiting_detail: Some(
                "最近已派发 4 个任务；当前关注 task-codex-computer-use-mcp-toolu_3b67674b4f2d4；等待 worker 回报"
                    .into(),
            ),
            failure_detail: None,
            updated_at: "2026-04-24T21:20:00+08:00".into(),
        }),
        source_cards: vec![
            SourceActivityCardView {
                source_id: "system-agent".into(),
                source_kind: "system_agent".into(),
                title: "System Agent".into(),
                visibility: "detailed".into(),
                state: "waiting".into(),
                summary: "已派发任务，等待 worker 回报".into(),
                focus_label: None,
                auto_promoted: true,
                current_activity: Some(
                    "dispatch_ready_tasks count=4 [task-codex-hermes-fin-runtime_11111111, task-codex-computer-use-mcp-toolu_3b67674b4f2d4, task-fin-other-03, task-fin-other-04]"
                        .into(),
                ),
                recent_actions: Vec::new(),
                waiting_detail: Some(
                    "最近已派发 4 个任务；当前关注 task-codex-computer-use-mcp-toolu_3b67674b4f2d4；等待 worker 回报"
                        .into(),
                ),
                failure_detail: None,
                session_id: Some("session-system-entry".into()),
                task_id: Some("task-codex-computer-use-mcp-toolu_3b67674b4f2d4".into()),
                updated_at: "2026-04-24T21:20:00+08:00".into(),
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
                current_activity: Some("bound to session session-system-entry".into()),
                recent_actions: Vec::new(),
                waiting_detail: None,
                failure_detail: None,
                session_id: Some("session-system-entry".into()),
                task_id: Some("task-codex-computer-use-mcp-toolu_3b67674b4f2d4".into()),
                updated_at: "2026-04-24T21:20:00+08:00".into(),
            },
            SourceActivityCardView {
                source_id: "worker-01".into(),
                source_kind: "project_agent".into(),
                title: "worker-01".into(),
                visibility: "compact".into(),
                state: "idle".into(),
                summary: "ready".into(),
                focus_label: None,
                auto_promoted: false,
                current_activity: None,
                recent_actions: Vec::new(),
                waiting_detail: None,
                failure_detail: None,
                session_id: None,
                task_id: None,
                updated_at: "2026-04-24T21:20:00+08:00".into(),
            },
            SourceActivityCardView {
                source_id: "worker-02".into(),
                source_kind: "project_agent".into(),
                title: "worker-02".into(),
                visibility: "compact".into(),
                state: "idle".into(),
                summary: "ready".into(),
                focus_label: None,
                auto_promoted: false,
                current_activity: None,
                recent_actions: Vec::new(),
                waiting_detail: None,
                failure_detail: None,
                session_id: None,
                task_id: None,
                updated_at: "2026-04-24T21:20:00+08:00".into(),
            },
            SourceActivityCardView {
                source_id: "worker-03".into(),
                source_kind: "project_agent".into(),
                title: "worker-03".into(),
                visibility: "compact".into(),
                state: "idle".into(),
                summary: "ready".into(),
                focus_label: None,
                auto_promoted: false,
                current_activity: None,
                recent_actions: Vec::new(),
                waiting_detail: None,
                failure_detail: None,
                session_id: None,
                task_id: None,
                updated_at: "2026-04-24T21:20:00+08:00".into(),
            },
            SourceActivityCardView {
                source_id: "worker-04".into(),
                source_kind: "project_agent".into(),
                title: "worker-04".into(),
                visibility: "compact".into(),
                state: "idle".into(),
                summary: "ready".into(),
                focus_label: None,
                auto_promoted: false,
                current_activity: None,
                recent_actions: Vec::new(),
                waiting_detail: None,
                failure_detail: None,
                session_id: None,
                task_id: None,
                updated_at: "2026-04-24T21:20:00+08:00".into(),
            },
        ],
        tool_semantics: Vec::new(),
    };

    let rendered = render_compact_text(&snapshot);
    assert!(rendered.contains("👥 QQ 就绪 · 已绑定会话 session-system-entry"));
    assert!(
        rendered.contains("👤 等待: system · 空闲: worker-01, worker-02, worker-03, worker-04")
    );
    assert!(rendered.contains("task-codex-computer-use-mcp-toolu_3b67674b4f2d4"));
    assert!(!rendered.contains("+1"));
    assert!(!rendered.contains('…'));
}

#[test]
fn render_heartbeat_text_preserves_full_waiting_identifiers() {
    let snapshot = ActivityCardsSnapshot {
        session_id: Some("session-system-entry".into()),
        task_id: Some("task-codex-computer-use-mcp-toolu_3b67674b4f2d4".into()),
        generated_at: "2026-04-24T21:25:00+08:00".into(),
        user_card: Some(UserActivityCardView {
            owner_source_id: "system-agent".into(),
            header: "frontstage".into(),
            state: "waiting".into(),
            focus_source_id: Some("system-agent".into()),
            focus_summary: Some("已派发任务，等待 worker 回报".into()),
            stage: Some("dispatch_ready_tasks count=4 [task-codex-computer-use-mcp-toolu_3b67674b4f2d4]".into()),
            recent_items: Vec::new(),
            active_sources: vec![ActivitySourceSummary {
                source_id: "system-agent".into(),
                title: "System Agent".into(),
                state: "waiting".into(),
                summary: "已派发任务，等待 worker 回报".into(),
                visibility: "detailed".into(),
                auto_promoted: true,
            }],
            total_sources: 1,
            running_sources: 0,
            waiting_sources: 1,
            failed_sources: 0,
            idle_sources: 0,
            waiting_detail: Some(
                "最近已派发 4 个任务；当前关注 task-codex-computer-use-mcp-toolu_3b67674b4f2d4；等待 worker 回报"
                    .into(),
            ),
            failure_detail: None,
            updated_at: "2026-04-24T21:23:00+08:00".into(),
        }),
        source_cards: vec![SourceActivityCardView {
            source_id: "system-agent".into(),
            source_kind: "system_agent".into(),
            title: "System Agent".into(),
            visibility: "detailed".into(),
            state: "waiting".into(),
            summary: "已派发任务，等待 worker 回报".into(),
            focus_label: None,
            auto_promoted: true,
            current_activity: Some(
                "dispatch_ready_tasks count=4 [task-codex-computer-use-mcp-toolu_3b67674b4f2d4]".into(),
            ),
            recent_actions: Vec::new(),
            waiting_detail: Some(
                "最近已派发 4 个任务；当前关注 task-codex-computer-use-mcp-toolu_3b67674b4f2d4；等待 worker 回报"
                    .into(),
            ),
            failure_detail: None,
            session_id: Some("session-system-entry".into()),
            task_id: Some("task-codex-computer-use-mcp-toolu_3b67674b4f2d4".into()),
            updated_at: "2026-04-24T21:23:00+08:00".into(),
        }],
        tool_semantics: Vec::new(),
    };

    let rendered = channel_peer_activity_delivery_render::render_compact_text(
        &snapshot,
    );
    assert!(rendered.contains(
        "📍 当前等待: 最近已派发 4 个任务；当前关注 task-codex-computer-use-mcp-toolu_3b67674b4f2d4；等待 worker 回报"
    ));
    assert!(!rendered.contains('…'));
}

#[test]
fn render_compact_text_hides_tool_lines_in_compact_mode_when_disabled_by_policy() {
    let snapshot = ActivityCardsSnapshot {
        session_id: Some("session-1".into()),
        task_id: Some("task-1".into()),
        generated_at: "2026-04-23T12:00:00+08:00".into(),
        user_card: Some(UserActivityCardView {
            owner_source_id: "system-agent".into(),
            header: "frontstage".into(),
            state: "running".into(),
            focus_source_id: Some("system-agent".into()),
            focus_summary: Some("dispatching workers".into()),
            stage: Some("owner dispatch".into()),
            recent_items: vec!["claimed worker-builder".into()],
            active_sources: vec![ActivitySourceSummary {
                source_id: "system-agent".into(),
                title: "System Agent".into(),
                state: "running".into(),
                summary: "dispatch".into(),
                visibility: "detailed".into(),
                auto_promoted: true,
            }],
            total_sources: 3,
            running_sources: 2,
            waiting_sources: 1,
            failed_sources: 0,
            idle_sources: 0,
            waiting_detail: None,
            failure_detail: None,
            updated_at: "2026-04-23T12:00:00+08:00".into(),
        }),
        source_cards: vec![
            SourceActivityCardView {
                source_id: "mac.system".into(),
                source_kind: "system_agent".into(),
                title: "System Agent".into(),
                visibility: "detailed".into(),
                state: "running".into(),
                summary: "dispatch".into(),
                focus_label: None,
                auto_promoted: true,
                current_activity: Some("dispatching workers".into()),
                recent_actions: vec![ToolSemanticView {
                    category: "plan".into(),
                    detail: Some("claim worker-builder".into()),
                    summary: "claimed worker-builder".into(),
                    ..ToolSemanticView::default()
                }],
                waiting_detail: None,
                failure_detail: None,
                session_id: Some("session-1".into()),
                task_id: Some("task-1".into()),
                updated_at: "2026-04-23T12:00:00+08:00".into(),
            },
            SourceActivityCardView {
                source_id: "mac.system-worker-01".into(),
                source_kind: "system_worker".into(),
                title: "System Worker system-worker-01".into(),
                visibility: "compact".into(),
                state: "waiting".into(),
                summary: "awaiting tool".into(),
                focus_label: None,
                auto_promoted: false,
                current_activity: Some("waiting tool_result".into()),
                recent_actions: Vec::new(),
                waiting_detail: Some("waiting tool_result".into()),
                failure_detail: None,
                session_id: Some("session-1".into()),
                task_id: Some("task-1".into()),
                updated_at: "2026-04-23T12:00:00+08:00".into(),
            },
            SourceActivityCardView {
                source_id: "mac.system-worker-02".into(),
                source_kind: "system_worker".into(),
                title: "System Worker system-worker-02".into(),
                visibility: "compact".into(),
                state: "idle".into(),
                summary: "worker ready".into(),
                focus_label: None,
                auto_promoted: false,
                current_activity: Some("worker ready".into()),
                recent_actions: Vec::new(),
                waiting_detail: None,
                failure_detail: None,
                session_id: Some("session-1".into()),
                task_id: Some("task-1".into()),
                updated_at: "2026-04-23T12:00:00+08:00".into(),
            },
        ],
        tool_semantics: Vec::new(),
    };
    let rendered = render_compact_text(&snapshot);
    assert!(rendered.contains("🧮 资源 · 总3 · 运行1 · 等待1 · 空闲1"));
    assert!(!rendered.contains("👤 运行:"));
    assert!(!rendered.contains("✅ 计划:"));
    assert!(!rendered.contains("🧩 System Agent"));
}

#[test]
fn render_compact_text_surfaces_failed_retry_guidance_in_tool_lines() {
    let snapshot = ActivityCardsSnapshot {
        session_id: Some("session-err".into()),
        task_id: Some("task-err".into()),
        generated_at: "2026-04-24T00:30:00+08:00".into(),
        user_card: Some(UserActivityCardView {
            owner_source_id: "system-agent".into(),
            header: "frontstage".into(),
            state: "failed".into(),
            focus_source_id: Some("system-agent".into()),
            focus_summary: Some("agent presence query failed".into()),
            stage: Some("checking agent truth".into()),
            recent_items: vec!["失败: 重试：先生成 agent truth".into()],
            active_sources: vec![ActivitySourceSummary {
                source_id: "system-agent".into(),
                title: "System Agent".into(),
                state: "failed".into(),
                summary: "agent presence query failed".into(),
                visibility: "detailed".into(),
                auto_promoted: true,
            }],
            total_sources: 1,
            running_sources: 0,
            waiting_sources: 0,
            failed_sources: 1,
            idle_sources: 0,
            waiting_detail: None,
            failure_detail: Some("重试：先生成 agent truth".into()),
            updated_at: "2026-04-24T00:30:00+08:00".into(),
        }),
        source_cards: vec![SourceActivityCardView {
            source_id: "system-agent".into(),
            source_kind: "system_agent".into(),
            title: "System Agent".into(),
            visibility: "detailed".into(),
            state: "failed".into(),
            summary: "agent presence query failed".into(),
            focus_label: None,
            auto_promoted: true,
            current_activity: Some("checking agent truth".into()),
            recent_actions: vec![ToolSemanticView {
                category: "search".into(),
                tool_name: "agent.presence.list".into(),
                status: "failed".into(),
                detail: Some("重试：先生成 agent truth".into()),
                summary: "Searched agent presence (失败)".into(),
                ..ToolSemanticView::default()
            }],
            waiting_detail: None,
            failure_detail: Some("重试：先生成 agent truth".into()),
            session_id: Some("session-err".into()),
            task_id: Some("task-err".into()),
            updated_at: "2026-04-24T00:30:00+08:00".into(),
        }],
        tool_semantics: Vec::new(),
    };
    let rendered = render_compact_text(&snapshot);
    assert!(rendered.contains("❌ 重试：先生成 agent truth"));
    assert!(rendered.contains("✅ 失败: 重试：先生成 agent truth"));
}

#[test]
fn periodic_delivery_skips_first_ack_equivalent_waiting_snapshot() {
    let runtime_home = temp_runtime_home();
    let session_rel = "sessions/2026/04/session-1";
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

#[path = "channel_peer_activity_delivery_tests_periodic.rs"]
mod channel_peer_activity_delivery_tests_periodic;
