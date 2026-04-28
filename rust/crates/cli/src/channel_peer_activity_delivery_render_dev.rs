use super::{
    humanize_stage, latest_actions, normalize_action_fingerprint, render_recent_action, short_text,
};
use crate::channel_peer_activity_names::concise_source_name;
use fin_contracts::{ActivityCardsSnapshot, SourceActivityCardView, ToolSemanticView};
use std::collections::BTreeSet;

pub(super) fn render_dev_agent_lines(snapshot: &ActivityCardsSnapshot) -> Vec<String> {
    snapshot
        .source_cards
        .iter()
        .filter(|card| is_execution_source(card))
        .map(|card| render_dev_agent_line(snapshot, card))
        .collect()
}

pub(super) fn render_dev_tool_lines(snapshot: &ActivityCardsSnapshot, limit: usize) -> Vec<String> {
    let mut seen = BTreeSet::new();
    let mut lines = Vec::new();
    for action in latest_actions(snapshot.tool_semantics.as_slice()) {
        if should_suppress_user_visible_action(action) {
            continue;
        }
        let line = render_dev_tool_line(action);
        let fingerprint = normalize_action_fingerprint(line.as_str());
        if !seen.insert(fingerprint) {
            continue;
        }
        lines.push(line);
        if lines.len() >= limit {
            break;
        }
    }
    lines
}

fn is_execution_source(card: &SourceActivityCardView) -> bool {
    matches!(
        card.source_kind.as_str(),
        "system_agent" | "system_worker" | "project_agent" | "project_worker"
    )
}

fn render_dev_agent_line(
    snapshot: &ActivityCardsSnapshot,
    card: &SourceActivityCardView,
) -> String {
    let state = humanize_state(card.state.as_str());
    let busy = busy_degree(card.state.as_str());
    let display_name = concise_source_name(snapshot, card);
    let mut parts = vec![format!(
        "🤖 {} · {}({})",
        short_text(display_name.as_str(), 22),
        state,
        busy
    )];
    if let Some(focus) = card
        .focus_label
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    {
        parts.push(short_text(focus, 28));
    }
    let activity = card
        .current_activity
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .map(humanize_stage)
        .unwrap_or_else(|| humanize_stage(card.summary.as_str()));
    if !activity.trim().is_empty() && activity != display_name {
        parts.push(short_text(activity.as_str(), 56));
    }
    let summary = humanize_stage(card.summary.as_str());
    if !summary.trim().is_empty() && summary != activity {
        parts.push(short_text(summary.as_str(), 44));
    }
    if let Some(detail) = card
        .failure_detail
        .as_deref()
        .or(card.waiting_detail.as_deref())
        .filter(|value| !value.trim().is_empty())
    {
        let detail = humanize_stage(detail);
        if detail != activity && detail != summary {
            parts.push(short_text(detail.as_str(), 44));
        }
    }
    let recent_actions = recent_action_snippets(card.recent_actions.as_slice());
    if !recent_actions.is_empty() {
        parts.push(format!("最近: {}", recent_actions.join(" | ")));
    }
    parts.join(" · ")
}

fn render_dev_tool_line(action: &ToolSemanticView) -> String {
    let rendered = render_recent_action(action);
    let prefix = if action.status == "failed" {
        "❌"
    } else {
        "🛠"
    };
    format!(
        "{prefix} [{}] {}",
        short_text(action.tool_name.as_str(), 24),
        short_text(rendered.as_str(), 96)
    )
}

fn should_suppress_user_visible_action(action: &ToolSemanticView) -> bool {
    if action.tool_name != "mailbox.poll" || action.status == "failed" {
        return false;
    }
    let detail = action
        .detail
        .as_deref()
        .unwrap_or(action.object_label.as_str());
    named_value(detail, "messages=").unwrap_or("0") == "0"
        && named_value(detail, "remaining=").unwrap_or("0") == "0"
        && named_value(detail, "ids=").is_none()
}

fn humanize_state(state: &str) -> &'static str {
    match state {
        "running" => "执行中",
        "waiting" => "等待中",
        "paused" => "已暂停",
        "failed" => "失败",
        "ready" => "就绪",
        "idle" => "空闲",
        _ => "未知",
    }
}

fn busy_degree(state: &str) -> &'static str {
    match state {
        "running" => "高",
        "waiting" | "paused" => "中",
        "failed" => "异常",
        _ => "低",
    }
}

fn recent_action_snippets(actions: &[ToolSemanticView]) -> Vec<String> {
    let mut seen = BTreeSet::new();
    let mut rendered = Vec::new();
    for action in actions {
        if should_suppress_user_visible_action(action) {
            continue;
        }
        let line = render_recent_action(action);
        let fingerprint = normalize_action_fingerprint(line.as_str());
        if !seen.insert(fingerprint) {
            continue;
        }
        rendered.push(short_text(line.as_str(), 30));
        if rendered.len() >= 2 {
            break;
        }
    }
    rendered
}

fn named_value<'a>(detail: &'a str, key: &str) -> Option<&'a str> {
    let tail = detail.split(key).nth(1)?;
    let end = tail.find([',', ';', ' ', '\n']).unwrap_or(tail.len());
    let value = tail[..end].trim();
    (!value.is_empty()).then_some(value)
}
