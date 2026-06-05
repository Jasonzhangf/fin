use fin_contracts::{PeerContextBlock, ProjectContextBlock, RolePromptBlock};

pub(crate) fn render_role_prompt_lines(role_prompt: &RolePromptBlock) -> Vec<String> {
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

pub(crate) fn render_peer_lines(peer: &PeerContextBlock) -> Vec<String> {
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

pub(crate) fn render_project_lines(project: &ProjectContextBlock) -> Vec<String> {
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
    if !project.task_status_counts.is_empty() {
        lines.push(format!(
            "task_status_counts={}",
            project.task_status_counts.join(", ")
        ));
    }
    if !project.ready_task_ids.is_empty() {
        lines.push(format!(
            "ready_task_ids={}",
            project.ready_task_ids.join(", ")
        ));
    }
    if !project.submitted_task_ids.is_empty() {
        lines.push(format!(
            "submitted_task_ids={}",
            project.submitted_task_ids.join(", ")
        ));
    }
    if let Some(owner_loop_summary) = &project.owner_loop_summary {
        lines.push(format!("owner_loop={owner_loop_summary}"));
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
