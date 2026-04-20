use fin_contracts::ToolSemanticView;

pub(super) fn most_recent_actions(
    semantics: &[ToolSemanticView],
    limit: usize,
) -> Vec<ToolSemanticView> {
    let mut sorted = semantics.to_vec();
    sorted.sort_by(|left, right| {
        left.started_at
            .cmp(&right.started_at)
            .then_with(|| left.tool_call_id.cmp(&right.tool_call_id))
    });
    let reversed = sorted.into_iter().rev().collect::<Vec<_>>();
    let non_provider = reversed
        .iter()
        .filter(|item| item.tool_name != "provider.call")
        .cloned()
        .take(limit)
        .collect::<Vec<_>>();
    if !non_provider.is_empty() {
        return non_provider;
    }
    reversed.into_iter().take(limit).collect()
}

pub(super) fn latest_failed_action(semantics: &[ToolSemanticView]) -> Option<ToolSemanticView> {
    semantics
        .iter()
        .rev()
        .find(|item| item.status == "failed")
        .cloned()
}

pub(super) fn source_rank(state: &str) -> u8 {
    match state {
        "failed" => 0,
        "running" => 1,
        "waiting" | "paused" => 2,
        "degraded" => 3,
        _ => 4,
    }
}

pub(super) fn visibility_for_state(state: &str) -> &'static str {
    match state {
        "running" | "failed" | "waiting" | "paused" => "detailed",
        _ => "compact",
    }
}

pub(super) fn should_promote(
    state: &str,
    failure_detail: Option<&str>,
    waiting_detail: Option<&str>,
) -> bool {
    failure_detail.is_some()
        || waiting_detail.is_some()
        || matches!(state, "running" | "failed" | "waiting" | "paused")
}

pub(super) fn peer_title(peer: &super::PeerRegistryEntry) -> String {
    match peer.peer_kind.as_str() {
        "channel_gateway.qqbot" => "QQ Channel Peer".into(),
        other => format!("Peer {other}"),
    }
}

pub(super) fn peer_state(peer: &super::PeerRegistryEntry) -> String {
    if peer.connectivity_state.as_deref() == Some("degraded")
        || peer.connectivity_state.as_deref() == Some("failed")
    {
        "failed".into()
    } else if peer.binding_state.as_deref() == Some("invalidated") {
        "waiting".into()
    } else if peer.presence_state == "online" {
        "ready".into()
    } else {
        peer.presence_state.clone()
    }
}

pub(super) fn peer_summary(peer: &super::PeerRegistryEntry) -> String {
    let mut parts = vec![format!("presence {}", peer.presence_state)];
    if let Some(connectivity) = peer
        .connectivity_state
        .as_deref()
        .filter(|value| !value.is_empty())
    {
        parts.push(format!("connectivity {connectivity}"));
    }
    if let Some(binding) = peer
        .binding_state
        .as_deref()
        .filter(|value| !value.is_empty())
    {
        parts.push(format!("binding {binding}"));
    }
    if peer.pairing_required == Some(true) {
        parts.push("pairing required".into());
    }
    shorten(&parts.join(" · "), 120)
}

pub(super) fn peer_activity(peer: &super::PeerRegistryEntry) -> String {
    if peer.binding_state.as_deref() == Some("invalidated") {
        "binding invalidated".into()
    } else if peer.session_valid == Some(true) {
        let session_id = peer.session_id.as_deref().unwrap_or("-");
        format!("bound to session {session_id}")
    } else if peer.binding_state.as_deref() == Some("unbound") {
        "waiting inbound session restore".into()
    } else if peer.session_valid == Some(false) {
        "session released".into()
    } else {
        "idle".into()
    }
}

pub(super) fn shorten(value: &str, limit: usize) -> String {
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

pub(super) fn local_now_fallback() -> String {
    "local-now".into()
}
