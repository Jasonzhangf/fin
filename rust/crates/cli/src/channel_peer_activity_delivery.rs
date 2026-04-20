use crate::{
    CliError,
    channel_peer::active_builtin_qqbot_session,
    channel_peer_activity_signature::{signature_source_card, signature_user_card},
    time::local_timestamp_now,
};
use fin_contracts::{ActivityCardsSnapshot, SourceActivityCardView, UserActivityCardView};
use fin_runtime::build_activity_cards;
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
};

const MAX_ACTIVE_SOURCES: usize = 3;
const SYSTEM_SOURCE_ID: &str = "system-agent";
const PENDING_INBOUND_NOTICE: &str = "已收到，正在处理";

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct QqbotActivityDeliveryState {
    #[serde(default)]
    pub(crate) session_id: Option<String>,
    #[serde(default)]
    pub(crate) target: Option<String>,
    #[serde(default)]
    pub(crate) last_user_signature: Option<String>,
    #[serde(default)]
    pub(crate) last_delivered_text: Option<String>,
    #[serde(default)]
    pub(crate) last_delivery_reason: Option<String>,
    #[serde(default)]
    pub(crate) last_delivery_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PreparedActivityDelivery {
    pub(crate) target: String,
    pub(crate) text: String,
    pub(crate) signature: String,
    pub(crate) reason: String,
}

pub(crate) fn bind_target(
    runtime_home: &Path,
    session_id: &str,
    target: &str,
) -> Result<QqbotActivityDeliveryState, CliError> {
    let mut state = load_state(runtime_home)?;
    state.session_id = Some(session_id.to_string());
    state.target = Some(target.to_string());
    persist_state(runtime_home, &state)?;
    Ok(state)
}

pub(crate) fn clear_target(runtime_home: &Path) -> Result<(), CliError> {
    let mut state = load_state(runtime_home)?;
    state.target = None;
    state.session_id = None;
    persist_state(runtime_home, &state)
}

pub(crate) fn prepare_periodic_delivery(
    runtime_home: &Path,
) -> Result<Option<PreparedActivityDelivery>, CliError> {
    let state = load_state(runtime_home)?;
    let Some(target) = state.target.clone() else {
        return Ok(None);
    };
    let Some(session_id) = state.session_id.clone() else {
        return Ok(None);
    };
    let Some(active_session_id) = active_builtin_qqbot_session(runtime_home)? else {
        clear_target(runtime_home)?;
        return Ok(None);
    };
    if active_session_id != session_id {
        clear_target(runtime_home)?;
        return Ok(None);
    }
    let snapshot = build_activity_cards(runtime_home)?;
    if snapshot
        .session_id
        .as_deref()
        .is_some_and(|current| current != session_id)
    {
        clear_target(runtime_home)?;
        return Ok(None);
    }
    if !should_emit_snapshot(&snapshot) {
        return Ok(None);
    }
    let signature = signature_for_snapshot(&snapshot);
    let rendered = render_compact_text(&snapshot);
    if rendered.trim().is_empty() {
        return Ok(None);
    }
    let changed = state.last_user_signature.as_deref() != Some(signature.as_str());
    if !changed {
        return Ok(None);
    }
    Ok(Some(PreparedActivityDelivery {
        target,
        text: rendered,
        signature,
        reason: "diff".into(),
    }))
}

pub(crate) fn current_delivery_signature_if_deliverable(
    runtime_home: &Path,
) -> Result<Option<String>, CliError> {
    let snapshot = build_activity_cards(runtime_home)?;
    if !should_emit_snapshot(&snapshot) {
        return Ok(None);
    }
    Ok(Some(signature_for_snapshot(&snapshot)))
}

#[allow(dead_code)]
pub(crate) fn prepare_embedded_reply(
    runtime_home: &Path,
    answer: &str,
) -> Result<Option<(String, String)>, CliError> {
    let snapshot = build_activity_cards(runtime_home)?;
    let rendered = render_compact_text(&snapshot);
    if rendered.trim().is_empty() {
        return Ok(None);
    }
    Ok(Some((
        format!("{}\n\n{}", answer.trim_end(), rendered),
        signature_for_snapshot(&snapshot),
    )))
}

pub(crate) fn mark_delivered(
    runtime_home: &Path,
    signature: &str,
    text: &str,
    reason: &str,
) -> Result<QqbotActivityDeliveryState, CliError> {
    let mut state = load_state(runtime_home)?;
    state.last_user_signature = Some(signature.to_string());
    state.last_delivered_text = Some(text.to_string());
    state.last_delivery_reason = Some(reason.to_string());
    state.last_delivery_at = Some(local_timestamp_now());
    persist_state(runtime_home, &state)?;
    Ok(state)
}

fn load_state(runtime_home: &Path) -> Result<QqbotActivityDeliveryState, CliError> {
    let path = state_path(runtime_home);
    match fs::read_to_string(&path) {
        Ok(body) => serde_json::from_str(&body).map_err(CliError::Serialize),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            Ok(QqbotActivityDeliveryState::default())
        }
        Err(source) => Err(CliError::ReadFile {
            path: path.display().to_string(),
            source,
        }),
    }
}

fn persist_state(runtime_home: &Path, state: &QqbotActivityDeliveryState) -> Result<(), CliError> {
    let path = state_path(runtime_home);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|source| CliError::WriteFile {
            path: parent.display().to_string(),
            source,
        })?;
    }
    fs::write(
        &path,
        serde_json::to_vec_pretty(state).map_err(CliError::Serialize)?,
    )
    .map_err(|source| CliError::WriteFile {
        path: path.display().to_string(),
        source,
    })
}

fn state_path(runtime_home: &Path) -> PathBuf {
    runtime_home.join("runtime/peers/qqbot/activity_delivery_state.json")
}

fn signature_for_snapshot(snapshot: &ActivityCardsSnapshot) -> String {
    serde_json::to_string(&serde_json::json!({
        "session_id": snapshot.session_id,
        "task_id": snapshot.task_id,
        "user_card": snapshot.user_card.as_ref().map(signature_user_card),
        "source_cards": snapshot.source_cards.iter().map(signature_source_card).collect::<Vec<_>>(),
    }))
    .unwrap_or_else(|_| "activity-cards-signature-error".into())
}

fn render_compact_text(snapshot: &ActivityCardsSnapshot) -> String {
    let Some(user_card) = snapshot.user_card.as_ref() else {
        return String::new();
    };
    let mut lines = vec![render_header(snapshot, user_card)];
    if let Some(active_sources) = render_active_sources(snapshot, user_card) {
        lines.push(active_sources);
    }
    if let Some(stage) = render_stage_line(user_card) {
        push_unique_line(&mut lines, stage);
    }
    if let Some(detail) = render_detail_line(user_card) {
        push_unique_line(&mut lines, detail);
    }
    let focus_source = snapshot
        .source_cards
        .iter()
        .find(|card| Some(card.source_id.as_str()) == user_card.focus_source_id.as_deref())
        .or_else(|| snapshot.source_cards.first());
    if let Some(source) = focus_source.and_then(render_source_supplement) {
        push_unique_line(&mut lines, source);
    }
    if let Some(source) = focus_source {
        for line in render_focus_action_lines(source, user_card) {
            push_unique_line(&mut lines, line);
        }
    }
    lines.retain(|line| !line.trim().is_empty());
    lines.join("\n")
}

fn should_emit_snapshot(snapshot: &ActivityCardsSnapshot) -> bool {
    let Some(user_card) = snapshot.user_card.as_ref() else {
        return false;
    };
    if has_pending_inbound_notice(user_card) {
        return true;
    }
    let Some(system_card) = snapshot
        .source_cards
        .iter()
        .find(|card| card.source_id == SYSTEM_SOURCE_ID)
    else {
        return false;
    };
    matches!(
        system_card.state.as_str(),
        "running" | "waiting" | "paused" | "failed"
    ) || system_card
        .current_activity
        .as_deref()
        .is_some_and(|value| value.starts_with("startup "))
}

fn has_pending_inbound_notice(user_card: &UserActivityCardView) -> bool {
    user_card.stage.as_deref() == Some(PENDING_INBOUND_NOTICE)
        || user_card.focus_summary.as_deref() == Some(PENDING_INBOUND_NOTICE)
        || user_card.waiting_detail.as_deref() == Some(PENDING_INBOUND_NOTICE)
        || user_card
            .recent_items
            .iter()
            .any(|item| item.as_str() == PENDING_INBOUND_NOTICE)
}

fn render_header(snapshot: &ActivityCardsSnapshot, user_card: &UserActivityCardView) -> String {
    let focus = snapshot
        .source_cards
        .iter()
        .find(|card| Some(card.source_id.as_str()) == user_card.focus_source_id.as_deref())
        .map(|card| card.title.as_str())
        .unwrap_or("System");
    let state = match user_card.state.as_str() {
        "running" => "🔄 执行中",
        "waiting" => "⏳ 等待中",
        "paused" => "⏸ 已暂停",
        "failed" => "❌ 失败",
        "ready" => "✅ 就绪",
        "idle" => "💤 空闲",
        other => other,
    };
    format!(
        "🌐 {} · {} · {}",
        short_text(focus, 18),
        state,
        short_text(
            user_card
                .focus_summary
                .as_deref()
                .unwrap_or(user_card.header.as_str()),
            48
        )
    )
}

fn render_active_sources(
    snapshot: &ActivityCardsSnapshot,
    user_card: &UserActivityCardView,
) -> Option<String> {
    let summaries = snapshot
        .source_cards
        .iter()
        .filter(|card| Some(card.source_id.as_str()) != user_card.focus_source_id.as_deref())
        .filter(|card| !matches!(card.state.as_str(), "idle"))
        .take(MAX_ACTIVE_SOURCES)
        .map(|card| {
            format!(
                "{} {}{}",
                short_text(card.title.as_str(), 14),
                short_text(card.state.as_str(), 10),
                if card.auto_promoted { "*" } else { "" }
            )
        })
        .collect::<Vec<_>>();
    if summaries.is_empty() {
        let has_focusless_secondary = snapshot.source_cards.iter().any(|card| {
            Some(card.source_id.as_str()) != user_card.focus_source_id.as_deref()
                && !matches!(card.state.as_str(), "idle")
        });
        if !has_focusless_secondary {
            return None;
        }
    }
    (!summaries.is_empty()).then(|| format!("👥 {}", summaries.join(" · ")))
}

fn render_stage_line(user_card: &UserActivityCardView) -> Option<String> {
    let stage = user_card
        .stage
        .as_deref()
        .or(user_card.focus_summary.as_deref())
        .unwrap_or("idle");
    let stage_humanized = humanize_stage(stage);
    if stage_humanized.trim().is_empty() {
        return None;
    }
    let recent = if same_semantic_text(stage_humanized.as_str(), PENDING_INBOUND_NOTICE) {
        Vec::new()
    } else {
        user_card
            .recent_items
            .as_slice()
            .iter()
            .map(|item| humanize_stage(item))
            .filter(|item| !same_semantic_text(item.as_str(), stage_humanized.as_str()))
            .filter(|item| !same_semantic_text(item.as_str(), PENDING_INBOUND_NOTICE))
            .filter(|item| !item.trim().is_empty())
            .take(2)
            .map(|item| short_text(item.as_str(), 28))
            .collect::<Vec<_>>()
    };
    let stage = short_text(stage_humanized.as_str(), 52);
    if recent.is_empty() {
        Some(format!("📍 {stage}"))
    } else {
        Some(format!(
            "📍 {} · 最近: {}",
            short_text(stage.as_str(), 32),
            recent.join(" | ")
        ))
    }
}

fn render_detail_line(user_card: &UserActivityCardView) -> Option<String> {
    let stage = user_card
        .stage
        .as_deref()
        .map(humanize_stage)
        .unwrap_or_default();
    user_card
        .failure_detail
        .as_deref()
        .and_then(|detail| {
            let humanized = humanize_stage(detail);
            (!same_semantic_text(humanized.as_str(), stage.as_str()))
                .then(|| format!("❌ {}", short_text(humanized.as_str(), 64)))
        })
        .or_else(|| {
            user_card.waiting_detail.as_deref().and_then(|detail| {
                let humanized = humanize_stage(detail);
                (!same_semantic_text(humanized.as_str(), stage.as_str()))
                    .then(|| format!("⏳ {}", short_text(humanized.as_str(), 64)))
            })
        })
}

fn render_source_supplement(source: &SourceActivityCardView) -> Option<String> {
    if !source.auto_promoted && source.visibility.as_str() == "compact" {
        return None;
    }
    let activity = humanize_stage(
        source
            .current_activity
            .as_deref()
            .unwrap_or(source.summary.as_str()),
    );
    let summary = humanize_stage(source.summary.as_str());
    if same_semantic_text(activity.as_str(), summary.as_str()) {
        return None;
    }
    Some(format!(
        "🧩 {} · {}",
        short_text(source.title.as_str(), 16),
        short_text(activity.as_str(), 40)
    ))
}

fn render_focus_action_lines(
    source: &SourceActivityCardView,
    user_card: &UserActivityCardView,
) -> Vec<String> {
    let stage = user_card
        .stage
        .as_deref()
        .map(humanize_stage)
        .unwrap_or_default();
    source
        .recent_actions
        .iter()
        .take(2)
        .map(render_recent_action)
        .filter(|item| !same_semantic_text(item.as_str(), stage.as_str()))
        .map(|action| format!("✅ {}", short_text(action.as_str(), 48)))
        .collect()
}

fn humanize_stage(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed == "active closure running" {
        "正在处理输入".into()
    } else if trimmed == "closure completed" {
        "本轮处理完成".into()
    } else if trimmed.contains("phase=inference_completed") && trimmed.contains("render_projection")
    {
        "本轮推理完成，正在整理结果".into()
    } else if trimmed.contains("phase=inference_completed")
        && trimmed.contains("continue_reasoning")
    {
        "本轮推理完成，准备继续下一步".into()
    } else if trimmed.contains("phase=") && trimmed.contains("waiting_model") {
        "等待模型响应".into()
    } else if trimmed.contains("phase=") && trimmed.contains("tool_result") {
        "等待工具结果".into()
    } else if trimmed.starts_with("bound to session ") {
        format!(
            "已绑定会话 {}",
            trimmed.trim_start_matches("bound to session ")
        )
    } else if trimmed == "pairing required" {
        "等待重新配对".into()
    } else if trimmed == "binding invalidated" {
        "绑定已失效".into()
    } else {
        trimmed.to_string()
    }
}

fn render_recent_action(action: &fin_contracts::ToolSemanticView) -> String {
    let detail = action
        .detail
        .as_deref()
        .unwrap_or(action.object_label.as_str());
    match action.category.as_str() {
        "search" => format!("搜索: {}", short_text(detail, 40)),
        "read" => format!("查看: {}", short_text(detail, 40)),
        "write" => format!("修改: {}", short_text(detail, 40)),
        "plan" => format!("计划: {}", short_text(detail, 40)),
        "command" => format!("命令: {}", short_text(detail, 40)),
        "reasoning" => "推理: 本轮已停止".into(),
        "model" | _ if action.tool_name == "provider.call" => {
            format!("模型: {}", short_text(action.object_label.as_str(), 40))
        }
        _ => short_text(action.summary.as_str(), 40),
    }
}

fn short_text(value: &str, limit: usize) -> String {
    let trimmed = value.trim();
    let mut chars = trimmed.chars();
    let shortened = chars.by_ref().take(limit).collect::<String>();
    if chars.next().is_some() {
        format!("{shortened}…")
    } else if shortened.is_empty() {
        "-".into()
    } else {
        shortened
    }
}

fn push_unique_line(lines: &mut Vec<String>, candidate: String) {
    let trimmed = candidate.trim();
    if trimmed.is_empty() {
        return;
    }
    if lines
        .iter()
        .any(|existing| same_semantic_text(existing.as_str(), trimmed))
    {
        return;
    }
    lines.push(trimmed.to_string());
}

fn same_semantic_text(left: &str, right: &str) -> bool {
    normalize_semantic_text(left) == normalize_semantic_text(right)
}

fn normalize_semantic_text(value: &str) -> String {
    value
        .replace("🌐", "")
        .replace("👥", "")
        .replace("📍", "")
        .replace("🧩", "")
        .replace("✅", "")
        .replace("❌", "")
        .replace("⏳", "")
        .replace("🔄", "")
        .replace("⏸", "")
        .replace("💤", "")
        .replace("Global status", "")
        .replace("System Agent", "")
        .replace(" · 进行中", "")
        .replace("最近:", "")
        .replace('|', " ")
        .replace('*', "")
        .split_whitespace()
        .collect::<String>()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::channel_peer::complete_builtin_qqbot_pairing;
    use fin_contracts::{
        ActivitySourceSummary, SourceActivityCardView, ToolSemanticView, UserActivityCardView,
    };
    use std::{
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

        let signature =
            current_delivery_signature_if_deliverable(&runtime_home).expect("signature");
        assert!(signature.is_some());
    }
}
