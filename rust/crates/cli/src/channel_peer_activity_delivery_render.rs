use super::{ActivityCardsSnapshot, MAX_ACTIVE_SOURCES, PENDING_INBOUND_NOTICE, SYSTEM_SOURCE_ID};
use fin_contracts::{SourceActivityCardView, ToolSemanticView, UserActivityCardView};

pub(super) fn render_compact_text(snapshot: &ActivityCardsSnapshot) -> String {
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
    lines.join(
        "
",
    )
}

pub(super) fn should_emit_snapshot(snapshot: &ActivityCardsSnapshot) -> bool {
    let Some(user_card) = snapshot.user_card.as_ref() else {
        return false;
    };
    if has_pending_inbound_notice(user_card) {
        return !is_ack_equivalent_snapshot(snapshot);
    }
    let Some(system_card) = snapshot
        .source_cards
        .iter()
        .find(|card| card.source_id == SYSTEM_SOURCE_ID)
    else {
        return false;
    };
    if is_ack_equivalent_snapshot(snapshot) {
        return false;
    }
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

fn is_ack_equivalent_snapshot(snapshot: &ActivityCardsSnapshot) -> bool {
    let system_card = snapshot
        .source_cards
        .iter()
        .find(|card| card.source_id == SYSTEM_SOURCE_ID);
    system_card.is_some_and(|card| {
        card.summary.as_str() == PENDING_INBOUND_NOTICE
            || card.waiting_detail.as_deref() == Some(PENDING_INBOUND_NOTICE)
            || card.current_activity.as_deref() == Some(PENDING_INBOUND_NOTICE)
    })
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

pub(super) fn render_recent_action(action: &ToolSemanticView) -> String {
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
