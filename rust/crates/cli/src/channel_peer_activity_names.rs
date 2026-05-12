use fin_contracts::{ActivityCardsSnapshot, SourceActivityCardView};
use std::collections::BTreeSet;

pub(crate) fn concise_source_name(
    snapshot: &ActivityCardsSnapshot,
    card: &SourceActivityCardView,
) -> String {
    if is_system_display(card) {
        return "system".into();
    }
    if is_execution_source(card) {
        return execution_display_name(snapshot, card);
    }
    if is_qq_gateway(card) {
        return "QQ".into();
    }
    if is_external_peer(card) {
        return concise_peer_name(card);
    }
    card.title.clone()
}

pub(crate) fn concise_resource_name(
    snapshot: &ActivityCardsSnapshot,
    card: &SourceActivityCardView,
) -> String {
    if is_external_peer(card) {
        return concise_peer_name(card);
    }
    concise_source_name(snapshot, card)
}

fn execution_display_name(
    snapshot: &ActivityCardsSnapshot,
    card: &SourceActivityCardView,
) -> String {
    let Some((device, agent)) = split_agent_identity(card.source_id.as_str()) else {
        return card.title.clone();
    };
    let display_agent = concise_agent_name(agent);
    if execution_device_count(snapshot) <= 1 {
        display_agent
    } else {
        format!("{device}.{}", display_agent)
    }
}

fn concise_agent_name(agent: &str) -> String {
    if agent == "system" {
        return "system".into();
    }
    if let Some(suffix) = agent.strip_prefix("system-worker-") {
        return format!("worker-{suffix}");
    }
    agent.to_string()
}

fn concise_peer_name(card: &SourceActivityCardView) -> String {
    let title = card.title.trim();
    if !title.is_empty() {
        return title.to_string();
    }
    card.source_id.clone()
}

fn execution_device_count(snapshot: &ActivityCardsSnapshot) -> usize {
    snapshot
        .source_cards
        .iter()
        .filter(|card| is_execution_source(card))
        .filter_map(|card| split_agent_identity(card.source_id.as_str()).map(|(device, _)| device))
        .collect::<BTreeSet<_>>()
        .len()
}

fn split_agent_identity(source_id: &str) -> Option<(&str, &str)> {
    let (device, agent) = source_id.split_once('.')?;
    if device.trim().is_empty() || agent.trim().is_empty() {
        return None;
    }
    Some((device, agent))
}

fn is_execution_source(card: &SourceActivityCardView) -> bool {
    matches!(
        card.source_kind.as_str(),
        "system_agent" | "system_entry" | "system_worker" | "project_agent" | "project_worker"
    )
}

fn is_external_peer(card: &SourceActivityCardView) -> bool {
    !is_execution_source(card)
        && !matches!(card.source_kind.as_str(), "system_agent")
        && !is_qq_gateway(card)
}

fn is_qq_gateway(card: &SourceActivityCardView) -> bool {
    card.source_kind == "channel_gateway.qqbot"
}

fn is_system_display(card: &SourceActivityCardView) -> bool {
    card.source_id == "system-agent"
        || matches!(card.source_kind.as_str(), "system_agent" | "system_entry")
            && matches!(card.title.trim(), "" | "System Agent")
}
