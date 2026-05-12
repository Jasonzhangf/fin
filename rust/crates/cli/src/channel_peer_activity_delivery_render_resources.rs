use crate::channel_peer_activity_names::concise_resource_name;
use crate::channel_peer_progress_policy::QqbotProgressPolicy;
use fin_contracts::{ActivityCardsSnapshot, SourceActivityCardView};

pub(super) fn render_resource_summary(
    snapshot: &ActivityCardsSnapshot,
    policy: &QqbotProgressPolicy,
) -> Vec<String> {
    let resources = snapshot
        .source_cards
        .iter()
        .filter(|card| !card.source_kind.starts_with("channel_gateway."))
        .collect::<Vec<_>>();
    if resources.is_empty() {
        return Vec::new();
    }

    let running = names_for_state(snapshot, &resources, &["running"]);
    let waiting = names_for_state(snapshot, &resources, &["waiting", "paused"]);
    let failed = names_for_state(snapshot, &resources, &["failed"]);
    let idle = resources
        .iter()
        .filter(|card| {
            !matches!(
                card.state.as_str(),
                "running" | "waiting" | "paused" | "failed"
            )
        })
        .map(|card| concise_resource_name(snapshot, card))
        .collect::<Vec<_>>();

    let mut lines = vec![format!(
        "🧮 资源 · 总{} · 运行{} · 等待{} · 空闲{}{}",
        resources.len(),
        running.len(),
        waiting.len(),
        idle.len(),
        if failed.is_empty() {
            String::new()
        } else {
            format!(" · 失败{}", failed.len())
        }
    )];

    if policy.detail_level != "compact" {
        let mut groups = Vec::new();
        if !running.is_empty() {
            groups.push(format!("运行: {}", preview_names(&running)));
        }
        if !waiting.is_empty() {
            groups.push(format!("等待: {}", preview_names(&waiting)));
        }
        if !failed.is_empty() {
            groups.push(format!("失败: {}", preview_names(&failed)));
        }
        if !idle.is_empty() {
            groups.push(format!("空闲: {}", preview_names(&idle)));
        }
        if !groups.is_empty() {
            lines.push(format!("👤 {}", groups.join(" · ")));
        }
    }

    lines
}

fn names_for_state(
    snapshot: &ActivityCardsSnapshot,
    resources: &[&SourceActivityCardView],
    states: &[&str],
) -> Vec<String> {
    resources
        .iter()
        .filter(|card| states.iter().any(|state| **state == card.state))
        .map(|card| concise_resource_name(snapshot, card))
        .collect::<Vec<_>>()
}

fn preview_names(items: &[String]) -> String {
    items
        .iter()
        .map(|name| {
            let trimmed = name.trim();
            if trimmed.is_empty() { "-" } else { trimmed }
        })
        .collect::<Vec<_>>()
        .join(", ")
}
