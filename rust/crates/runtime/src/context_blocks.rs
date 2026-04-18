use fin_contracts::{
    DaemonStateSummary, PeerBindingSummary, PeerContextBlock, PeerDescriptorSummary,
    ProjectContextBlock, ProjectRef, RolePromptBlock,
};
use std::path::{Path, PathBuf};

use super::ContextAssemblyInput;
use crate::WorkerRuntime;

pub(super) fn build_project_block(input: &ContextAssemblyInput) -> ProjectContextBlock {
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

    ProjectContextBlock {
        primary_project: Some(primary_project.clone()),
        active_projects: vec![primary_project.clone()],
        projects: vec![primary_project],
        project_label: input.project_label.clone(),
        project_root,
        runtime_home: input.runtime_home.clone(),
        cwd: input.cwd.clone(),
        selected_paths: input.selected_paths.clone(),
        relative_selected_paths,
        scope_summary: Some("current project/session scoped reasoning input".into()),
        focus_summary,
    }
}

pub(super) fn build_peer_block(
    worker: &WorkerRuntime,
    input: &ContextAssemblyInput,
) -> PeerContextBlock {
    let local_peer_id = format!("local-{}", worker.worker_id);
    let role_id = worker.policy.role.role_id.as_str();
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
        topology_summary: Some(
            "peer registry snapshot unavailable; running in local-only M1 placeholder mode".into(),
        ),
        active_peer_ids: vec![local_peer_id.clone()],
        peers: vec![PeerDescriptorSummary {
            peer_id: local_peer_id.clone(),
            label: format!("local {}", role_id.replace('_', " ")),
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
        }],
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
            supervision_state: Some("not_attached".into()),
            status_summary: Some("daemon ensure is contract-only in current M1 runtime".into()),
        }),
        routing_hints: vec![
            "use peer.list and peer.describe to inspect topology before selecting routes".into(),
            "daemon.ensure_peer is placeholder-only until lifecycle supervisor is connected".into(),
        ],
    }
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
    lines
}

fn inferred_peer_kind(role_id: &str) -> &'static str {
    match role_id {
        "channel_gateway" => "channel_gateway",
        _ => "agent",
    }
}
fn resolve_project_root(cwd: &str) -> Option<String> {
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

fn relativize_selected_paths(project_root: &str, selected_paths: &[String]) -> Vec<String> {
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

fn path_basename(path: &str) -> Option<String> {
    Path::new(path)
        .file_name()
        .and_then(|value| value.to_str())
        .map(str::to_string)
}

fn sanitize_project_id(label: &str) -> String {
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
