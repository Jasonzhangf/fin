use fin_config::{ProjectAgentMode, ProjectAgentStartupConfig, SystemConfig};

pub(crate) fn find_project_agent_config<'a>(
    system: &'a SystemConfig,
    project_id: Option<&str>,
    agent_name: Option<&str>,
) -> Option<&'a ProjectAgentStartupConfig> {
    system
        .runtime
        .startup
        .project_agents
        .iter()
        .find(|project| {
            project_id.is_some_and(|value| value == project.project_id)
                || agent_name.is_some_and(|value| value == project_agent_name(project))
        })
}

pub(crate) fn project_agent_name(project: &ProjectAgentStartupConfig) -> String {
    project
        .agent_name
        .as_deref()
        .and_then(sanitize_name_part)
        .unwrap_or_else(|| {
            sanitize_name_part(format!("project-{}", project.project_id).as_str())
                .unwrap_or_else(|| "project".into())
        })
}

pub(crate) fn project_mode_name(project: &ProjectAgentStartupConfig) -> String {
    match project.mode {
        ProjectAgentMode::Local => "local".into(),
        ProjectAgentMode::Remote => "remote".into(),
    }
}

fn sanitize_name_part(raw: &str) -> Option<String> {
    let sanitized = raw
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
    if sanitized.is_empty() {
        None
    } else {
        Some(sanitized)
    }
}
