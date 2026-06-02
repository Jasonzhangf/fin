use fin_contracts::ProjectRef;
use serde::Deserialize;
use std::{
    fs,
    path::{Path, PathBuf},
};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct ProjectRegistryContextSnapshot {
    pub(super) active_projects: Vec<ProjectRef>,
    pub(super) projects: Vec<ProjectRef>,
    pub(super) active_agent_ids: Vec<String>,
    pub(super) agent_presence_summary: Option<String>,
    pub(super) supervision_actions: Vec<String>,
    pub(super) project_supervision_summary: Option<String>,
    pub(super) assignment_queue_summary: Option<String>,
    pub(super) mailbox_summary: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct AgentPresenceSnapshot {
    pub(super) agents: Vec<AgentPresenceSnapshotEntry>,
    pub(super) active_agent_ids: Vec<String>,
    pub(super) summary: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct AgentPresenceSnapshotEntry {
    pub(super) agent_id: String,
    pub(super) worker_id: String,
    pub(super) device_name: String,
    pub(super) agent_name: String,
    pub(super) status: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct ProjectSupervisionSnapshot {
    pub(super) ready_count: usize,
    pub(super) resume_ready_count: usize,
    pub(super) busy_count: usize,
    pub(super) waiting_count: usize,
    pub(super) recover_needed_count: usize,
    pub(super) projects: Vec<ProjectSupervisionSnapshotEntry>,
    pub(super) actions: Vec<String>,
    pub(super) summary: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct ProjectSupervisionSnapshotEntry {
    pub(super) project_id: String,
    pub(super) desired_action: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
struct StoredProjectRegistryEntry {
    project_id: String,
    #[serde(default)]
    agent_id: String,
    #[serde(default)]
    project_root: Option<String>,
    #[serde(default)]
    presence_state: String,
    #[serde(default)]
    unfinished_task_count: usize,
}

pub(super) fn load_project_registry_context(
    runtime_home: Option<&str>,
) -> ProjectRegistryContextSnapshot {
    let Some(runtime_home) = runtime_home.filter(|value| !value.trim().is_empty()) else {
        return ProjectRegistryContextSnapshot::default();
    };
    let path = Path::new(runtime_home).join("runtime/projects/registry.json");
    let Ok(content) = fs::read_to_string(path) else {
        return ProjectRegistryContextSnapshot::default();
    };
    let Ok(entries) = serde_json::from_str::<Vec<StoredProjectRegistryEntry>>(&content) else {
        return ProjectRegistryContextSnapshot::default();
    };

    let mut projects = entries
        .iter()
        .map(project_ref_from_registry)
        .collect::<Vec<_>>();
    projects.sort_by(|left, right| left.project_id.cmp(&right.project_id));

    let mut active_projects = entries
        .iter()
        .filter(|entry| {
            matches!(entry.presence_state.as_str(), "busy" | "idle" | "waiting")
                || entry.unfinished_task_count > 0
        })
        .map(project_ref_from_registry)
        .collect::<Vec<_>>();
    active_projects.sort_by(|left, right| left.project_id.cmp(&right.project_id));

    ProjectRegistryContextSnapshot {
        active_projects,
        projects,
        active_agent_ids: load_active_agent_ids(runtime_home),
        agent_presence_summary: load_agent_presence_summary(runtime_home),
        supervision_actions: load_project_supervision_actions(runtime_home),
        project_supervision_summary: load_project_supervision_summary(runtime_home),
        assignment_queue_summary: load_assignment_queue_summary(runtime_home),
        mailbox_summary: load_mailbox_summary(runtime_home),
    }
}

fn project_ref_from_registry(entry: &StoredProjectRegistryEntry) -> ProjectRef {
    ProjectRef {
        project_id: entry.project_id.clone(),
        label: path_basename(
            entry
                .project_root
                .as_deref()
                .unwrap_or(entry.project_id.as_str()),
        )
        .unwrap_or_else(|| entry.project_id.clone()),
        root: entry.project_root.clone(),
        state: Some(if entry.unfinished_task_count > 0 {
            format!(
                "presence={} unfinished_tasks={}",
                entry.presence_state, entry.unfinished_task_count
            )
        } else {
            format!("presence={}", entry.presence_state)
        }),
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
struct StoredAgentPresenceRegistry {
    #[serde(default)]
    agents: Vec<StoredAgentPresenceEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
struct StoredAgentPresenceEntry {
    agent_id: String,
    #[serde(default)]
    worker_id: String,
    #[serde(default)]
    device_name: String,
    #[serde(default)]
    agent_name: String,
    #[serde(default)]
    status: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
struct StoredProjectSupervisionSnapshot {
    #[serde(default)]
    ready_count: usize,
    #[serde(default)]
    resume_ready_count: usize,
    #[serde(default)]
    busy_count: usize,
    #[serde(default)]
    waiting_count: usize,
    #[serde(default)]
    recover_needed_count: usize,
    #[serde(default)]
    projects: Vec<StoredProjectSupervisionRecord>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
struct StoredProjectSupervisionRecord {
    project_id: String,
    desired_action: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
struct StoredAssignmentRecord {
    #[serde(default)]
    peer_id: String,
    #[serde(default)]
    target_worker_id: Option<String>,
    #[serde(default)]
    owner_worker_id: Option<String>,
    #[serde(default)]
    status: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
struct StoredMailboxMessage {
    #[serde(default)]
    target_peer_id: String,
}

fn load_agent_presence_summary(runtime_home: &str) -> Option<String> {
    load_agent_presence_snapshot(runtime_home).and_then(|snapshot| snapshot.summary)
}

fn load_active_agent_ids(runtime_home: &str) -> Vec<String> {
    load_agent_presence_snapshot(runtime_home)
        .map(|snapshot| snapshot.active_agent_ids)
        .unwrap_or_default()
}

fn load_project_supervision_summary(runtime_home: &str) -> Option<String> {
    load_project_supervision_snapshot(runtime_home).and_then(|snapshot| snapshot.summary)
}

fn load_project_supervision_actions(runtime_home: &str) -> Vec<String> {
    load_project_supervision_snapshot(runtime_home)
        .map(|snapshot| snapshot.actions)
        .unwrap_or_default()
}

fn load_assignment_queue_summary(runtime_home: &str) -> Option<String> {
    let path = Path::new(runtime_home).join("runtime/assignments/pending.json");
    let Ok(content) = fs::read_to_string(path) else {
        return None;
    };
    let Ok(items) = serde_json::from_str::<Vec<StoredAssignmentRecord>>(&content) else {
        return None;
    };
    if items.is_empty() {
        return Some("pending_assignments=0".into());
    }

    let mut display = items
        .iter()
        .take(4)
        .map(|item| {
            let target = item
                .target_worker_id
                .as_deref()
                .filter(|value| !value.trim().is_empty())
                .unwrap_or(item.peer_id.as_str());
            let owner = item.owner_worker_id.as_deref().unwrap_or("-");
            let status = if item.status.trim().is_empty() {
                "pending"
            } else {
                item.status.as_str()
            };
            format!("{target}<-{owner}:{status}")
        })
        .collect::<Vec<_>>();
    display.sort();

    Some(format!(
        "pending_assignments={} [{}{}]",
        items.len(),
        display.join(", "),
        if items.len() > display.len() {
            ", ..."
        } else {
            ""
        }
    ))
}

fn load_mailbox_summary(runtime_home: &str) -> Option<String> {
    let mailbox_root = Path::new(runtime_home).join("runtime/mailbox");
    let Ok(entries) = fs::read_dir(mailbox_root) else {
        return None;
    };

    let mut total_messages = 0usize;
    let mut boxes = Vec::new();
    for entry in entries.filter_map(Result::ok) {
        let peer_id = entry.file_name().to_string_lossy().to_string();
        let inbox_path = entry.path().join("inbox.json");
        let Ok(content) = fs::read_to_string(&inbox_path) else {
            continue;
        };
        let Ok(items) = serde_json::from_str::<Vec<StoredMailboxMessage>>(&content) else {
            continue;
        };
        if items.is_empty() {
            continue;
        }
        total_messages += items.len();
        boxes.push(format!("{peer_id}:{}", items.len()));
    }
    boxes.sort();

    Some(if total_messages == 0 {
        "mailbox_messages=0".into()
    } else {
        format!(
            "mailbox_messages={} [{}{}]",
            total_messages,
            boxes.iter().take(4).cloned().collect::<Vec<_>>().join(", "),
            if boxes.len() > 4 { ", ..." } else { "" }
        )
    })
}

pub(super) fn load_agent_presence_snapshot(runtime_home: &str) -> Option<AgentPresenceSnapshot> {
    let path = Path::new(runtime_home).join("runtime/current/current_agent_presence_registry.json");
    let Ok(content) = fs::read_to_string(path) else {
        return None;
    };
    let Ok(registry) = serde_json::from_str::<StoredAgentPresenceRegistry>(&content) else {
        return None;
    };

    let agents = registry
        .agents
        .into_iter()
        .map(|entry| AgentPresenceSnapshotEntry {
            agent_id: entry.agent_id,
            worker_id: entry.worker_id,
            device_name: entry.device_name,
            agent_name: entry.agent_name,
            status: entry.status,
        })
        .collect::<Vec<_>>();
    let mut display = agents
        .iter()
        .map(|entry| {
            if !entry.device_name.trim().is_empty() && !entry.agent_name.trim().is_empty() {
                format!(
                    "{}.{}:{}",
                    entry.device_name, entry.agent_name, entry.status
                )
            } else {
                format!("{}:{}", entry.agent_id, entry.status)
            }
        })
        .collect::<Vec<_>>();
    display.sort();
    display.dedup();

    let mut active_agent_ids = agents
        .iter()
        .filter(|entry| matches!(entry.status.as_str(), "busy" | "idle" | "waiting"))
        .map(|entry| entry.agent_id.clone())
        .filter(|agent_id| !agent_id.trim().is_empty())
        .collect::<Vec<_>>();
    active_agent_ids.sort();
    active_agent_ids.dedup();

    Some(AgentPresenceSnapshot {
        agents,
        active_agent_ids,
        summary: Some(if display.is_empty() {
            "agents=none".into()
        } else {
            format!("agents={} [{}]", display.len(), display.join(", "))
        }),
    })
}

pub(super) fn load_project_supervision_snapshot(
    runtime_home: &str,
) -> Option<ProjectSupervisionSnapshot> {
    let path = Path::new(runtime_home).join("runtime/current/current_project_supervision.json");
    let Ok(content) = fs::read_to_string(path) else {
        return None;
    };
    let Ok(snapshot) = serde_json::from_str::<StoredProjectSupervisionSnapshot>(&content) else {
        return None;
    };
    let projects = snapshot
        .projects
        .iter()
        .map(|project| ProjectSupervisionSnapshotEntry {
            project_id: project.project_id.clone(),
            desired_action: project.desired_action.clone(),
        })
        .collect::<Vec<_>>();
    let mut actions = snapshot
        .projects
        .into_iter()
        .map(|project| format!("{}:{}", project.project_id, project.desired_action))
        .collect::<Vec<_>>();
    actions.sort();

    Some(ProjectSupervisionSnapshot {
        ready_count: snapshot.ready_count,
        resume_ready_count: snapshot.resume_ready_count,
        busy_count: snapshot.busy_count,
        waiting_count: snapshot.waiting_count,
        recover_needed_count: snapshot.recover_needed_count,
        projects,
        actions,
        summary: Some(format!(
            "ready={} resume_ready={} busy={} waiting={} recover_needed={}",
            snapshot.ready_count,
            snapshot.resume_ready_count,
            snapshot.busy_count,
            snapshot.waiting_count,
            snapshot.recover_needed_count
        )),
    })
}

pub(super) fn resolve_project_root(cwd: &str) -> Option<String> {
    let mut current = PathBuf::from(cwd);
    if !current.exists() {
        return None;
    }
    let mut nearest_cargo = None;
    loop {
        if current.join(".git").exists() {
            return Some(current.display().to_string());
        }
        if nearest_cargo.is_none() && current.join("Cargo.toml").exists() {
            nearest_cargo = Some(current.display().to_string());
        }
        if !current.pop() {
            return nearest_cargo;
        }
    }
}

pub(super) fn relativize_selected_paths(
    project_root: &str,
    selected_paths: &[String],
) -> Vec<String> {
    let root = Path::new(project_root);
    selected_paths
        .iter()
        .map(|path| {
            let candidate = Path::new(path);
            if candidate.is_absolute() {
                candidate
                    .strip_prefix(root)
                    .ok()
                    .map(|relative| relative.display().to_string())
                    .unwrap_or_else(|| path.clone())
            } else {
                path.clone()
            }
        })
        .collect()
}

pub(super) fn path_basename(path: &str) -> Option<String> {
    Path::new(path)
        .file_name()
        .and_then(|value| value.to_str())
        .map(str::to_string)
}

pub(super) fn sanitize_project_id(label: &str) -> String {
    let value = label
        .trim()
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                ch.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>()
        .trim_matches('-')
        .to_string();
    if value.is_empty() {
        "project".into()
    } else {
        value
    }
}
