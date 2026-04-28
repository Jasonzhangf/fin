use super::*;
use crate::channel_peer_progress_policy::QqbotProgressPolicy;
use fin_contracts::{
    ActivitySourceSummary, SourceActivityCardView, ToolSemanticView, UserActivityCardView,
};

fn debug_policy() -> QqbotProgressPolicy {
    QqbotProgressPolicy {
        detail_level: "verbose".into(),
        ..QqbotProgressPolicy::default()
    }
}

#[test]
fn render_compact_text_debug_mode_lists_parallel_agents_and_deduped_tools() {
    let snapshot = ActivityCardsSnapshot {
        session_id: Some("session-dev".into()),
        task_id: Some("task-dev".into()),
        generated_at: "2026-04-24T12:00:00+08:00".into(),
        user_card: Some(UserActivityCardView {
            owner_source_id: "system-agent".into(),
            header: "frontstage".into(),
            state: "running".into(),
            focus_source_id: Some("system-agent".into()),
            focus_summary: Some("dispatching + resuming workers".into()),
            stage: Some("resume workers".into()),
            recent_items: vec!["Delegated worker-a".into()],
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
            updated_at: "2026-04-24T12:00:00+08:00".into(),
        }),
        source_cards: vec![
            SourceActivityCardView {
                source_id: "system-agent".into(),
                source_kind: "system_agent".into(),
                title: "System Agent".into(),
                visibility: "detailed".into(),
                state: "running".into(),
                summary: "dispatch owner loop".into(),
                focus_label: Some("task task-dev".into()),
                auto_promoted: true,
                current_activity: Some("resume_candidate_present".into()),
                recent_actions: vec![ToolSemanticView {
                    tool_name: "agent.assign".into(),
                    category: "delegation".into(),
                    detail: Some(
                        "peer_id=local-worker-a, target_worker_id=worker-a; task=inspect build log → assignment queued: assign-1 -> local-worker-a".into(),
                    ),
                    summary: "Delegated worker-a".into(),
                    status: "completed".into(),
                    ..ToolSemanticView::default()
                }],
                waiting_detail: None,
                failure_detail: None,
                session_id: Some("session-dev".into()),
                task_id: Some("task-dev".into()),
                updated_at: "2026-04-24T12:00:00+08:00".into(),
            },
            SourceActivityCardView {
                source_id: "mac.atlas".into(),
                source_kind: "project_worker".into(),
                title: "Worker proj/worker-a".into(),
                visibility: "detailed".into(),
                state: "running".into(),
                summary: "checking build log".into(),
                focus_label: Some("task task-build".into()),
                auto_promoted: true,
                current_activity: Some("phase=tool_result waiting exec".into()),
                recent_actions: Vec::new(),
                waiting_detail: None,
                failure_detail: None,
                session_id: Some("session-dev".into()),
                task_id: Some("task-build".into()),
                updated_at: "2026-04-24T12:00:00+08:00".into(),
            },
            SourceActivityCardView {
                source_id: "mac.nova".into(),
                source_kind: "project_worker".into(),
                title: "Worker proj/worker-b".into(),
                visibility: "detailed".into(),
                state: "waiting".into(),
                summary: "resume ready".into(),
                focus_label: Some("task task-test".into()),
                auto_promoted: true,
                current_activity: Some("prepared_idle".into()),
                recent_actions: Vec::new(),
                waiting_detail: Some("await explicit trigger".into()),
                failure_detail: None,
                session_id: Some("session-dev".into()),
                task_id: Some("task-test".into()),
                updated_at: "2026-04-24T12:00:00+08:00".into(),
            },
        ],
        tool_semantics: vec![
            ToolSemanticView {
                tool_name: "update_plan".into(),
                category: "plan".into(),
                detail: Some(
                    "steps=3, explanation=split work → plan updated with 3 step(s)".into(),
                ),
                summary: "Updated plan".into(),
                status: "completed".into(),
                ..ToolSemanticView::default()
            },
            ToolSemanticView {
                tool_name: "agent.assign".into(),
                category: "delegation".into(),
                detail: Some(
                    "peer_id=local-worker-a, target_worker_id=worker-a; task=inspect build log → assignment queued: assign-1 -> local-worker-a".into(),
                ),
                summary: "Delegated worker-a".into(),
                status: "completed".into(),
                ..ToolSemanticView::default()
            },
            ToolSemanticView {
                tool_name: "agent.assign".into(),
                category: "delegation".into(),
                detail: Some(
                    "peer_id=local-worker-a, target_worker_id=worker-a; task=inspect build log → assignment queued: assign-1 -> local-worker-a".into(),
                ),
                summary: "Delegated worker-a".into(),
                status: "completed".into(),
                ..ToolSemanticView::default()
            },
        ],
    };

    let rendered = channel_peer_activity_delivery_render::render_compact_text(
        &snapshot,
        true,
        &debug_policy(),
    );
    assert!(rendered.contains("🤖 system · 执行中(高)"));
    assert!(rendered.contains("🤖 atlas · 执行中(高)"));
    assert!(rendered.contains("🤖 nova · 等待中(中)"));
    assert!(rendered.contains("resume_candidate_present"));
    assert!(rendered.contains("等待工具结果"));
    assert!(rendered.contains("await explicit trigger"));
    assert!(rendered.contains("🛠 [agent.assign]"));
    assert!(rendered.contains("🛠 [update_plan]"));
    assert_eq!(rendered.matches("🛠 [agent.assign]").count(), 1);
}

#[test]
fn render_compact_text_debug_mode_prefers_latest_tool_by_started_at() {
    let snapshot = ActivityCardsSnapshot {
        session_id: Some("session-dev-latest".into()),
        task_id: Some("task-dev-latest".into()),
        generated_at: "2026-04-24T12:10:00+08:00".into(),
        user_card: Some(UserActivityCardView {
            owner_source_id: "system-agent".into(),
            header: "frontstage".into(),
            state: "running".into(),
            focus_source_id: Some("system-agent".into()),
            focus_summary: Some("system dispatching".into()),
            stage: Some("running owner loop".into()),
            recent_items: Vec::new(),
            active_sources: Vec::new(),
            total_sources: 1,
            running_sources: 1,
            waiting_sources: 0,
            failed_sources: 0,
            idle_sources: 0,
            waiting_detail: None,
            failure_detail: None,
            updated_at: "2026-04-24T12:10:00+08:00".into(),
        }),
        source_cards: vec![SourceActivityCardView {
            source_id: "system-agent".into(),
            source_kind: "system_agent".into(),
            title: "System Agent".into(),
            visibility: "detailed".into(),
            state: "running".into(),
            summary: "running owner loop".into(),
            focus_label: Some("task task-dev-latest".into()),
            auto_promoted: true,
            current_activity: Some("dispatch ready task".into()),
            recent_actions: Vec::new(),
            waiting_detail: None,
            failure_detail: None,
            session_id: Some("session-dev-latest".into()),
            task_id: Some("task-dev-latest".into()),
            updated_at: "2026-04-24T12:10:00+08:00".into(),
        }],
        tool_semantics: vec![
            ToolSemanticView {
                tool_call_id: "tool-older".into(),
                tool_name: "agent.assign".into(),
                category: "delegation".into(),
                detail: Some("agent_name=atlas, task=旧任务".into()),
                summary: "older assign".into(),
                status: "completed".into(),
                started_at: "2026-04-24T12:09:50+08:00".into(),
                ..ToolSemanticView::default()
            },
            ToolSemanticView {
                tool_call_id: "tool-newer".into(),
                tool_name: "mailbox.send".into(),
                category: "mailbox".into(),
                detail: Some("target_agent=nova, message={\"text\":\"最新同步\"}".into()),
                summary: "latest mailbox send".into(),
                status: "completed".into(),
                started_at: "2026-04-24T12:09:59+08:00".into(),
                ..ToolSemanticView::default()
            },
        ],
    };

    let rendered = channel_peer_activity_delivery_render::render_compact_text(
        &snapshot,
        true,
        &debug_policy(),
    );
    let latest_idx = rendered
        .find("🛠 [mailbox.send]")
        .expect("latest mailbox tool should be shown");
    let older_idx = rendered
        .find("🛠 [agent.assign]")
        .expect("older assign tool should still be shown");
    assert!(
        latest_idx < older_idx,
        "latest tool must appear before older tool in debug summary: {rendered}"
    );
    assert!(rendered.contains("最新同步"));
}

#[test]
fn render_heartbeat_text_debug_mode_keeps_parallel_agent_visibility() {
    let snapshot = ActivityCardsSnapshot {
        session_id: Some("session-heartbeat".into()),
        task_id: Some("task-heartbeat".into()),
        generated_at: "2026-04-24T12:01:00+08:00".into(),
        user_card: Some(UserActivityCardView {
            owner_source_id: "system-agent".into(),
            header: "frontstage".into(),
            state: "waiting".into(),
            focus_source_id: Some("system-agent".into()),
            focus_summary: Some("waiting worker resume".into()),
            stage: Some("resume workers".into()),
            recent_items: vec!["Delegated worker-a".into()],
            active_sources: Vec::new(),
            total_sources: 2,
            running_sources: 0,
            waiting_sources: 2,
            failed_sources: 0,
            idle_sources: 0,
            waiting_detail: Some("waiting worker resume".into()),
            failure_detail: None,
            updated_at: "2026-04-24T12:00:30+08:00".into(),
        }),
        source_cards: vec![
            SourceActivityCardView {
                source_id: "system-agent".into(),
                source_kind: "system_agent".into(),
                title: "System Agent".into(),
                visibility: "detailed".into(),
                state: "waiting".into(),
                summary: "resume manager".into(),
                focus_label: Some("task task-heartbeat".into()),
                auto_promoted: true,
                current_activity: Some("resume_candidate_present".into()),
                recent_actions: Vec::new(),
                waiting_detail: Some("waiting worker resume".into()),
                failure_detail: None,
                session_id: Some("session-heartbeat".into()),
                task_id: Some("task-heartbeat".into()),
                updated_at: "2026-04-24T12:00:20+08:00".into(),
            },
            SourceActivityCardView {
                source_id: "mac.nova".into(),
                source_kind: "project_worker".into(),
                title: "Worker proj/worker-b".into(),
                visibility: "detailed".into(),
                state: "waiting".into(),
                summary: "resume ready".into(),
                focus_label: Some("task task-test".into()),
                auto_promoted: true,
                current_activity: Some("prepared_idle".into()),
                recent_actions: Vec::new(),
                waiting_detail: Some("await explicit trigger".into()),
                failure_detail: None,
                session_id: Some("session-heartbeat".into()),
                task_id: Some("task-test".into()),
                updated_at: "2026-04-24T12:00:25+08:00".into(),
            },
        ],
        tool_semantics: vec![ToolSemanticView {
            tool_name: "agent.assign".into(),
            category: "delegation".into(),
            detail: Some(
                "peer_id=local-worker-b, target_worker_id=worker-b; task=resume test task → assignment queued".into(),
            ),
            summary: "Delegated worker-b".into(),
            status: "completed".into(),
            ..ToolSemanticView::default()
        }],
    };

    let rendered = channel_peer_activity_delivery_render::render_heartbeat_text(
        &snapshot,
        true,
        &debug_policy(),
    );
    assert!(rendered.contains("⏱ 进度心跳"));
    assert!(rendered.contains("🤖 system · 等待中(中)"));
    assert!(rendered.contains("🤖 nova · 等待中(中)"));
    assert!(rendered.contains("🛠 [agent.assign]"));
}

#[test]
fn render_heartbeat_text_returns_empty_for_generic_waiting_without_progress() {
    let snapshot = ActivityCardsSnapshot {
        session_id: Some("session-generic".into()),
        task_id: None,
        generated_at: "2026-04-24T12:01:00+08:00".into(),
        user_card: Some(UserActivityCardView {
            owner_source_id: "system-agent".into(),
            header: "frontstage".into(),
            state: "waiting".into(),
            focus_source_id: Some("system-agent".into()),
            focus_summary: Some("已收到，正在处理".into()),
            stage: Some("已收到，正在处理".into()),
            recent_items: Vec::new(),
            active_sources: Vec::new(),
            total_sources: 1,
            running_sources: 0,
            waiting_sources: 1,
            failed_sources: 0,
            idle_sources: 0,
            waiting_detail: Some("已收到，正在处理".into()),
            failure_detail: None,
            updated_at: "2026-04-24T12:00:30+08:00".into(),
        }),
        source_cards: vec![SourceActivityCardView {
            source_id: "system-agent".into(),
            source_kind: "system_agent".into(),
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
            session_id: Some("session-generic".into()),
            task_id: None,
            updated_at: "2026-04-24T12:00:20+08:00".into(),
        }],
        tool_semantics: Vec::new(),
    };

    let rendered = channel_peer_activity_delivery_render::render_heartbeat_text(
        &snapshot,
        false,
        &QqbotProgressPolicy::default(),
    );
    assert!(rendered.is_empty());
}

#[test]
fn render_compact_text_debug_mode_uses_device_prefix_when_multiple_devices_exist() {
    let snapshot = ActivityCardsSnapshot {
        session_id: Some("session-multi-device".into()),
        task_id: Some("task-multi-device".into()),
        generated_at: "2026-04-24T12:05:00+08:00".into(),
        user_card: Some(UserActivityCardView {
            owner_source_id: "system-agent".into(),
            header: "frontstage".into(),
            state: "running".into(),
            focus_source_id: Some("system-agent".into()),
            focus_summary: Some("cross-device dispatch".into()),
            stage: Some("cross-device dispatch".into()),
            recent_items: Vec::new(),
            active_sources: Vec::new(),
            total_sources: 3,
            running_sources: 3,
            waiting_sources: 0,
            failed_sources: 0,
            idle_sources: 0,
            waiting_detail: None,
            failure_detail: None,
            updated_at: "2026-04-24T12:05:00+08:00".into(),
        }),
        source_cards: vec![
            SourceActivityCardView {
                source_id: "system-agent".into(),
                source_kind: "system_agent".into(),
                title: "System Agent".into(),
                visibility: "detailed".into(),
                state: "running".into(),
                summary: "owner loop".into(),
                focus_label: None,
                auto_promoted: true,
                current_activity: Some("dispatching".into()),
                recent_actions: Vec::new(),
                waiting_detail: None,
                failure_detail: None,
                session_id: Some("session-multi-device".into()),
                task_id: Some("task-multi-device".into()),
                updated_at: "2026-04-24T12:05:00+08:00".into(),
            },
            SourceActivityCardView {
                source_id: "mac.atlas".into(),
                source_kind: "project_worker".into(),
                title: "Worker A".into(),
                visibility: "detailed".into(),
                state: "running".into(),
                summary: "running".into(),
                focus_label: None,
                auto_promoted: true,
                current_activity: Some("tool exec".into()),
                recent_actions: Vec::new(),
                waiting_detail: None,
                failure_detail: None,
                session_id: Some("session-multi-device".into()),
                task_id: Some("task-a".into()),
                updated_at: "2026-04-24T12:05:00+08:00".into(),
            },
            SourceActivityCardView {
                source_id: "mbp.nova".into(),
                source_kind: "project_worker".into(),
                title: "Worker B".into(),
                visibility: "detailed".into(),
                state: "running".into(),
                summary: "running".into(),
                focus_label: None,
                auto_promoted: true,
                current_activity: Some("tool exec".into()),
                recent_actions: Vec::new(),
                waiting_detail: None,
                failure_detail: None,
                session_id: Some("session-multi-device".into()),
                task_id: Some("task-b".into()),
                updated_at: "2026-04-24T12:05:00+08:00".into(),
            },
        ],
        tool_semantics: Vec::new(),
    };

    let rendered = channel_peer_activity_delivery_render::render_compact_text(
        &snapshot,
        true,
        &debug_policy(),
    );
    assert!(rendered.contains("🤖 mac.atlas · 执行中(高)"));
    assert!(rendered.contains("🤖 mbp.nova · 执行中(高)"));
}

#[test]
fn render_compact_text_uses_external_peer_alias_in_active_sources() {
    let snapshot = ActivityCardsSnapshot {
        session_id: Some("session-peer-alias".into()),
        task_id: Some("task-peer-alias".into()),
        generated_at: "2026-04-24T12:06:00+08:00".into(),
        user_card: Some(UserActivityCardView {
            owner_source_id: "system-agent".into(),
            header: "frontstage".into(),
            state: "running".into(),
            focus_source_id: Some("system-agent".into()),
            focus_summary: Some("sync external peer".into()),
            stage: Some("sync external peer".into()),
            recent_items: Vec::new(),
            active_sources: Vec::new(),
            total_sources: 2,
            running_sources: 2,
            waiting_sources: 0,
            failed_sources: 0,
            idle_sources: 0,
            waiting_detail: None,
            failure_detail: None,
            updated_at: "2026-04-24T12:06:00+08:00".into(),
        }),
        source_cards: vec![
            SourceActivityCardView {
                source_id: "system-agent".into(),
                source_kind: "system_agent".into(),
                title: "System Agent".into(),
                visibility: "detailed".into(),
                state: "running".into(),
                summary: "owner loop".into(),
                focus_label: None,
                auto_promoted: true,
                current_activity: Some("sync external peer".into()),
                recent_actions: Vec::new(),
                waiting_detail: None,
                failure_detail: None,
                session_id: Some("session-peer-alias".into()),
                task_id: Some("task-peer-alias".into()),
                updated_at: "2026-04-24T12:06:00+08:00".into(),
            },
            SourceActivityCardView {
                source_id: "peer-remote-1".into(),
                source_kind: "project_agent_remote".into(),
                title: "peer.alpha".into(),
                visibility: "detailed".into(),
                state: "running".into(),
                summary: "connected".into(),
                focus_label: None,
                auto_promoted: false,
                current_activity: Some("bound to remote task".into()),
                recent_actions: Vec::new(),
                waiting_detail: None,
                failure_detail: None,
                session_id: None,
                task_id: None,
                updated_at: "2026-04-24T12:06:00+08:00".into(),
            },
        ],
        tool_semantics: Vec::new(),
    };

    let rendered = channel_peer_activity_delivery_render::render_compact_text(
        &snapshot,
        false,
        &debug_policy(),
    );
    assert!(rendered.contains("👥 peer.alpha 执行中"));
}

#[test]
fn render_recent_action_compresses_collab_and_plan_tools() {
    let assign = ToolSemanticView {
        tool_name: "agent.assign".into(),
        category: "delegation".into(),
        detail: Some(
            "agent_name=atlas, worker_id=worker-atlas; task=inspect logs → assignment queued: assign-1 -> atlas"
                .into(),
        ),
        status: "completed".into(),
        ..ToolSemanticView::default()
    };
    assert_eq!(
        channel_peer_activity_delivery_render::render_recent_action(&assign),
        "派发: 给 atlas：inspect logs"
    );

    let mailbox_send = ToolSemanticView {
        tool_name: "mailbox.send".into(),
        category: "mailbox".into(),
        detail: Some(
            r#"target_agent=nova, worker_id=worker-nova, route=worker_local_alias, message={"kind":"assignment","text":"inspect logs"} → enqueued mailbox message: msg-1"#
                .into(),
        ),
        status: "completed".into(),
        ..ToolSemanticView::default()
    };
    assert_eq!(
        channel_peer_activity_delivery_render::render_recent_action(&mailbox_send),
        "协作: 发给 nova：inspect logs"
    );

    let mailbox_poll = ToolSemanticView {
        tool_name: "mailbox.poll".into(),
        category: "mailbox".into(),
        detail: Some(
            "agent_name=nova, worker_id=worker-nova, route=worker_local_alias, limit=20, consume=true → mailbox nova: messages=1, remaining=0, ids=msg-1"
                .into(),
        ),
        status: "completed".into(),
        ..ToolSemanticView::default()
    };
    assert_eq!(
        channel_peer_activity_delivery_render::render_recent_action(&mailbox_poll),
        "协作: 收取 nova：1 条，余 0"
    );

    let ensure_peer = ToolSemanticView {
        tool_name: "daemon.ensure_peer".into(),
        category: "peer".into(),
        detail: Some(
            "peer_kind=project_worker, agent_name=builder, lease_ttl_ms=30000 → lease + heartbeat persisted for builder"
                .into(),
        ),
        status: "completed".into(),
        ..ToolSemanticView::default()
    };
    assert_eq!(
        channel_peer_activity_delivery_render::render_recent_action(&ensure_peer),
        "Peer: 确保 builder 在线"
    );

    let update_plan = ToolSemanticView {
        tool_name: "update_plan".into(),
        category: "plan".into(),
        detail: Some("steps=3, explanation=split work → plan updated with 3 step(s)".into()),
        status: "completed".into(),
        ..ToolSemanticView::default()
    };
    assert_eq!(
        channel_peer_activity_delivery_render::render_recent_action(&update_plan),
        "计划: 3 步：split work"
    );
}

#[test]
fn render_dev_tool_lines_hide_zero_result_mailbox_poll() {
    let snapshot = ActivityCardsSnapshot {
        session_id: Some("session-1".into()),
        task_id: Some("task-1".into()),
        generated_at: "2026-04-24T19:10:00+08:00".into(),
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
            updated_at: "2026-04-24T19:10:00+08:00".into(),
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
            recent_actions: Vec::new(),
            waiting_detail: Some("等待 worker 回报".into()),
            failure_detail: None,
            session_id: Some("session-1".into()),
            task_id: Some("task-1".into()),
            updated_at: "2026-04-24T19:10:00+08:00".into(),
        }],
        tool_semantics: vec![
            ToolSemanticView {
                tool_name: "mailbox.poll".into(),
                category: "mailbox".into(),
                status: "completed".into(),
                detail: Some(
                    "agent_name=worker-01, peer_id=worker-01, limit=10, consume=false → mailbox worker-01: messages=0, remaining=0, ids="
                        .into(),
                ),
                started_at: "2026-04-24T19:10:00+08:00".into(),
                ..ToolSemanticView::default()
            },
            ToolSemanticView {
                tool_name: "agent.assign".into(),
                category: "delegation".into(),
                status: "completed".into(),
                detail: Some(
                    "agent_name=worker-01, worker_id=worker-01; task=inspect logs → assignment queued: assign-1 -> worker-01"
                        .into(),
                ),
                started_at: "2026-04-24T19:09:59+08:00".into(),
                ..ToolSemanticView::default()
            },
        ],
    };

    let rendered = channel_peer_activity_delivery_render::render_compact_text(
        &snapshot,
        false,
        &debug_policy(),
    );
    assert!(rendered.contains("派发: 给 worker-01：inspect logs"));
    assert!(!rendered.contains("收取 worker-01：0 条，余 0"));
}
