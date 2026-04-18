use fin_contracts::{
    ProjectContextBlock, ProjectRef, RolePromptBlock, ToolCatalogBlock, ToolCatalogEntry,
};
use std::path::{Path, PathBuf};

use super::ContextAssemblyInput;

pub(super) fn build_tool_catalog_block() -> ToolCatalogBlock {
    ToolCatalogBlock {
        model_tools: Vec::new(),
        framework_tools: vec![
            framework_tool(
                "provider.call",
                "framework-owned provider execution boundary",
                "dispatch the compiled prompt to the configured provider and normalize the response",
                vec!["the framework has already assembled context and needs a provider round-trip".into()],
                vec!["the model is deciding whether it should call a tool itself".into()],
                "no model-visible input schema; framework passes compiled prompt + provider path".into(),
                "normalized provider response event + sanitized debug snapshot".into(),
                vec!["network request to external provider".into()],
                vec!["framework auto-runs provider.call after inference operation acceptance".into()],
            ),
            framework_tool(
                "session.materialize",
                "session artifact write + revision advance",
                "persist messages, contexts, digests, and revision pointers to session truth",
                vec!["a closure has produced artifacts that must become channel render truth".into()],
                vec!["the model wants to directly write UI-visible state".into()],
                "framework-owned session artifacts bundle".into(),
                "updated session files + revision advance".into(),
                vec!["writes session files under ~/.fin/sessions".into()],
                vec!["framework materializes session artifacts before Web reads them".into()],
            ),
            framework_tool(
                "event.append",
                "append-only runtime fact recording",
                "record operation lifecycle and provider facts as immutable events",
                vec!["runtime state changes or side effects must become facts".into()],
                vec!["a channel only needs a projection refresh".into()],
                "structured event payload".into(),
                "event row appended to stream.jsonl".into(),
                vec!["appends raw event data to session/runtime event streams".into()],
                vec!["framework emits started/completed/failed events automatically".into()],
            ),
            framework_tool(
                "progress.update",
                "structured progress snapshot emission",
                "publish the current execution phase, health hint, and next step",
                vec!["execution phase changes and observers need a new progress snapshot".into()],
                vec!["nothing changed in execution state".into()],
                "progress block".into(),
                "latest progress snapshot".into(),
                vec!["updates progress/latest.json".into()],
                vec!["framework updates progress after provider completion".into()],
            ),
            framework_tool(
                "execution_note.append",
                "framework note persistence for ongoing execution",
                "persist concise execution note for later digest merge and inspection",
                vec!["a closure or step yields a durable lesson / decision / next step".into()],
                vec!["the content is only transient chain-of-thought".into()],
                "execution note block".into(),
                "notes/latest.json + note refs".into(),
                vec!["writes note artifact visible to debug tools".into()],
                vec!["framework records execution note after provider response normalization".into()],
            ),
            framework_tool(
                "digest.finalize",
                "closure compression and continuity carry-over",
                "compress the closure result into continuity tail, summary, and artifact candidates",
                vec!["a closure reaches a stable stop and continuity must roll forward".into()],
                vec!["the turn was interrupted and not closure-complete".into()],
                "closure result bundle".into(),
                "digest artifact for history + future context rebuild".into(),
                vec!["writes digest artifact and continuity tail".into()],
                vec!["framework finalizes digest only on successful closure stop".into()],
            ),
        ],
        tool_selection_policy: vec![
            "only model_tools are eligible for model-selected tool use".into(),
            "framework_tools are runtime-owned capabilities and must not be hallucinated as direct tool calls".into(),
            "if model_tools is empty, answer directly using current context and do not fabricate tool execution".into(),
        ],
        disabled_tools: vec![
            "direct_fs_write".into(),
            "direct_channel_render".into(),
            "runtime_fact_mutation".into(),
        ],
        hard_guards: vec![
            "session artifacts are the only channel render truth".into(),
            "events are the only runtime fact truth".into(),
            "framework writes session files before UI consumes them".into(),
        ],
    }
}

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

fn framework_tool(
    name: &str,
    summary: &str,
    purpose: &str,
    when_to_use: Vec<String>,
    when_not_to_use: Vec<String>,
    input_schema_summary: String,
    output_schema_summary: String,
    side_effects: Vec<String>,
    example_uses: Vec<String>,
) -> ToolCatalogEntry {
    ToolCatalogEntry {
        tool_name: name.into(),
        kind: "framework_capability".into(),
        summary: summary.into(),
        purpose: purpose.into(),
        when_to_use,
        when_not_to_use,
        input_schema_summary,
        output_schema_summary,
        side_effects,
        example_uses,
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
