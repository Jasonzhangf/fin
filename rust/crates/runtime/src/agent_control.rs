use crate::agent_control_io::{
    identities_path, identity_path, mailbox_path, read_identity_required, read_json,
    read_run_required, run_path, runs_path, sanitize_id, write_json,
};
use fin_shared::{
    AgentIdentity, AgentKind, AgentRunRecord, CloseAgentResult, RegisterPrimaryAgentInput,
    ResumeAgentResult, SpawnSubagentInput, WaitAgentResult, primary_path, validate_context_policy,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AgentMailboxMessage {
    pub message_id: String,
    pub seq: u64,
    pub from_agent_id: String,
    pub to_agent_id: String,
    pub thread_id: Option<String>,
    pub task_id: Option<String>,
    pub trigger_turn: bool,
    pub payload: Value,
    pub consumed_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SendAgentInput {
    pub message_id: String,
    pub from_agent_id: String,
    pub to_agent_id: String,
    pub thread_id: Option<String>,
    pub task_id: Option<String>,
    pub trigger_turn: bool,
    pub payload: Value,
}

#[derive(Debug, Clone)]
pub struct AgentControlStore {
    root: PathBuf,
}

impl AgentControlStore {
    pub fn new(runtime_home: impl AsRef<Path>) -> Self {
        Self {
            root: runtime_home.as_ref().join("runtime/agents/control"),
        }
    }

    pub fn register_primary_agent(
        &self,
        input: RegisterPrimaryAgentInput,
    ) -> Result<AgentIdentity, String> {
        if !matches!(input.kind, AgentKind::SystemAgent | AgentKind::ProjectAgent) {
            return Err("register_primary_agent only accepts system_agent or project_agent".into());
        }
        if matches!(input.kind, AgentKind::ProjectAgent)
            && input.project_id.as_deref().unwrap_or("").is_empty()
        {
            return Err("project_agent registration requires project_id".into());
        }
        let identity = AgentIdentity {
            path: primary_path(&input.kind, &input.agent_id, input.project_id.as_deref()),
            agent_id: input.agent_id,
            kind: input.kind,
            parent_agent_id: None,
            project_id: input.project_id,
            device_binding: input.device_binding,
            auth_subject: input.auth_subject,
            capability_descriptor: input.capability_descriptor,
            auth_lease_id: input.auth_lease_id,
        };
        self.write_identity(&identity)?;
        self.upsert_identity_registry(identity.clone())?;
        Ok(identity)
    }

    pub fn spawn_subagent(
        &self,
        input: SpawnSubagentInput,
    ) -> Result<(AgentIdentity, AgentRunRecord), String> {
        let parent = read_identity_required(&self.root, &input.parent_agent_id)?;
        if matches!(parent.kind, AgentKind::Subagent) {
            return Err("subagent cannot mint an independent durable child identity".into());
        }
        validate_context_policy(&input.context_policy)?;
        let agent_id = format!("{}:subagent:{}", parent.agent_id, input.agent_run_id);
        let path = format!("{}/subagent:{}", parent.path, input.agent_run_id);
        let identity = AgentIdentity {
            agent_id: agent_id.clone(),
            kind: AgentKind::Subagent,
            parent_agent_id: Some(parent.agent_id.clone()),
            project_id: parent.project_id.clone(),
            device_binding: parent.device_binding.clone(),
            auth_subject: parent.auth_subject.clone(),
            capability_descriptor: parent.capability_descriptor.clone(),
            auth_lease_id: parent.auth_lease_id.clone(),
            path: path.clone(),
        };
        let run = AgentRunRecord {
            agent_run_id: input.agent_run_id,
            agent_id,
            parent_run_id: input.parent_run_id,
            task_id: input.task_id,
            assignment_id: input.assignment_id,
            status: "running".into(),
            result_refs: Vec::new(),
            last_heartbeat_at: input.now,
            closed_at: None,
            context_policy: Some(input.context_policy),
            path,
        };
        self.write_identity(&identity)?;
        self.upsert_identity_registry(identity.clone())?;
        self.write_run(&run)?;
        self.upsert_run_registry(run.clone())?;
        Ok((identity, run))
    }

    pub fn send_agent_input(&self, input: SendAgentInput) -> Result<AgentMailboxMessage, String> {
        read_identity_required(&self.root, &input.from_agent_id)?;
        read_identity_required(&self.root, &input.to_agent_id)?;
        let mut inbox = self.read_mailbox(&input.to_agent_id)?;
        let seq = inbox.last().map(|item| item.seq).unwrap_or(0) + 1;
        let message = AgentMailboxMessage {
            message_id: input.message_id,
            seq,
            from_agent_id: input.from_agent_id,
            to_agent_id: input.to_agent_id.clone(),
            thread_id: input.thread_id,
            task_id: input.task_id,
            trigger_turn: input.trigger_turn,
            payload: input.payload,
            consumed_at: None,
        };
        inbox.push(message.clone());
        self.write_mailbox(&input.to_agent_id, &inbox)?;
        Ok(message)
    }

    pub fn consume_next_mailbox_message(
        &self,
        agent_id: &str,
        consumed_at: &str,
    ) -> Result<Option<AgentMailboxMessage>, String> {
        read_identity_required(&self.root, agent_id)?;
        let mut inbox = self.read_mailbox(agent_id)?;
        let Some(index) = inbox
            .iter()
            .position(|message| message.consumed_at.is_none())
        else {
            return Ok(None);
        };
        inbox[index].consumed_at = Some(consumed_at.into());
        let message = inbox[index].clone();
        self.write_mailbox(agent_id, &inbox)?;
        Ok(Some(message))
    }

    pub fn wait_agent(&self, agent_run_id: &str) -> Result<WaitAgentResult, String> {
        let started = Instant::now();
        let timeout = Duration::from_secs(90);
        loop {
            let run = read_run_required(&self.root, agent_run_id)?;
            match run.status.as_str() {
                "completed" | "failed" | "timeout" | "closed" => {
                    return Ok(WaitAgentResult {
                        status: run.status,
                        agent_run_id: run.agent_run_id,
                        result_refs: run.result_refs,
                    });
                }
                _ => {
                    if started.elapsed() >= timeout {
                        return Ok(WaitAgentResult {
                            status: "timeout".into(),
                            agent_run_id: run.agent_run_id,
                            result_refs: run.result_refs,
                        });
                    }
                    std::thread::sleep(Duration::from_millis(50));
                }
            }
        }
    }

    pub fn close_agent(&self, agent_id: &str, now: &str) -> Result<CloseAgentResult, String> {
        let identity = read_identity_required(&self.root, agent_id)?;
        match identity.kind {
            AgentKind::Subagent => {
                let runs = self.read_run_registry()?;
                let descendant_prefix = format!("{}/", identity.path);
                let mut affected = Vec::new();
                for mut run in runs.into_iter().filter(|run| {
                    run.agent_id == identity.agent_id || run.path.starts_with(&descendant_prefix)
                }) {
                    if run.status != "closed" {
                        run.status = "closed".into();
                        run.closed_at = Some(now.into());
                        self.write_run(&run)?;
                        affected.push(run.agent_run_id.clone());
                    }
                }
                Ok(CloseAgentResult {
                    agent_id: identity.agent_id,
                    agent_run_id: affected.first().cloned(),
                    status: "closed".into(),
                    action: "close_subagent_subtree".into(),
                    affected_run_ids: affected,
                })
            }
            AgentKind::SystemAgent | AgentKind::ProjectAgent => Ok(CloseAgentResult {
                agent_id: identity.agent_id,
                agent_run_id: None,
                status: "detached".into(),
                action: "release_primary_lease".into(),
                affected_run_ids: Vec::new(),
            }),
        }
    }

    pub fn resume_agent(
        &self,
        agent_id: &str,
        agent_run_id: Option<&str>,
        now: &str,
    ) -> Result<ResumeAgentResult, String> {
        let identity = read_identity_required(&self.root, agent_id)?;
        match identity.kind {
            AgentKind::SystemAgent | AgentKind::ProjectAgent => {
                let run_id = agent_run_id
                    .map(str::to_string)
                    .unwrap_or_else(|| format!("resume-{}", sanitize_id(now)));
                let run = AgentRunRecord {
                    agent_run_id: run_id.clone(),
                    agent_id: identity.agent_id.clone(),
                    parent_run_id: None,
                    task_id: None,
                    assignment_id: None,
                    status: "running".into(),
                    result_refs: Vec::new(),
                    last_heartbeat_at: now.into(),
                    closed_at: None,
                    context_policy: None,
                    path: identity.path.clone(),
                };
                self.write_run(&run)?;
                self.upsert_run_registry(run.clone())?;
                Ok(ResumeAgentResult {
                    agent_id: identity.agent_id,
                    agent_run_id: run.agent_run_id,
                    status: "running".into(),
                    path: identity.path,
                })
            }
            AgentKind::Subagent => {
                let run_id = agent_run_id
                    .ok_or_else(|| "subagent resume requires agent_run_id".to_string())?;
                let mut run = read_run_required(&self.root, run_id)?;
                if run.agent_id != identity.agent_id {
                    return Err("agent_run_id does not belong to subagent identity".into());
                }
                run.status = "running".into();
                run.closed_at = None;
                run.last_heartbeat_at = now.into();
                self.write_run(&run)?;
                self.upsert_run_registry(run.clone())?;
                Ok(ResumeAgentResult {
                    agent_id: identity.agent_id,
                    agent_run_id: run.agent_run_id,
                    status: run.status,
                    path: run.path,
                })
            }
        }
    }

    pub fn update_run_status(
        &self,
        agent_run_id: &str,
        status: &str,
        result_refs: Vec<String>,
        now: &str,
    ) -> Result<AgentRunRecord, String> {
        let mut run = read_run_required(&self.root, agent_run_id)?;
        run.status = status.into();
        run.result_refs = result_refs;
        run.last_heartbeat_at = now.into();
        if matches!(status, "completed" | "failed" | "closed") {
            run.closed_at = Some(now.into());
        }
        self.write_run(&run)?;
        self.upsert_run_registry(run.clone())?;
        Ok(run)
    }

    pub fn read_run(&self, agent_run_id: &str) -> Option<AgentRunRecord> {
        read_run_required(&self.root, agent_run_id).ok()
    }

    pub fn read_mailbox(&self, agent_id: &str) -> Result<Vec<AgentMailboxMessage>, String> {
        read_json(&mailbox_path(&self.root, agent_id)).map(|value| value.unwrap_or_default())
    }

    fn write_identity(&self, identity: &AgentIdentity) -> Result<(), String> {
        write_json(&identity_path(&self.root, &identity.agent_id), identity)
    }

    fn write_run(&self, run: &AgentRunRecord) -> Result<(), String> {
        write_json(&run_path(&self.root, &run.agent_run_id), run)
    }

    fn read_identity_registry(&self) -> Result<Vec<AgentIdentity>, String> {
        read_json(&identities_path(&self.root)).map(|value| value.unwrap_or_default())
    }

    fn read_run_registry(&self) -> Result<Vec<AgentRunRecord>, String> {
        read_json(&runs_path(&self.root)).map(|value| value.unwrap_or_default())
    }

    fn upsert_identity_registry(&self, identity: AgentIdentity) -> Result<(), String> {
        let mut identities = self.read_identity_registry()?;
        identities.retain(|item| item.agent_id != identity.agent_id);
        identities.push(identity);
        identities.sort_by(|left, right| left.agent_id.cmp(&right.agent_id));
        write_json(&identities_path(&self.root), &identities)
    }

    fn upsert_run_registry(&self, run: AgentRunRecord) -> Result<(), String> {
        let mut runs = self.read_run_registry()?;
        runs.retain(|item| item.agent_run_id != run.agent_run_id);
        runs.push(run);
        runs.sort_by(|left, right| left.agent_run_id.cmp(&right.agent_run_id));
        write_json(&runs_path(&self.root), &runs)
    }

    fn write_mailbox(
        &self,
        agent_id: &str,
        messages: &[AgentMailboxMessage],
    ) -> Result<(), String> {
        write_json(&mailbox_path(&self.root, agent_id), messages)
    }
}
