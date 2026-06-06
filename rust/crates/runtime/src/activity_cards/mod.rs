use crate::{RuntimeError, tools::tool_semantics};
mod agents;
mod helpers;
mod store;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod delivery_tests;
use fin_contracts::{
    ActivityCardsSnapshot, ActivitySourceSummary, ExecutionStateRecord, SourceActivityCardView,
    ToolExecutionRecord, ToolSemanticView, TurnRecord, UserActivityCardView,
};
use serde::Deserialize;
use std::{cmp::Reverse, path::Path};

use agents::{AgentPresenceRegistry, build_agent_cards};
use helpers::{
    frontstage_recent_item, frontstage_source_recent_item, latest_failed_action,
    local_now_placeholder, most_recent_actions, peer_activity, peer_state, peer_summary, peer_title,
    shorten, should_promote, source_rank, visibility_for_state,
};
use store::{
    pending_inbound_notice, read_json_if_exists, read_last_run_json, read_last_run_value,
    read_last_run_vec, read_session_vec, string_field,
};

const SYSTEM_SOURCE_ID: &str = "system-agent";
const PENDING_INBOUND_NOTICE: &str = "已收到，正在处理";

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
struct PeerRegistry {
    #[serde(default)]
    peers: Vec<PeerRegistryEntry>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
struct PeerRegistryEntry {
    peer_id: String,
    peer_kind: String,
    #[serde(default)]
    alias: Option<String>,
    #[serde(default)]
    label: Option<String>,
    presence_state: String,
    #[serde(default)]
    lifecycle_state: String,
    #[serde(default)]
    runtime_state: Option<String>,
    #[serde(default)]
    connectivity_state: Option<String>,
    #[serde(default)]
    binding_state: Option<String>,
    updated_at: String,
    #[serde(default)]
    pairing_required: Option<bool>,
    #[serde(default)]
    session_valid: Option<bool>,
    #[serde(default)]
    session_id: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
struct ChannelConversationRegistry {
    #[serde(default)]
    conversations: Vec<ChannelConversationRecord>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
struct ChannelConversationRecord {
    #[serde(default)]
    session_id: Option<String>,
    #[serde(default)]
    last_inbound_message_id: Option<String>,
    #[serde(default)]
    last_inbound_at: Option<String>,
    #[serde(default)]
    last_delivered_message_id: Option<String>,
    #[serde(default)]
    last_delivery_at: Option<String>,
    #[serde(default)]
    status: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
struct StartupControlSummaryRecord {
    #[serde(default)]
    startup_config_summary: String,
    #[serde(default)]
    startup_state_summary: String,
    #[serde(default)]
    started_resource_count: usize,
    #[serde(default)]
    busy_resource_count: usize,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
struct OwnerLoopActionRecord {
    #[serde(default)]
    action_kind: String,
    #[serde(default)]
    reason: String,
    #[serde(default)]
    active_task_id: Option<String>,
    #[serde(default)]
    target_task_ids: Vec<String>,
}

pub fn build_activity_cards(runtime_home: &Path) -> Result<ActivityCardsSnapshot, RuntimeError> {
    build_activity_cards_for_session(runtime_home, None)
}

pub fn build_activity_cards_for_session(
    runtime_home: &Path,
    preferred_session_id: Option<&str>,
) -> Result<ActivityCardsSnapshot, RuntimeError> {
    let last_run = read_last_run_json(runtime_home)?;
    let last_run_session_id = string_field(&last_run, "session_id");
    let requested_session_id = preferred_session_id
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    let session_id = requested_session_id
        .clone()
        .or_else(|| last_run_session_id.clone());
    let generated_at = string_field(&last_run, "submitted_at")
        .or_else(|| string_field(&last_run, "occurred_at"))
        .unwrap_or_else(local_now_placeholder);
    let startup_summary = read_json_if_exists::<StartupControlSummaryRecord>(
        &runtime_home.join("runtime/current/current_startup_control_summary.json"),
    )?;
    let peer_registry =
        read_json_if_exists::<PeerRegistry>(&runtime_home.join("runtime/peers/registry.json"))?
            .unwrap_or_default();
    let agent_presence = read_json_if_exists::<AgentPresenceRegistry>(
        &runtime_home.join("runtime/current/current_agent_presence_registry.json"),
    )?
    .or(read_json_if_exists::<AgentPresenceRegistry>(
        &runtime_home.join("runtime/agents/presence_registry.json"),
    )?)
    .unwrap_or_default();
    let pending_inbound_notice = pending_inbound_notice(runtime_home, session_id.as_deref())?;
    let owner_loop_action = read_json_if_exists::<OwnerLoopActionRecord>(
        &runtime_home.join("runtime/current/current_owner_loop_action.json"),
    )?;
    let task_id = infer_task_id(
        session_id.as_deref(),
        last_run_session_id.as_deref(),
        &last_run,
        agent_presence.agents.as_slice(),
    );
    let mut tool_records = if session_id.as_deref() == last_run_session_id.as_deref() {
        read_last_run_vec::<ToolExecutionRecord>(runtime_home, "session_recent_tool_records_path")?
    } else if let Some(session_id) = session_id.as_deref() {
        read_session_vec(runtime_home, session_id, "tools/recent_tool_records.json")?
    } else {
        Vec::new()
    };
    let turns = if session_id.as_deref() == last_run_session_id.as_deref() {
        read_last_run_vec::<TurnRecord>(runtime_home, "session_recent_turns_path")?
    } else if let Some(session_id) = session_id.as_deref() {
        read_session_vec(runtime_home, session_id, "turns/recent_turns.json")?
    } else {
        Vec::new()
    };
    let execution_state = if session_id.as_deref() == last_run_session_id.as_deref() {
        read_last_run_value::<ExecutionStateRecord>(runtime_home, "current_execution_state_path")?
    } else {
        None
    };
    let current_operation_id = current_operation_scope(
        execution_state.as_ref(),
        turns.as_slice(),
        &last_run,
        session_id.as_deref(),
        last_run_session_id.as_deref(),
    );
    if let Some(operation_id) = current_operation_id.as_deref() {
        tool_records.retain(|record| record.operation_id == operation_id);
    }

    let semantics = tool_semantics::semantic_views(&tool_records);
    let mut system_card = build_system_card(
        session_id.as_deref(),
        task_id.as_deref(),
        execution_state.as_ref(),
        startup_summary.as_ref(),
        semantics.as_slice(),
        turns.as_slice(),
        generated_at.as_str(),
    );
    let peer_cards = build_peer_cards(
        peer_registry.peers.as_slice(),
        session_id.as_deref(),
        task_id.as_deref(),
    );
    let agent_cards = build_agent_cards(
        agent_presence.agents.as_slice(),
        session_id.as_deref(),
        task_id.as_deref(),
    );
    apply_dispatch_frontstage_overlay(
        &mut system_card,
        agent_cards.as_slice(),
        owner_loop_action.as_ref(),
    );

    let mut source_cards = Vec::with_capacity(1 + peer_cards.len() + agent_cards.len());
    source_cards.push(system_card);
    source_cards.extend(peer_cards);
    source_cards.extend(agent_cards);
    source_cards.sort_by_key(|card| {
        (
            Reverse(card.auto_promoted),
            source_rank(card.state.as_str()),
            card.title.clone(),
        )
    });

    let mut snapshot = ActivityCardsSnapshot {
        session_id,
        task_id,
        generated_at: generated_at.clone(),
        user_card: Some(build_user_card(
            source_cards.as_slice(),
            generated_at.as_str(),
        )),
        source_cards,
        tool_semantics: semantics,
    };
    if let (Some(user_card), Some(notice)) = (
        snapshot.user_card.as_mut(),
        pending_inbound_notice.as_deref(),
    ) {
        user_card.state = "waiting".into();
        user_card.focus_summary = Some(notice.to_string());
        user_card.stage = Some(notice.to_string());
        user_card.waiting_detail = Some(notice.to_string());
        user_card.failure_detail = None;
        if user_card
            .recent_items
            .first()
            .is_none_or(|item| item.as_str() != notice)
        {
            user_card.recent_items.insert(0, notice.to_string());
        }
        if let Some(system_card) = snapshot
            .source_cards
            .iter_mut()
            .find(|card| card.source_id == SYSTEM_SOURCE_ID)
        {
            system_card.state = "waiting".into();
            system_card.summary = notice.to_string();
            system_card.current_activity = Some(notice.to_string());
            system_card.waiting_detail = Some(notice.to_string());
            system_card.failure_detail = None;
            system_card.recent_actions.clear();
            system_card.auto_promoted = true;
        }
    }
    Ok(snapshot)
}

fn current_operation_scope(
    _execution_state: Option<&ExecutionStateRecord>,
    turns: &[TurnRecord],
    last_run: &serde_json::Value,
    session_id: Option<&str>,
    last_run_session_id: Option<&str>,
) -> Option<String> {
    turns
        .last()
        .and_then(|turn| (!turn.operation_id.trim().is_empty()).then(|| turn.operation_id.clone()))
        .or_else(|| {
            (session_id == last_run_session_id)
                .then(|| string_field(last_run, "operation_id"))
                .flatten()
        })
}

fn infer_task_id(
    session_id: Option<&str>,
    last_run_session_id: Option<&str>,
    last_run: &serde_json::Value,
    agents: &[agents::AgentPresenceEntry],
) -> Option<String> {
    if session_id == last_run_session_id {
        return string_field(last_run, "task_id");
    }
    let session_id = session_id?;
    agents
        .iter()
        .filter(|agent| agent.current_session_id.as_deref() == Some(session_id))
        .filter_map(|agent| {
            agent
                .current_task_id
                .as_ref()
                .map(|task_id| (agent.updated_at.as_str(), task_id.clone()))
        })
        .max_by(|left, right| left.0.cmp(right.0))
        .map(|(_, task_id)| task_id)
}

fn build_system_card(
    session_id: Option<&str>,
    task_id: Option<&str>,
    execution_state: Option<&ExecutionStateRecord>,
    startup_summary: Option<&StartupControlSummaryRecord>,
    semantics: &[ToolSemanticView],
    turns: &[TurnRecord],
    generated_at: &str,
) -> SourceActivityCardView {
    let recent_actions = most_recent_actions(semantics, 3);
    let latest_turn = turns.last();
    let startup_only_snapshot =
        execution_state.is_none() && latest_turn.is_none() && recent_actions.is_empty();
    let state = execution_state
        .map(|value| value.status.clone())
        .or_else(|| latest_turn.map(|turn| turn.status.clone()))
        .or_else(|| {
            startup_summary.map(|summary| {
                if summary.busy_resource_count > 0 {
                    "running".into()
                } else if summary.started_resource_count > 0 {
                    "ready".into()
                } else {
                    "idle".into()
                }
            })
        })
        .unwrap_or_else(|| "idle".into());
    let execution_reason = execution_state
        .and_then(|state| state.reason.as_ref())
        .filter(|reason| !reason.trim().is_empty())
        .cloned();
    let waiting_detail = execution_state
        .and_then(|state| state.reason.as_ref())
        .filter(|_| matches!(state.as_str(), "waiting" | "paused"))
        .cloned();
    let failure_detail = latest_failed_action(semantics)
        .map(|value| value.detail.unwrap_or(value.summary))
        .or_else(|| {
            latest_turn
                .filter(|turn| turn.status == "failed")
                .and_then(|turn| turn.progress_summary.clone())
        });
    let summary = if startup_only_snapshot {
        startup_summary
            .and_then(|item| (!item.startup_config_summary.trim().is_empty()).then_some(item))
            .map(|item| item.startup_config_summary.clone())
            .unwrap_or_else(|| state.clone())
    } else if matches!(state.as_str(), "running" | "waiting" | "paused") {
        execution_reason
            .clone()
            .or_else(|| {
                latest_turn.and_then(|turn| {
                    turn.progress_summary
                        .clone()
                        .or_else(|| turn.assistant_visible_output.clone())
                })
            })
            .or_else(|| recent_actions.first().map(|action| action.summary.clone()))
            .unwrap_or_else(|| state.clone())
    } else if let Some(action) = recent_actions.first() {
        action.summary.clone()
    } else if let Some(turn) = latest_turn {
        turn.progress_summary
            .clone()
            .or_else(|| turn.assistant_visible_output.clone())
            .unwrap_or_else(|| "no recent activity".into())
    } else {
        "idle".into()
    };
    let current_activity = if startup_only_snapshot {
        startup_summary
            .and_then(|item| (!item.startup_state_summary.trim().is_empty()).then_some(item))
            .map(|item| item.startup_state_summary.clone())
    } else if matches!(state.as_str(), "running" | "waiting" | "paused") {
        execution_reason.clone().or_else(|| {
            latest_turn.and_then(|turn| {
                turn.progress_summary
                    .clone()
                    .or_else(|| turn.assistant_visible_output.clone())
            })
        })
    } else {
        latest_turn
            .and_then(|turn| turn.progress_summary.clone())
            .or_else(|| recent_actions.first().map(|item| item.summary.clone()))
    };

    SourceActivityCardView {
        source_id: SYSTEM_SOURCE_ID.into(),
        source_kind: "system_agent".into(),
        title: "System Agent".into(),
        visibility: visibility_for_state(state.as_str()).into(),
        state: state.clone(),
        summary: shorten(&summary, 120),
        focus_label: latest_turn
            .and_then(|turn| turn.turn_id.split('-').next_back())
            .map(|suffix| format!("turn {suffix}")),
        auto_promoted: should_promote(
            state.as_str(),
            failure_detail.as_deref(),
            waiting_detail.as_deref(),
        ),
        current_activity: current_activity.map(|value| shorten(&value, 512)),
        recent_actions,
        waiting_detail,
        failure_detail,
        session_id: session_id.map(str::to_string),
        task_id: task_id.map(str::to_string),
        updated_at: execution_state
            .map(|value| value.updated_at.clone())
            .or_else(|| latest_turn.and_then(|turn| turn.completed_at.clone()))
            .unwrap_or_else(|| generated_at.to_string()),
    }
}

fn build_peer_cards(
    peers: &[PeerRegistryEntry],
    session_id: Option<&str>,
    task_id: Option<&str>,
) -> Vec<SourceActivityCardView> {
    peers
        .iter()
        .map(|peer| {
            let state = peer_state(peer);
            let failure_detail = peer
                .connectivity_state
                .as_deref()
                .filter(|value| matches!(*value, "degraded" | "failed" | "disconnected"))
                .map(|value| format!("connectivity {value}"));
            let waiting_detail = peer
                .binding_state
                .as_deref()
                .filter(|value| matches!(*value, "pairing_required" | "invalidated"))
                .map(|value| format!("binding {value}"));
            SourceActivityCardView {
                source_id: peer.peer_id.clone(),
                source_kind: peer.peer_kind.clone(),
                title: peer_title(peer),
                visibility: if failure_detail.is_some() || waiting_detail.is_some() {
                    "detailed".into()
                } else {
                    "compact".into()
                },
                state: state.clone(),
                summary: peer_summary(peer),
                focus_label: peer
                    .session_id
                    .as_deref()
                    .map(|value| format!("session {value}")),
                auto_promoted: should_promote(
                    state.as_str(),
                    failure_detail.as_deref(),
                    waiting_detail.as_deref(),
                ),
                current_activity: Some(peer_activity(peer)),
                recent_actions: Vec::new(),
                waiting_detail,
                failure_detail,
                session_id: session_id
                    .filter(|_| peer.session_id.as_deref() == session_id)
                    .map(str::to_string),
                task_id: task_id.map(str::to_string),
                updated_at: peer.updated_at.clone(),
            }
        })
        .collect()
}

fn build_user_card(cards: &[SourceActivityCardView], generated_at: &str) -> UserActivityCardView {
    let total_sources = cards.len();
    let running_sources = cards
        .iter()
        .filter(|card| matches!(card.state.as_str(), "running"))
        .count();
    let waiting_sources = cards
        .iter()
        .filter(|card| matches!(card.state.as_str(), "waiting" | "paused"))
        .count();
    let failed_sources = cards
        .iter()
        .filter(|card| matches!(card.state.as_str(), "failed"))
        .count();
    let idle_sources = total_sources
        .saturating_sub(running_sources)
        .saturating_sub(waiting_sources)
        .saturating_sub(failed_sources);
    let focus = cards
        .iter()
        .find(|card| card.source_id == SYSTEM_SOURCE_ID)
        .or_else(|| {
            cards.iter().find(|card| {
                card.auto_promoted
                    && (card.source_id == SYSTEM_SOURCE_ID || card.source_kind == "system_agent")
            })
        })
        .or_else(|| {
            cards.iter().find(|card| {
                card.auto_promoted && !card.source_kind.starts_with("channel_gateway.")
            })
        })
        .or_else(|| {
            cards
                .iter()
                .find(|card| !card.source_kind.starts_with("channel_gateway."))
        })
        .or_else(|| cards.first());
    let header = if let Some(focus_card) = focus {
        format!("system frontstage · {}", shorten(&focus_card.summary, 96))
    } else {
        "system frontstage · idle".into()
    };
    UserActivityCardView {
        owner_source_id: SYSTEM_SOURCE_ID.into(),
        header,
        state: focus
            .map(|card| card.state.clone())
            .unwrap_or_else(|| "idle".into()),
        focus_source_id: focus.map(|card| card.source_id.clone()),
        focus_summary: focus.map(|card| card.summary.clone()),
        stage: focus.and_then(|card| card.current_activity.clone()),
        recent_items: frontstage_recent_items(cards, focus),
        active_sources: cards
            .iter()
            .map(|card| ActivitySourceSummary {
                source_id: card.source_id.clone(),
                title: card.title.clone(),
                state: card.state.clone(),
                summary: card.summary.clone(),
                visibility: card.visibility.clone(),
                auto_promoted: card.auto_promoted,
            })
            .collect(),
        total_sources,
        running_sources,
        waiting_sources,
        failed_sources,
        idle_sources,
        waiting_detail: focus.and_then(|card| card.waiting_detail.clone()),
        failure_detail: focus.and_then(|card| card.failure_detail.clone()),
        updated_at: focus
            .map(|card| card.updated_at.clone())
            .unwrap_or_else(|| generated_at.to_string()),
    }
}

fn frontstage_recent_items(
    cards: &[SourceActivityCardView],
    focus: Option<&SourceActivityCardView>,
) -> Vec<String> {
    let mut items = focus
        .map(|card| {
            card.recent_actions
                .iter()
                .take(3)
                .map(frontstage_recent_item)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let focus_id = focus.map(|card| card.source_id.as_str());
    for item in cards
        .iter()
        .filter(|card| Some(card.source_id.as_str()) != focus_id)
        .filter(|card| !card.source_kind.starts_with("channel_gateway."))
        .filter(|card| {
            matches!(
                card.state.as_str(),
                "running" | "waiting" | "paused" | "failed"
            )
        })
        .take(2)
        .map(|card| {
            if let Some(action) = card.recent_actions.first() {
                let label = card
                    .source_id
                    .rsplit('.')
                    .next()
                    .unwrap_or(card.source_id.as_str());
                return format!(
                    "{label} · {} · {}",
                    card.state.as_str(),
                    shorten(frontstage_recent_item(action).as_str(), 72)
                );
            }
            frontstage_source_recent_item(
                card.source_id.as_str(),
                card.state.as_str(),
                card.summary.as_str(),
                card.current_activity.as_deref(),
                card.failure_detail.as_deref(),
            )
        })
    {
        if !items.iter().any(|existing| existing == &item) {
            items.push(item);
        }
    }
    items
}

fn apply_dispatch_frontstage_overlay(
    system_card: &mut SourceActivityCardView,
    agent_cards: &[SourceActivityCardView],
    owner_loop_action: Option<&OwnerLoopActionRecord>,
) {
    let active_workers = agent_cards
        .iter()
        .filter(|card| {
            matches!(
                card.source_kind.as_str(),
                "system_worker" | "project_agent" | "project_worker"
            ) && matches!(
                card.state.as_str(),
                "running" | "waiting" | "paused" | "failed"
            )
        })
        .take(2)
        .map(|card| {
            let label = card
                .source_id
                .rsplit('.')
                .next()
                .unwrap_or(card.source_id.as_str());
            let detail = card
                .current_activity
                .as_deref()
                .filter(|value| !value.trim().is_empty())
                .unwrap_or(card.summary.as_str());
            format!("{label}: {}", shorten(detail, 120))
        })
        .collect::<Vec<_>>();
    if !active_workers.is_empty() && matches!(system_card.state.as_str(), "idle" | "ready") {
        let detail = active_workers.join(" | ");
        system_card.state = "waiting".into();
        system_card.summary = "已派发任务，等待 worker 持续回报".into();
        system_card.current_activity = Some(format!("并行执行中：{detail}"));
        system_card.waiting_detail = Some(detail);
        system_card.failure_detail = None;
        system_card.recent_actions.clear();
        system_card.auto_promoted = true;
        return;
    }
    let Some(owner_loop_action) = owner_loop_action else {
        return;
    };
    if owner_loop_action.action_kind != "ready_tasks_present"
        || !matches!(system_card.state.as_str(), "idle" | "ready")
    {
        return;
    }
    let task_count = owner_loop_action.target_task_ids.len().max(1);
    let active_task = owner_loop_action.active_task_id.as_deref().unwrap_or("-");
    let waiting_detail =
        format!("最近已派发 {task_count} 个任务；当前关注 {active_task}；等待 worker 启动或回报");
    system_card.state = "waiting".into();
    system_card.summary = "已派发任务，等待 worker 回报".into();
    system_card.current_activity = Some(owner_loop_action.reason.clone());
    system_card.waiting_detail = Some(waiting_detail);
    system_card.failure_detail = None;
    system_card.recent_actions.clear();
    system_card.auto_promoted = true;
}
