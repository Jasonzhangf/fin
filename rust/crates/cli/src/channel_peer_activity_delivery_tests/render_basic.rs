use super::*;

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
    assert!(rendered.contains("🌐 System Agent"));
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
    assert!(rendered.contains("✅ 计划:"));
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
    assert!(rendered.contains("🌐 System Agent"));
    assert!(rendered.contains("⏳ 等待中"));
    assert!(!rendered.contains("idle"));
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
    assert!(rendered.contains("👥"));
    assert!(rendered.contains("QQ Channel Pee"));
    assert!(!rendered.contains("QQ Channel Peer,"));
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
    assert!(rendered.contains("✅"));
    assert!(rendered.contains("task-alpha"));
}

#[test]
fn render_compact_text_does_not_use_global_tool_failures_when_focus_source_has_no_recent_actions() {
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
