use super::ContextAssemblyInput;
use crate::WorkerRuntime;
use crate::context_project_support::{
    load_project_registry_context, path_basename, relativize_selected_paths, resolve_project_root,
    sanitize_project_id,
};
use crate::task_board_snapshot::build_task_board_context;
use fin_contracts::{
    DaemonStateSummary, PeerBindingSummary, PeerContextBlock, PeerDescriptorSummary,
    ProjectContextBlock, ProjectRef, RolePromptBlock,
};
use serde::Deserialize;
use std::fs;
use std::path::Path;
pub(super) fn build_project_block(
    worker: &WorkerRuntime,
    input: &ContextAssemblyInput,
) -> ProjectContextBlock {
    let registry = load_project_registry_context(input.runtime_home.as_deref());
    let project_root = input.cwd.as_deref().and_then(resolve_project_root);
    let inferred_label = project_root
        .as_deref()
        .and_then(path_basename)
        .or_else(|| input.project_label.clone())
        .unwrap_or_else(|| "project".into());
    let primary_project = ProjectRef {
        project_id: sanitize_project_id(&inferred_label),
        label: inferred_label.clone(),
        root: project_root.clone(),
        state: Some("active".into()),
    };
    let relative_selected_paths = project_root
        .as_deref()
        .map(|root| relativize_selected_paths(root, &input.selected_paths))
        .unwrap_or_default();
    let focus_summary = if !relative_selected_paths.is_empty() {
        Some(format!(
            "{} focused path(s): {}",
            relative_selected_paths.len(),
            relative_selected_paths.join(", ")
        ))
    } else if !input.selected_paths.is_empty() {
        Some(format!(
            "{} absolute focused path(s)",
            input.selected_paths.len()
        ))
    } else if let Some(cwd) = &input.cwd {
        Some(format!("cwd-scoped reasoning at {cwd}"))
    } else {
        Some("session-scoped reasoning without explicit path focus".into())
    };
    let role_id = worker.policy.role.role_id.as_str();
    let primary_project = if role_id == "system" {
        registry
            .active_projects
            .iter()
            .chain(registry.projects.iter())
            .find(|project| project.project_id == sanitize_project_id(&inferred_label))
            .cloned()
            .unwrap_or(primary_project)
    } else {
        primary_project
    };
    let active_projects = if role_id == "system" && !registry.active_projects.is_empty() {
        registry.active_projects.clone()
    } else {
        vec![primary_project.clone()]
    };
    let projects = if role_id == "system" && !registry.projects.is_empty() {
        registry.projects.clone()
    } else {
        vec![primary_project.clone()]
    };
    let task_board = build_task_board_context(
        input.runtime_home.as_deref(),
        input.refs.session_id.as_deref(),
        input.refs.task_id.as_deref(),
    );
    let base_scope = if role_id == "system" {
        format!(
            "backlog-first orchestration over the current session/project surface; active_projects={} registered_projects={}",
            active_projects.len(),
            projects.len()
        )
    } else {
        "current project-scoped execution and review input".into()
    };
    let scope_summary = Some(match task_board.task_board_summary.as_deref() {
        Some(summary) => format!("{base_scope}; {summary}"),
        None => base_scope,
    });
    let focus_summary = if role_id == "system" {
        Some(match focus_summary {
            Some(value) => format!("current frontstage focus with project visibility: {value}"),
            None => "current frontstage focus without explicit path selection".into(),
        })
    } else {
        focus_summary
    };
    ProjectContextBlock {
        primary_project: Some(primary_project.clone()),
        active_projects,
        projects,
        project_label: input.project_label.clone(),
        project_root,
        runtime_home: input.runtime_home.clone(),
        cwd: input.cwd.clone(),
        selected_paths: input.selected_paths.clone(),
        relative_selected_paths,
        scope_summary,
        focus_summary,
        active_task_id: task_board.active_task_id,
        task_board_summary: task_board.task_board_summary,
        known_task_ids: task_board.known_task_ids,
        active_agent_ids: registry.active_agent_ids,
        agent_presence_summary: registry.agent_presence_summary,
        supervision_actions: registry.supervision_actions,
        project_supervision_summary: registry.project_supervision_summary,
        assignment_queue_summary: registry.assignment_queue_summary,
        mailbox_summary: registry.mailbox_summary,
    }
}
pub(super) fn build_peer_block(
    worker: &WorkerRuntime,
    input: &ContextAssemblyInput,
) -> PeerContextBlock {
    let local_peer_id = format!("local-{}", worker.worker_id);
    let role_id = worker.policy.role.role_id.as_str();
    let mut peers = vec![PeerDescriptorSummary {
        peer_id: local_peer_id.clone(),
        label: format!(
            "{} [{}]",
            worker.agent_id.as_str(),
            role_id.replace('_', " ")
        ),
        peer_kind: inferred_peer_kind(role_id).into(),
        presence_state: "local_only".into(),
        health_state: Some("unknown".into()),
        capability_ids: vec![
            "peer.list".into(),
            "peer.describe".into(),
            "daemon.ensure_peer".into(),
        ],
        supports_session_binding: true,
        supports_agentic_execution: !matches!(role_id, "channel_gateway"),
    }];
    let ensured_peers = load_ensured_peers(input, &local_peer_id);
    peers.extend(ensured_peers);
    let active_peer_ids = peers
        .iter()
        .map(|peer| peer.peer_id.clone())
        .collect::<Vec<_>>();
    let binding_scope = if input.refs.task_id.is_some() {
        Some("task".into())
    } else if input.refs.session_id.is_some() {
        Some("session".into())
    } else {
        Some("turn".into())
    };
    let binding_state = if matches!(role_id, "system" | "system_agent") {
        Some("controller_local_only".into())
    } else {
        Some("local_execution_only".into())
    };
    PeerContextBlock {
        topology_summary: Some(peer_topology_summary(peers.len())),
        active_peer_ids,
        peers,
        binding: Some(PeerBindingSummary {
            owner_peer_id: Some(if matches!(role_id, "system" | "system_agent") {
                local_peer_id.clone()
            } else {
                "system-agent".into()
            }),
            bound_peer_id: Some(local_peer_id),
            binding_scope,
            binding_state,
            lease_ttl_ms: None,
            rebind_hint: Some("placeholder binding until peer plane is wired".into()),
        }),
        daemon: Some(DaemonStateSummary {
            daemon_id: Some("daemon-local".into()),
            supervision_state: Some(daemon_supervision_state(input)),
            status_summary: Some(daemon_status_summary(input)),
        }),
        routing_hints: routing_hints(input),
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
struct StoredPeerStateRecord {
    peer_id: String,
    peer_kind: String,
    lifecycle_state: String,
    last_heartbeat_at: String,
    reconnect_backoff_ms: u64,
}
fn load_ensured_peers(
    input: &ContextAssemblyInput,
    local_peer_id: &str,
) -> Vec<PeerDescriptorSummary> {
    let Some(runtime_home) = input
        .runtime_home
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    else {
        return Vec::new();
    };
    let state_dir = Path::new(runtime_home).join("runtime/peers/state");
    let Ok(entries) = fs::read_dir(&state_dir) else {
        return Vec::new();
    };
    let mut peers = entries
        .filter_map(Result::ok)
        .filter_map(|entry| fs::read_to_string(entry.path()).ok())
        .filter_map(|content| serde_json::from_str::<StoredPeerStateRecord>(&content).ok())
        .filter(|state| state.peer_id != local_peer_id)
        .map(|state| PeerDescriptorSummary {
            label: state.peer_id.replace('-', " "),
            peer_id: state.peer_id,
            peer_kind: state.peer_kind,
            presence_state: state.lifecycle_state,
            health_state: Some(format!(
                "heartbeat={} backoff_ms={}",
                state.last_heartbeat_at, state.reconnect_backoff_ms
            )),
            capability_ids: vec![
                "mailbox.send".into(),
                "mailbox.poll".into(),
                "agent.assign".into(),
            ],
            supports_session_binding: false,
            supports_agentic_execution: true,
        })
        .collect::<Vec<_>>();
    peers.sort_by(|left, right| left.peer_id.cmp(&right.peer_id));
    peers
}
fn peer_topology_summary(peer_count: usize) -> String {
    if peer_count <= 1 {
        "peer registry snapshot unavailable; running in local-only M1 placeholder mode".into()
    } else {
        format!(
            "local peer context plus {} ensured peer(s) loaded from runtime peer state",
            peer_count - 1
        )
    }
}
fn daemon_supervision_state(input: &ContextAssemblyInput) -> String {
    let Some(runtime_home) = input
        .runtime_home
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    else {
        return "not_attached".into();
    };
    let state_dir = Path::new(runtime_home).join("runtime/peers/state");
    match fs::read_dir(state_dir) {
        Ok(entries) => {
            if entries.filter_map(Result::ok).next().is_some() {
                "local_peer_state_visible".into()
            } else {
                "not_attached".into()
            }
        }
        Err(_) => "not_attached".into(),
    }
}

fn daemon_status_summary(input: &ContextAssemblyInput) -> String {
    let Some(runtime_home) = input
        .runtime_home
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    else {
        return "daemon ensure is contract-only in current M1 runtime".into();
    };
    let state_dir = Path::new(runtime_home).join("runtime/peers/state");
    match fs::read_dir(state_dir) {
        Ok(entries) => {
            let count = entries.filter_map(Result::ok).count();
            if count == 0 {
                "daemon ensure is contract-only in current M1 runtime".into()
            } else {
                format!("{count} ensured peer state file(s) visible to current context")
            }
        }
        Err(_) => "daemon ensure is contract-only in current M1 runtime".into(),
    }
}

fn routing_hints(input: &ContextAssemblyInput) -> Vec<String> {
    let mut hints = vec![
        "use peer.list and peer.describe to inspect topology before selecting routes".into(),
        "daemon.ensure_peer is placeholder-only until lifecycle supervisor is connected".into(),
    ];
    if daemon_supervision_state(input) == "local_peer_state_visible" {
        hints.push(
            "runtime peer state is visible; project can route bounded slices to ensured local workers"
                .into(),
        );
    }
    hints
}

pub(super) fn render_role_prompt_lines(role_prompt: &RolePromptBlock) -> Vec<String> {
    let mut lines = vec![
        format!("role={}", role_prompt.role_id),
        format!("current={}", role_prompt.current_prompt_summary),
    ];
    if !role_prompt.prompt_history.is_empty() {
        lines.push(format!(
            "history:\n- {}",
            role_prompt.prompt_history.join("\n- ")
        ));
    }
    if !role_prompt.prompt_lineage.is_empty() {
        lines.push(format!(
            "lineage:\n- {}",
            role_prompt.prompt_lineage.join("\n- ")
        ));
    }
    if !role_prompt.prompt_layers.is_empty() {
        lines.push(format!(
            "layers:\n- {}",
            role_prompt
                .prompt_layers
                .iter()
                .map(|layer| format!("{}: {}", layer.layer_id, layer.summary))
                .collect::<Vec<_>>()
                .join("\n- ")
        ));
    }
    if !role_prompt.prompt_modules.is_empty() {
        lines.push(format!(
            "modules:\n- {}",
            role_prompt
                .prompt_modules
                .iter()
                .map(|module| format!("{}: {}", module.module_id, module.summary))
                .collect::<Vec<_>>()
                .join("\n- ")
        ));
    }
    if !role_prompt.behavior_rules.is_empty() {
        lines.push(format!(
            "rules:\n- {}",
            role_prompt.behavior_rules.join("\n- ")
        ));
    }
    if !role_prompt.output_contract.is_empty() {
        lines.push(format!(
            "output_contract:\n- {}",
            role_prompt.output_contract.join("\n- ")
        ));
    }
    lines
}

pub(super) fn render_peer_lines(peer: &PeerContextBlock) -> Vec<String> {
    let mut lines = Vec::new();
    if let Some(summary) = &peer.topology_summary {
        lines.push(format!("topology={summary}"));
    }
    if !peer.active_peer_ids.is_empty() {
        lines.push(format!(
            "active_peer_ids={}",
            peer.active_peer_ids.join(", ")
        ));
    }
    if !peer.peers.is_empty() {
        lines.push(format!(
            "peers={}",
            peer.peers
                .iter()
                .map(|item| format!(
                    "{} [{}:{}]",
                    item.peer_id, item.peer_kind, item.presence_state
                ))
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    if let Some(binding) = &peer.binding {
        if let Some(state) = &binding.binding_state {
            lines.push(format!("binding_state={state}"));
        }
    }
    if let Some(daemon) = &peer.daemon {
        if let Some(state) = &daemon.supervision_state {
            lines.push(format!("daemon_state={state}"));
        }
    }
    lines
}

pub(super) fn render_project_lines(project: &ProjectContextBlock) -> Vec<String> {
    let mut lines = Vec::new();
    if let Some(primary_project) = &project.primary_project {
        lines.push(format!(
            "primary_project={} ({})",
            primary_project.label, primary_project.project_id
        ));
    }
    if !project.active_projects.is_empty() {
        lines.push(format!(
            "active_projects={}",
            project
                .active_projects
                .iter()
                .map(|project| project.label.clone())
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    if !project.projects.is_empty() {
        lines.push(format!(
            "projects={}",
            project
                .projects
                .iter()
                .map(|project| project.label.clone())
                .collect::<Vec<_>>()
                .join(", ")
        ));
    } else if let Some(label) = &project.project_label {
        lines.push(format!("project={label}"));
    }
    if let Some(project_root) = &project.project_root {
        lines.push(format!("project_root={project_root}"));
    }
    if let Some(cwd) = &project.cwd {
        lines.push(format!("cwd={cwd}"));
    }
    if !project.relative_selected_paths.is_empty() {
        lines.push(format!(
            "relative_selected_paths={}",
            project.relative_selected_paths.join(", ")
        ));
    } else if !project.selected_paths.is_empty() {
        lines.push(format!(
            "selected_paths={}",
            project.selected_paths.join(", ")
        ));
    }
    if let Some(focus_summary) = &project.focus_summary {
        lines.push(format!("focus={focus_summary}"));
    }
    if let Some(active_task_id) = &project.active_task_id {
        lines.push(format!("active_task={active_task_id}"));
    }
    if let Some(task_board_summary) = &project.task_board_summary {
        lines.push(format!("task_board={task_board_summary}"));
    }
    if !project.known_task_ids.is_empty() {
        lines.push(format!(
            "known_task_ids={}",
            project.known_task_ids.join(", ")
        ));
    }
    if !project.active_agent_ids.is_empty() {
        lines.push(format!(
            "active_agent_ids={}",
            project.active_agent_ids.join(", ")
        ));
    }
    if let Some(agent_presence_summary) = &project.agent_presence_summary {
        lines.push(format!("agent_presence={agent_presence_summary}"));
    }
    if !project.supervision_actions.is_empty() {
        lines.push(format!(
            "supervision_actions={}",
            project.supervision_actions.join(", ")
        ));
    }
    if let Some(project_supervision_summary) = &project.project_supervision_summary {
        lines.push(format!("project_supervision={project_supervision_summary}"));
    }
    if let Some(assignment_queue_summary) = &project.assignment_queue_summary {
        lines.push(format!("assignments={assignment_queue_summary}"));
    }
    if let Some(mailbox_summary) = &project.mailbox_summary {
        lines.push(format!("mailbox={mailbox_summary}"));
    }
    lines
}

fn inferred_peer_kind(role_id: &str) -> &'static str {
    match role_id {
        "channel_gateway" => "channel_gateway",
        _ => "agent",
    }
}
