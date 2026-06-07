use super::*;

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
    assert!(rendered.contains("👥"));
    assert!(rendered.contains("QQ Channel Pee"));
    assert!(rendered.contains("task-codex"));
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

    let rendered = channel_peer_activity_delivery_render::render_compact_text(&snapshot);
    assert!(rendered.contains("🌐"));
    assert!(rendered.contains("等待中"));
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
    assert!(rendered.contains("🌐"));
    assert!(rendered.contains("🔄 执行中"));
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
    assert!(rendered.contains("❌ 失败"));
    assert!(rendered.contains("重试：先生成 agent truth"));
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
