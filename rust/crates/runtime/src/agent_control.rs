use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    fs,
    path::{Path, PathBuf},
};

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
        let parent = self.read_identity_required(&input.parent_agent_id)?;
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
        self.read_identity_required(&input.from_agent_id)?;
        self.read_identity_required(&input.to_agent_id)?;
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

    pub fn wait_agent(&self, agent_run_id: &str) -> Result<WaitAgentResult, String> {
        let run = self.read_run_required(agent_run_id)?;
        let status = match run.status.as_str() {
            "completed" | "failed" | "timeout" | "closed" => run.status.clone(),
            _ => "timeout".into(),
        };
        Ok(WaitAgentResult {
            status,
            agent_run_id: run.agent_run_id,
            result_refs: run.result_refs,
        })
    }

    pub fn close_agent(&self, agent_id: &str, now: &str) -> Result<CloseAgentResult, String> {
        let identity = self.read_identity_required(agent_id)?;
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
        let identity = self.read_identity_required(agent_id)?;
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
                let mut run = self.read_run_required(run_id)?;
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
        let mut run = self.read_run_required(agent_run_id)?;
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

    pub fn read_mailbox(&self, agent_id: &str) -> Result<Vec<AgentMailboxMessage>, String> {
        read_json(&self.mailbox_path(agent_id)).map(|value| value.unwrap_or_default())
    }

    fn identities_path(&self) -> PathBuf {
        self.root.join("identities.json")
    }

    fn runs_path(&self) -> PathBuf {
        self.root.join("runs.json")
    }

    fn identity_path(&self, agent_id: &str) -> PathBuf {
        self.root
            .join("identities")
            .join(format!("{}.json", safe_file_name(agent_id)))
    }

    fn run_path(&self, agent_run_id: &str) -> PathBuf {
        self.root
            .join("runs")
            .join(format!("{}.json", safe_file_name(agent_run_id)))
    }

    fn mailbox_path(&self, agent_id: &str) -> PathBuf {
        self.root
            .join("mailbox")
            .join(safe_file_name(agent_id))
            .join("inbox.json")
    }

    fn read_identity_required(&self, agent_id: &str) -> Result<AgentIdentity, String> {
        read_json(&self.identity_path(agent_id))?
            .ok_or_else(|| format!("unknown agent identity: {agent_id}"))
    }

    fn read_run_required(&self, agent_run_id: &str) -> Result<AgentRunRecord, String> {
        read_json(&self.run_path(agent_run_id))?
            .ok_or_else(|| format!("unknown agent run: {agent_run_id}"))
    }

    fn write_identity(&self, identity: &AgentIdentity) -> Result<(), String> {
        write_json(&self.identity_path(&identity.agent_id), identity)
    }

    fn write_run(&self, run: &AgentRunRecord) -> Result<(), String> {
        write_json(&self.run_path(&run.agent_run_id), run)
    }

    fn read_identity_registry(&self) -> Result<Vec<AgentIdentity>, String> {
        read_json(&self.identities_path()).map(|value| value.unwrap_or_default())
    }

    fn read_run_registry(&self) -> Result<Vec<AgentRunRecord>, String> {
        read_json(&self.runs_path()).map(|value| value.unwrap_or_default())
    }

    fn upsert_identity_registry(&self, identity: AgentIdentity) -> Result<(), String> {
        let mut identities = self.read_identity_registry()?;
        identities.retain(|item| item.agent_id != identity.agent_id);
        identities.push(identity);
        identities.sort_by(|left, right| left.agent_id.cmp(&right.agent_id));
        write_json(&self.identities_path(), &identities)
    }

    fn upsert_run_registry(&self, run: AgentRunRecord) -> Result<(), String> {
        let mut runs = self.read_run_registry()?;
        runs.retain(|item| item.agent_run_id != run.agent_run_id);
        runs.push(run);
        runs.sort_by(|left, right| left.agent_run_id.cmp(&right.agent_run_id));
        write_json(&self.runs_path(), &runs)
    }

    fn write_mailbox(
        &self,
        agent_id: &str,
        messages: &[AgentMailboxMessage],
    ) -> Result<(), String> {
        write_json(&self.mailbox_path(agent_id), messages)
    }
}

fn primary_path(kind: &AgentKind, agent_id: &str, project_id: Option<&str>) -> String {
    match kind {
        AgentKind::SystemAgent => format!("system:{agent_id}"),
        AgentKind::ProjectAgent => {
            format!("project:{}:{agent_id}", project_id.unwrap_or("unknown"))
        }
        AgentKind::Subagent => unreachable!("subagent primary path is derived from parent run"),
    }
}

fn validate_context_policy(policy: &ContextPolicy) -> Result<(), String> {
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

fn read_json<T: for<'de> Deserialize<'de>>(path: &Path) -> Result<Option<T>, String> {
    match fs::read_to_string(path) {
        Ok(content) => serde_json::from_str::<T>(&content)
            .map(Some)
            .map_err(|err| err.to_string()),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(err) => Err(err.to_string()),
    }
}

fn write_json<T: Serialize + ?Sized>(path: &Path, value: &T) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|err| err.to_string())?;
    }
    fs::write(
        path,
        serde_json::to_vec_pretty(value).map_err(|err| err.to_string())?,
    )
    .map_err(|err| err.to_string())
}

fn safe_file_name(value: &str) -> String {
    value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.') {
                ch
            } else {
                '_'
            }
        })
        .collect()
}

fn sanitize_id(value: &str) -> String {
    safe_file_name(value).trim_matches('_').to_string()
}
