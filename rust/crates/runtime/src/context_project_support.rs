use fin_contracts::ProjectRef;
use serde::Deserialize;
use std::{fs, path::{Path, PathBuf}};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct ProjectRegistryContextSnapshot {
    pub(super) active_projects: Vec<ProjectRef>,
    pub(super) projects: Vec<ProjectRef>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
struct StoredProjectRegistryEntry {
    project_id: String,
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
    }
}

fn project_ref_from_registry(entry: &StoredProjectRegistryEntry) -> ProjectRef {
    ProjectRef {
        project_id: entry.project_id.clone(),
        label: path_basename(entry.project_root.as_deref().unwrap_or(entry.project_id.as_str()))
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

pub(super) fn relativize_selected_paths(project_root: &str, selected_paths: &[String]) -> Vec<String> {
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
