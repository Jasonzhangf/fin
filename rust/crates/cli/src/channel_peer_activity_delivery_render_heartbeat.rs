use super::{
    PENDING_INBOUND_NOTICE, humanize_stage, latest_actions, normalize_action_fingerprint,
    push_unique_line, render_debug_line, render_recent_action, short_text,
};
use crate::channel_peer_activity_names::concise_source_name;
use crate::channel_peer_progress_policy::QqbotProgressPolicy;
use chrono::{DateTime, FixedOffset};
use fin_contracts::{ActivityCardsSnapshot, UserActivityCardView};
use std::collections::BTreeSet;

#[path = "channel_peer_activity_delivery_render_dev.rs"]
mod render_dev;
#[path = "channel_peer_activity_delivery_render_resources.rs"]
mod render_resources;
use render_dev::{render_dev_agent_lines, render_dev_tool_lines};
use render_resources::render_resource_summary;

pub(super) fn render_heartbeat_text(
    snapshot: &ActivityCardsSnapshot,
    debug_enabled: bool,
    policy: &QqbotProgressPolicy,
) -> String {
    let Some(user_card) = snapshot.user_card.as_ref() else {
        return String::new();
    };
    let waiting_reason = render_heartbeat_waiting_reason(snapshot, user_card);
    let action_lines = render_heartbeat_action_lines(snapshot, user_card, 2);
    if action_lines.is_empty()
        && waiting_reason
            .as_deref()
            .is_none_or(is_low_signal_heartbeat_waiting_line)
    {
        return String::new();
    }
    let mut lines = vec![render_heartbeat_header(snapshot, user_card)];
    if debug_enabled {
        lines.push(render_debug_line(snapshot, user_card));
    }
    if policy.show_resource_summary {
        for resource_summary in render_resource_summary(snapshot, policy) {
            push_unique_line(&mut lines, resource_summary);
        }
    }
    if let Some(waiting_reason) = waiting_reason {
        push_unique_line(&mut lines, waiting_reason);
    }
    if policy.show_tool_summary {
        for line in action_lines {
            push_unique_line(&mut lines, line);
        }
    }
    if debug_enabled {
        for line in render_dev_agent_lines(snapshot) {
            push_unique_line(&mut lines, line);
        }
        if policy.show_tool_summary {
            for line in render_dev_tool_lines(snapshot, 4) {
                push_unique_line(&mut lines, line);
            }
        }
    }
    lines.retain(|line| !line.trim().is_empty());
    lines.join("\n")
}

fn render_heartbeat_header(
    snapshot: &ActivityCardsSnapshot,
    user_card: &UserActivityCardView,
) -> String {
    let focus = snapshot
        .source_cards
        .iter()
        .find(|card| Some(card.source_id.as_str()) == user_card.focus_source_id.as_deref())
        .map(|card| concise_source_name(snapshot, card))
        .unwrap_or_else(|| "system".into());
    let state = match user_card.state.as_str() {
        "running" => "🔄 执行中",
        "waiting" => "⏳ 等待中",
        "paused" => "⏸ 已暂停",
        "failed" => "❌ 失败",
        "ready" => "✅ 就绪",
        "idle" => "💤 空闲",
        other => other,
    };
    let elapsed = heartbeat_elapsed(snapshot, user_card)
        .map(|value| format!(" · 已持续 {value}"))
        .unwrap_or_default();
    format!(
        "⏱ 进度心跳 · {} · {}{}",
        short_text(focus.as_str(), 24),
        state,
        elapsed
    )
}

fn render_heartbeat_waiting_reason(
    snapshot: &ActivityCardsSnapshot,
    user_card: &UserActivityCardView,
) -> Option<String> {
    let focus_source = snapshot
        .source_cards
        .iter()
        .find(|card| Some(card.source_id.as_str()) == user_card.focus_source_id.as_deref())
        .or_else(|| snapshot.source_cards.first());
    let waiting_reason = user_card
        .waiting_detail
        .as_deref()
        .or_else(|| focus_source.and_then(|card| card.waiting_detail.as_deref()))
        .or_else(|| user_card.stage.as_deref())
        .or_else(|| user_card.focus_summary.as_deref())?;
    let humanized = humanize_stage(waiting_reason);
    if humanized.trim().is_empty() {
        return None;
    }
    Some(format!(
        "📍 当前等待: {}",
        short_text(humanized.as_str(), 180)
    ))
}

fn render_heartbeat_action_lines(
    snapshot: &ActivityCardsSnapshot,
    user_card: &UserActivityCardView,
    max_lines: usize,
) -> Vec<String> {
    let focus_source = snapshot
        .source_cards
        .iter()
        .find(|card| Some(card.source_id.as_str()) == user_card.focus_source_id.as_deref())
        .or_else(|| snapshot.source_cards.first());
    let source_actions = focus_source
        .map(|value| value.recent_actions.as_slice())
        .unwrap_or_default();
    let actions = if source_actions.is_empty() {
        snapshot.tool_semantics.as_slice()
    } else {
        source_actions
    };
    let mut seen = BTreeSet::new();
    latest_actions(actions)
        .map(render_recent_action)
        .filter(|value| !value.trim().is_empty())
        .filter(|value| !is_low_signal_action(value))
        .filter(|value| seen.insert(normalize_action_fingerprint(value.as_str())))
        .take(max_lines)
        .map(|value| format!("🛠 {}", short_text(value.as_str(), 120)))
        .collect()
}

fn is_low_signal_heartbeat_waiting_line(value: &str) -> bool {
    let waiting = value.trim();
    waiting.is_empty()
        || waiting.contains(PENDING_INBOUND_NOTICE)
        || waiting.contains("当前等待: 已收到，正在处理")
}

fn is_low_signal_action(value: &str) -> bool {
    let action = value.trim();
    action.is_empty() || action == "-" || action.contains(PENDING_INBOUND_NOTICE)
}

fn heartbeat_elapsed(
    snapshot: &ActivityCardsSnapshot,
    user_card: &UserActivityCardView,
) -> Option<String> {
    let since = snapshot
        .source_cards
        .iter()
        .find(|card| Some(card.source_id.as_str()) == user_card.focus_source_id.as_deref())
        .map(|card| card.updated_at.as_str())
        .or(user_card
            .waiting_detail
            .as_deref()
            .map(|_| user_card.updated_at.as_str()))
        .unwrap_or(user_card.updated_at.as_str());
    let generated_at = parse_local_ts(snapshot.generated_at.as_str())?;
    let since = parse_local_ts(since)?;
    let seconds = (generated_at - since).num_seconds().max(0);
    Some(format_elapsed_seconds(seconds))
}

fn parse_local_ts(value: &str) -> Option<DateTime<FixedOffset>> {
    DateTime::parse_from_rfc3339(value).ok()
}

fn format_elapsed_seconds(seconds: i64) -> String {
    if seconds >= 3600 {
        let hours = seconds / 3600;
        let minutes = (seconds % 3600) / 60;
        if minutes > 0 {
            format!("{hours}h{minutes}m")
        } else {
            format!("{hours}h")
        }
    } else if seconds >= 60 {
        let minutes = seconds / 60;
        let rem = seconds % 60;
        if rem > 0 {
            format!("{minutes}m{rem}s")
        } else {
            format!("{minutes}m")
        }
    } else {
        format!("{seconds}s")
    }
}
