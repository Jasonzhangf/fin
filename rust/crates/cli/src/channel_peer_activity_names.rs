use fin_contracts::{ActivityCardsSnapshot, SourceActivityCardView};

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
    let title = card.title.trim();
    if title.is_empty() {
        return card.source_id.clone();
    }
    match card.source_kind.as_str() {
        "system_agent" | "system_entry" => "system".into(),
        "system_worker" => title
            .strip_prefix("System Worker ")
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(normalize_system_worker_name)
            .unwrap_or_else(|| title.to_string()),
        "project_agent" | "project_worker" => {
            execution_source_identity(snapshot, card).unwrap_or_else(|| title.to_string())
        }
        _ => title.to_string(),
    }
}

fn execution_source_identity(
    snapshot: &ActivityCardsSnapshot,
    card: &SourceActivityCardView,
) -> Option<String> {
    let (device, agent) = split_agent_identity(card.source_id.as_str())?;
    if execution_device_count(snapshot) > 1 {
        return Some(format!("{device}.{agent}"));
    }
    Some(agent.to_string())
}

fn execution_device_count(snapshot: &ActivityCardsSnapshot) -> usize {
    snapshot
        .source_cards
        .iter()
        .filter(|card| {
            matches!(
                card.source_kind.as_str(),
                "project_agent" | "project_worker"
            )
        })
        .filter_map(|card| split_agent_identity(card.source_id.as_str()).map(|(device, _)| device))
        .collect::<std::collections::BTreeSet<_>>()
        .len()
}

fn split_agent_identity(source_id: &str) -> Option<(&str, &str)> {
    let (device, agent) = source_id.split_once('.')?;
    let device = device.trim();
    let agent = agent.trim();
    if device.is_empty() || agent.is_empty() {
        return None;
    }
    Some((device, agent))
}

fn normalize_system_worker_name(value: &str) -> String {
    value
        .strip_prefix("system-worker-")
        .map(|suffix| format!("worker-{suffix}"))
        .unwrap_or_else(|| value.to_string())
}

fn concise_peer_name(card: &SourceActivityCardView) -> String {
    let title = card.title.trim();
    if !title.is_empty() {
        return title.to_string();
    }
    card.source_id.clone()
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
