use crate::{RuntimeError, tools::semantics};
mod agents;
#[cfg(test)]
mod delivery_tests;
mod helpers;
mod peers;
mod records;
mod store;
#[cfg(test)]
mod tests;
mod user_card;
use fin_contracts::{
    ActivityCardsSnapshot, ExecutionStateRecord, SourceActivityCardView, ToolExecutionRecord,
    ToolSemanticView, TurnRecord,
};
use std::{cmp::Reverse, path::Path};

use agents::{AgentPresenceRegistry, build_agent_cards};
use helpers::{
    latest_failed_action, local_now_placeholder, most_recent_actions, shorten, should_promote,
    source_rank, visibility_for_state,
};
use peers::build_peer_cards_from_registry as build_peer_cards;
use records::{OwnerLoopActionRecord, PeerRegistry, StartupControlSummaryRecord};
use store::{
    pending_inbound_notice, read_json_if_exists, read_last_run_json, read_last_run_value,
    read_last_run_vec, read_session_vec, string_field,
};
use user_card::{apply_dispatch_frontstage_overlay, build_user_card};

const SYSTEM_SOURCE_ID: &str = "system-agent";
const PENDING_INBOUND_NOTICE: &str = "已收到，正在处理";

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

    let semantics = semantics::semantic_views(&tool_records);
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
