use crate::{RuntimeError, tool_semantics};
#[path = "activity_cards_helpers.rs"]
mod activity_cards_helpers;
#[path = "activity_cards_project_actions.rs"]
mod activity_cards_project_actions;
#[path = "activity_cards_render.rs"]
mod activity_cards_render;
#[path = "activity_cards_store.rs"]
mod activity_cards_store;
use fin_contracts::{
    ActivityCardsSnapshot, ActivitySourceSummary, ExecutionStateRecord, SourceActivityCardView,
    ToolExecutionRecord, ToolSemanticView, TurnRecord, UserActivityCardView,
};
use serde::Deserialize;
use serde_json::Value;
use std::{cmp::Reverse, path::Path};

use activity_cards_helpers::{
    latest_failed_action, local_now_fallback, most_recent_actions, peer_activity, peer_state,
    peer_summary, peer_title, shorten, should_promote, source_rank, visibility_for_state,
};
use activity_cards_project_actions::project_recent_actions;
use activity_cards_render::{build_peer_cards, build_system_card, build_user_card};
use activity_cards_store::{
    pending_inbound_notice, read_json_if_exists, read_json_vec_if_exists, read_last_run_json,
    read_last_run_value, read_last_run_vec, string_field,
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
    agent_name: Option<String>,
    #[serde(default)]
    display_name: Option<String>,
    #[serde(default)]
    device_name: Option<String>,
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
struct ProjectSupervisionSnapshotView {
    #[serde(default)]
    projects: Vec<ProjectSupervisionProjectView>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
struct ProjectSupervisionProjectView {
    project_id: String,
    agent_id: String,
    supervision_state: String,
    desired_action: String,
    summary: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
struct ProjectRuntimePickupSnapshotView {
    #[serde(default)]
    projects: Vec<ProjectRuntimePickupProjectView>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
struct ProjectRuntimePickupProjectView {
    project_id: String,
    agent_id: String,
    pickup_state: String,
    next_action: String,
    summary: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
struct AgentRunRecordView {
    agent_run_id: String,
    agent_id: String,
    #[serde(default)]
    task_id: Option<String>,
    #[serde(default)]
    assignment_id: Option<String>,
    status: String,
    #[serde(default)]
    result_refs: Vec<String>,
    last_heartbeat_at: String,
    #[serde(default)]
    closed_at: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
struct AgentMailboxMessageView {
    message_id: String,
    seq: u64,
    from_agent_id: String,
    to_agent_id: String,
    #[serde(default)]
    task_id: Option<String>,
    #[serde(default)]
    trigger_turn: bool,
    payload: Value,
    #[serde(default)]
    consumed_at: Option<String>,
}

pub fn build_activity_cards(runtime_home: &Path) -> Result<ActivityCardsSnapshot, RuntimeError> {
    let last_run = read_last_run_json(runtime_home)?;
    let session_id = string_field(&last_run, "session_id");
    let task_id = string_field(&last_run, "task_id");
    let generated_at = string_field(&last_run, "submitted_at")
        .or_else(|| string_field(&last_run, "occurred_at"))
        .unwrap_or_else(local_now_fallback);

    let tool_records =
        read_last_run_vec::<ToolExecutionRecord>(runtime_home, "session_recent_tool_records_path")?;
    let turns = read_last_run_vec::<TurnRecord>(runtime_home, "session_recent_turns_path")?;
    let execution_state =
        read_last_run_value::<ExecutionStateRecord>(runtime_home, "current_execution_state_path")?;
    let startup_summary = read_json_if_exists::<StartupControlSummaryRecord>(
        &runtime_home.join("runtime/current/current_startup_control_summary.json"),
    )?;
    let peer_registry =
        read_json_if_exists::<PeerRegistry>(&runtime_home.join("runtime/peers/registry.json"))?
            .unwrap_or_default();
    let pending_inbound_notice = pending_inbound_notice(runtime_home, session_id.as_deref())?;

    let semantics = tool_semantics::semantic_views(&tool_records);
    let system_card = build_system_card(
        session_id.as_deref(),
        task_id.as_deref(),
        execution_state.as_ref(),
        startup_summary.as_ref(),
        semantics.as_slice(),
        turns.as_slice(),
        generated_at.as_str(),
    );
    let peer_cards = build_peer_cards(
        runtime_home,
        peer_registry.peers.as_slice(),
        session_id.as_deref(),
        task_id.as_deref(),
    );

    let mut source_cards = Vec::with_capacity(1 + peer_cards.len());
    source_cards.push(system_card);
    source_cards.extend(peer_cards);
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
