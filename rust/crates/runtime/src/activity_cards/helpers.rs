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
    let mut sorted = semantics.to_vec();
    sorted.sort_by(|left, right| {
        left.started_at
            .cmp(&right.started_at)
            .then_with(|| left.tool_call_id.cmp(&right.tool_call_id))
            .reverse()
    });
    sorted.into_iter().find(|candidate| {
        if candidate.status != "failed" {
            return false;
        }
        !semantics.iter().any(|other| {
            other.operation_id == candidate.operation_id
                && other.tool_name == candidate.tool_name
                && other.status != "failed"
                && (other.started_at > candidate.started_at
                    || (other.started_at == candidate.started_at
                        && other.tool_call_id > candidate.tool_call_id))
        })
    })
}

pub(super) fn frontstage_recent_item(action: &ToolSemanticView) -> String {
    if action.status == "failed" {
        return action
            .detail
            .as_deref()
            .map(preferred_failure_recent_detail)
            .map(|detail| format!("失败: {}", shorten(detail.as_str(), 72)))
            .unwrap_or_else(|| format!("失败: {}", shorten(action.summary.as_str(), 72)));
    }
    action.summary.clone()
}

pub(super) fn frontstage_source_recent_item(
    source_id: &str,
    state: &str,
    summary: &str,
    current_activity: Option<&str>,
    failure_detail: Option<&str>,
) -> String {
    let label = source_id.rsplit('.').next().unwrap_or(source_id);
    let detail = failure_detail
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| {
            current_activity
                .filter(|value| !value.trim().is_empty())
                .unwrap_or(summary)
        });
    format!("{label} · {} · {}", state_label(state), shorten(detail, 72))
}

fn preferred_failure_recent_detail(detail: &str) -> String {
    let trimmed = detail.trim();
    trimmed
        .split("重试：")
        .nth(1)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(trimmed)
        .to_string()
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

pub(super) fn peer_title(peer: &super::records::PeerRegistryEntry) -> String {
    if let Some(alias) = peer
        .alias
        .as_deref()
        .or(peer.label.as_deref())
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        return alias.to_string();
    }
    match peer.peer_kind.as_str() {
        "channel_gateway.qqbot" => "QQ Channel Peer".into(),
        other => format!("Peer {other}"),
    }
}

pub(super) fn peer_state(peer: &super::records::PeerRegistryEntry) -> String {
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

pub(super) fn peer_summary(peer: &super::records::PeerRegistryEntry) -> String {
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

pub(super) fn peer_activity(peer: &super::records::PeerRegistryEntry) -> String {
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

pub(super) fn agent_title(agent: &super::agents::AgentPresenceEntry) -> String {
    match agent.agent_kind.as_str() {
        "system_entry" => "System Agent".into(),
        "system_worker" => format!("System Worker {}", agent.agent_name),
        "project_agent" => agent
            .project_id
            .as_deref()
            .map(|project_id| format!("Project Agent {project_id}"))
            .unwrap_or_else(|| format!("Project Agent {}", agent.agent_name)),
        "project_worker" => agent
            .project_id
            .as_deref()
            .map(|project_id| format!("Worker {project_id}/{}", agent.agent_name))
            .unwrap_or_else(|| format!("Worker {}", agent.agent_name)),
        _ => format!("Agent {}", agent.agent_name),
    }
}

pub(super) fn agent_state(agent: &super::agents::AgentPresenceEntry) -> String {
    match agent.status.as_str() {
        "busy" => "running".into(),
        "offline" => "idle".into(),
        other => other.into(),
    }
}

pub(super) fn agent_summary(agent: &super::agents::AgentPresenceEntry) -> String {
    let mut parts = Vec::new();
    if let Some(project_id) = agent
        .project_id
        .as_deref()
        .filter(|value: &&str| !value.is_empty())
    {
        parts.push(format!("project {project_id}"));
    }
    if !agent.progress_summary.trim().is_empty() {
        parts.push(agent.progress_summary.clone());
    }
    if let Some(task_id) = agent
        .current_task_id
        .as_deref()
        .filter(|value: &&str| !value.is_empty())
    {
        parts.push(format!("task {task_id}"));
    }
    shorten(&parts.join(" · "), 120)
}

pub(super) fn agent_activity(agent: &super::agents::AgentPresenceEntry) -> String {
    if let Some(reason) = agent
        .waiting_reason
        .as_deref()
        .filter(|value: &&str| !value.is_empty())
    {
        return shorten(reason, 120);
    }
    if let Some(phase) = agent
        .current_phase
        .as_deref()
        .filter(|value: &&str| !value.is_empty())
    {
        if let Some(task_id) = agent
            .current_task_id
            .as_deref()
            .filter(|value: &&str| !value.is_empty())
        {
            return shorten(&format!("{phase} · task {task_id}"), 120);
        }
        return shorten(phase, 120);
    }
    if !agent.progress_summary.trim().is_empty() {
        return shorten(&agent.progress_summary, 120);
    }
    "idle".into()
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

fn state_label(state: &str) -> &'static str {
    match state {
        "running" => "执行中",
        "waiting" => "等待中",
        "paused" => "已暂停",
        "failed" => "失败",
        "ready" => "就绪",
        "idle" => "空闲",
        _ => "状态",
    }
}

pub(super) fn local_now_placeholder() -> String {
    "local-now".into()
}
