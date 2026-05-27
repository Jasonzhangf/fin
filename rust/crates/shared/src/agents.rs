use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentKind {
    SystemAgent,
    ProjectAgent,
    Subagent,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContextMode {
    TaskSummaryOnly,
    LastNTurns,
    FullSessionContext,
}

impl Default for ContextMode {
    fn default() -> Self {
        Self::TaskSummaryOnly
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextPolicy {
    pub mode: ContextMode,
    pub last_n_turns: Option<u32>,
    pub reason: Option<String>,
}

impl Default for ContextPolicy {
    fn default() -> Self {
        Self {
            mode: ContextMode::TaskSummaryOnly,
            last_n_turns: None,
            reason: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct CapabilityDescriptor {
    pub capability_ids: Vec<String>,
    pub tool_allowlist: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentIdentity {
    pub agent_id: String,
    pub kind: AgentKind,
    pub parent_agent_id: Option<String>,
    pub project_id: Option<String>,
    pub device_binding: String,
    pub auth_subject: String,
    pub capability_descriptor: CapabilityDescriptor,
    pub auth_lease_id: String,
    pub path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentRunRecord {
    pub agent_run_id: String,
    pub agent_id: String,
    pub parent_run_id: Option<String>,
    pub task_id: Option<String>,
    pub assignment_id: Option<String>,
    pub status: String,
    pub result_refs: Vec<String>,
    pub last_heartbeat_at: String,
    pub closed_at: Option<String>,
    pub context_policy: Option<ContextPolicy>,
    pub path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RegisterPrimaryAgentInput {
    pub agent_id: String,
    pub kind: AgentKind,
    pub project_id: Option<String>,
    pub device_binding: String,
    pub auth_subject: String,
    pub auth_lease_id: String,
    pub capability_descriptor: CapabilityDescriptor,
    pub now: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpawnSubagentInput {
    pub parent_agent_id: String,
    pub parent_run_id: Option<String>,
    pub agent_run_id: String,
    pub task_id: Option<String>,
    pub assignment_id: Option<String>,
    pub context_policy: ContextPolicy,
    pub now: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WaitAgentResult {
    pub status: String,
    pub agent_run_id: String,
    pub result_refs: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CloseAgentResult {
    pub agent_id: String,
    pub agent_run_id: Option<String>,
    pub status: String,
    pub action: String,
    pub affected_run_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResumeAgentResult {
    pub agent_id: String,
    pub agent_run_id: String,
    pub status: String,
    pub path: String,
}

pub fn primary_path(kind: &AgentKind, agent_id: &str, project_id: Option<&str>) -> String {
    match kind {
        AgentKind::SystemAgent => format!("system:{agent_id}"),
        AgentKind::ProjectAgent => {
            format!("project:{}:{agent_id}", project_id.unwrap_or("unknown"))
        }
        AgentKind::Subagent => unreachable!("subagent primary path is derived from parent run"),
    }
}

pub fn validate_context_policy(policy: &ContextPolicy) -> Result<(), String> {
    if matches!(policy.mode, ContextMode::FullSessionContext)
        && policy.reason.as_deref().unwrap_or("").trim().is_empty()
    {
        return Err("full_session_context requires reason".into());
    }
    if matches!(policy.mode, ContextMode::LastNTurns) && policy.last_n_turns.unwrap_or(0) == 0 {
        return Err("last_n_turns context mode requires last_n_turns > 0".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn full_session_context_requires_reason() {
        let err = validate_context_policy(&ContextPolicy {
            mode: ContextMode::FullSessionContext,
            last_n_turns: None,
            reason: None,
        })
        .expect_err("missing reason must fail");
        assert!(err.contains("requires reason"));
    }
}
