use super::helpers::{
    agent_activity, agent_state, agent_summary, agent_title, should_promote, visibility_for_state,
};
use fin_contracts::{SourceActivityCardView, ToolSemanticView};
use serde::Deserialize;

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
pub(super) struct AgentPresenceRegistry {
    #[serde(default)]
    pub(super) agents: Vec<AgentPresenceEntry>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
pub(super) struct AgentPresenceEntry {
    pub(super) agent_id: String,
    pub(super) agent_name: String,
    pub(super) device_name: String,
    #[serde(default)]
    pub(super) worker_id: Option<String>,
    pub(super) role_id: String,
    pub(super) agent_kind: String,
    #[serde(default)]
    pub(super) project_id: Option<String>,
    pub(super) status: String,
    #[serde(default)]
    pub(super) current_task_id: Option<String>,
    #[serde(default)]
    pub(super) current_operation_id: Option<String>,
    #[serde(default)]
    pub(super) current_session_id: Option<String>,
    #[serde(default)]
    pub(super) current_phase: Option<String>,
    #[serde(default)]
    pub(super) is_reasoning: bool,
    pub(super) updated_at: String,
    #[serde(default)]
    pub(super) progress_summary: String,
    #[serde(default)]
    pub(super) pending_input_count: usize,
    #[serde(default)]
    pub(super) waiting_reason: Option<String>,
    #[serde(default)]
    pub(super) recent_actions: Vec<ToolSemanticView>,
}

pub(super) fn build_agent_cards(
    agents: &[AgentPresenceEntry],
    session_id: Option<&str>,
    task_id: Option<&str>,
) -> Vec<SourceActivityCardView> {
    agents
        .iter()
        .filter(|agent| agent.agent_kind != "system_entry")
        .map(|agent| {
            let state = agent_state(agent);
            let failure_detail = None;
            let waiting_detail = if matches!(state.as_str(), "waiting" | "paused") {
                agent
                    .waiting_reason
                    .clone()
                    .or_else(|| agent.current_phase.clone())
            } else {
                None
            };
            SourceActivityCardView {
                source_id: agent.agent_id.clone(),
                source_kind: agent.agent_kind.clone(),
                title: agent_title(agent),
                visibility: visibility_for_state(state.as_str()).into(),
                state: state.clone(),
                summary: agent_summary(agent),
                focus_label: agent
                    .current_task_id
                    .as_deref()
                    .map(|value| format!("task {value}"))
                    .or_else(|| {
                        agent
                            .current_session_id
                            .as_deref()
                            .map(|value| format!("session {value}"))
                    }),
                auto_promoted: should_promote(
                    state.as_str(),
                    failure_detail.as_deref(),
                    waiting_detail.as_deref(),
                ),
                current_activity: Some(agent_activity(agent)),
                recent_actions: agent.recent_actions.clone(),
                waiting_detail,
                failure_detail,
                session_id: session_id
                    .filter(|value| agent.current_session_id.as_deref() == Some(*value))
                    .map(str::to_string),
                task_id: task_id
                    .filter(|value| agent.current_task_id.as_deref() == Some(*value))
                    .map(str::to_string),
                updated_at: agent.updated_at.clone(),
            }
        })
        .collect()
}
