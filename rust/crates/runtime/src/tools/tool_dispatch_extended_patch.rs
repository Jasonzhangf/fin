use super::tool_dispatch::{
    ToolDispatchInput, ToolDispatchOutcome, failed_record, read_bool, read_string,
    runtime_home_from_context, short_text,
};
use super::tool_dispatch_extended_patch_v4a;
use fin_contracts::ToolExecutionRecord;
use serde::Serialize;
use serde_json::{Value, json};
use std::{
    fs,
    path::{Component, Path, PathBuf},
};

#[derive(Debug, Clone, Serialize)]
struct PatchReceipt {
    tool_call_id: String,
    mode: String,
    arguments: Value,
    files_modified: Vec<String>,
    files_created: Vec<String>,
    files_deleted: Vec<String>,
    files_moved: Vec<String>,
    replacement_count: u64,
}

#[derive(Debug, Clone, Default)]
pub(super) struct PatchApplyResult {
    pub(super) files_modified: Vec<String>,
    pub(super) files_created: Vec<String>,
    pub(super) files_deleted: Vec<String>,
    pub(super) files_moved: Vec<String>,
    pub(super) replacement_count: u64,
}

pub(super) fn handle_apply_patch(
    outcome: &mut ToolDispatchOutcome,
    input: &ToolDispatchInput<'_>,
    tool_call_id: &str,
    arguments: &Value,
) -> bool {
    let mode = patch_mode(arguments);
    let result = match mode.as_str() {
        "replace" => apply_replace_mode(input, arguments),
        "patch" => tool_dispatch_extended_patch_v4a::apply_v4a_mode(input, arguments),
        other => Err(format!("unsupported apply_patch mode: {other}")),
    };

    match result {
        Ok(applied) => push_success(
            outcome,
            input,
            tool_call_id,
            arguments,
            mode.as_str(),
            applied,
        ),
        Err(error) => outcome.tool_records.push(failed_record(
            input,
            tool_call_id.into(),
            "apply_patch",
            error.as_str(),
        )),
    }
    true
}

fn push_success(
    outcome: &mut ToolDispatchOutcome,
    input: &ToolDispatchInput<'_>,
    tool_call_id: &str,
    arguments: &Value,
    mode: &str,
    applied: PatchApplyResult,
) {
    let mut artifact_refs =
        persist_patch_receipt(input, tool_call_id, arguments, mode, &applied).unwrap_or_default();
    artifact_refs.extend(applied.files_modified.iter().cloned());
    artifact_refs.extend(applied.files_created.iter().cloned());
    artifact_refs.extend(applied.files_deleted.iter().cloned());
    artifact_refs.extend(applied.files_moved.iter().cloned());

    outcome.tool_records.push(ToolExecutionRecord {
        tool_call_id: tool_call_id.into(),
        operation_id: input.operation_id.into(),
        trace_id: input.trace_id.into(),
        refs: input.refs.clone(),
        tool_name: "apply_patch".into(),
        tool_kind: "agent_tool".into(),
        title: "Apply Patch".into(),
        purpose: "apply deterministic text or file patch operations to workspace files".into(),
        target_kind: Some("workspace_file".into()),
        target_ref: primary_target_ref(&applied),
        input_summary: Some(input_summary(arguments, mode)),
        output_summary: Some(format!(
            "modified={}, created={}, deleted={}, moved={}, replacements={}",
            applied.files_modified.len(),
            applied.files_created.len(),
            applied.files_deleted.len(),
            applied.files_moved.len(),
            applied.replacement_count
        )),
        status: "completed".into(),
        started_at: input.occurred_at.into(),
        ended_at: Some(input.occurred_at.into()),
        duration_ms: Some(0),
        side_effects: vec!["write_workspace_file".into()],
        artifact_refs,
        error_summary: None,
    });
    outcome.events.push((
        "tool.apply_patch_completed".into(),
        json!({
            "tool_call_id": tool_call_id,
            "mode": mode,
            "files_modified": applied.files_modified,
            "files_created": applied.files_created,
            "files_deleted": applied.files_deleted,
            "files_moved": applied.files_moved,
            "replacement_count": applied.replacement_count,
        }),
    ));
    outcome.note_hints.push(format!(
        "apply_patch mode={} modified={} created={} deleted={} moved={}",
        mode,
        applied.files_modified.len(),
        applied.files_created.len(),
        applied.files_deleted.len(),
        applied.files_moved.len()
    ));
}

fn apply_replace_mode(
    input: &ToolDispatchInput<'_>,
    arguments: &Value,
) -> Result<PatchApplyResult, String> {
    let Some(path) = read_string(arguments, "path") else {
        return Err("missing required argument: path".into());
    };
    let Some(old_string) = read_patch_string(arguments, "old_string") else {
        return Err("missing required argument: old_string".into());
    };
    let Some(new_string) = read_patch_string(arguments, "new_string") else {
        return Err("missing required argument: new_string".into());
    };
    let replace_all = read_bool(arguments, "replace_all").unwrap_or(false);

    let resolved = resolve_workspace_path(input, path.as_str())?;
    if !resolved.exists() {
        if !old_string.is_empty() {
            return Err(format!(
                "patch target '{}' does not exist; old_string must be empty to create a new file",
                resolved.display()
            ));
        }
        write_parent(&resolved)?;
        fs::write(&resolved, new_string.as_str()).map_err(|err| {
            format!(
                "failed to create patch target '{}': {err}",
                resolved.display()
            )
        })?;
        return Ok(PatchApplyResult {
            files_created: vec![display_artifact(input, &resolved)],
            replacement_count: 1,
            ..PatchApplyResult::default()
        });
    }

    let original = fs::read_to_string(&resolved).map_err(|err| {
        format!(
            "failed to read patch target '{}': {err}",
            resolved.display()
        )
    })?;
    if old_string.is_empty() {
        if !original.is_empty() {
            return Err(
                "old_string may be empty only when creating a new file or replacing an empty file"
                    .into(),
            );
        }
        fs::write(&resolved, new_string.as_str()).map_err(|err| {
            format!(
                "failed to write patch target '{}': {err}",
                resolved.display()
            )
        })?;
        return Ok(PatchApplyResult {
            files_modified: vec![display_artifact(input, &resolved)],
            replacement_count: 1,
            ..PatchApplyResult::default()
        });
    }
    let (updated, replacement_count) = replace_exact(
        original.as_str(),
        old_string.as_str(),
        new_string.as_str(),
        replace_all,
    )?;
    fs::write(&resolved, updated).map_err(|err| {
        format!(
            "failed to write patch target '{}': {err}",
            resolved.display()
        )
    })?;

    Ok(PatchApplyResult {
        files_modified: vec![display_artifact(input, &resolved)],
        replacement_count,
        ..PatchApplyResult::default()
    })
}

fn patch_mode(arguments: &Value) -> String {
    read_string(arguments, "mode")
        .unwrap_or_else(|| {
            if arguments.get("patch").is_some() || arguments.get("patch_content").is_some() {
                "patch".into()
            } else {
                "replace".into()
            }
        })
        .to_ascii_lowercase()
}

fn read_patch_string(arguments: &Value, key: &str) -> Option<String> {
    let object = arguments.as_object()?;
    object.get(key).and_then(|value| match value {
        Value::String(raw) => Some(raw.clone()),
        _ => None,
    })
}

pub(super) fn replace_exact(
    original: &str,
    old_string: &str,
    new_string: &str,
    replace_all: bool,
) -> Result<(String, u64), String> {
    let match_count = original.matches(old_string).count() as u64;
    if match_count == 0 {
        return Err("old_string was not found in target file".into());
    }
    if !replace_all && match_count > 1 {
        return Err(format!(
            "old_string matched {match_count} times; set replace_all=true or make the target unique"
        ));
    }
    let updated = if replace_all {
        original.replace(old_string, new_string)
    } else {
        original.replacen(old_string, new_string, 1)
    };
    Ok((updated, if replace_all { match_count } else { 1 }))
}

pub(super) fn resolve_workspace_path(
    input: &ToolDispatchInput<'_>,
    raw_path: &str,
) -> Result<PathBuf, String> {
    let requested = PathBuf::from(raw_path);
    let absolute = if requested.is_absolute() {
        normalize_path(&requested)
    } else {
        let Some(base) = preferred_base(input) else {
            return Err(
                "apply_patch requires context.project.cwd or project_root for relative paths"
                    .into(),
            );
        };
        normalize_path(&base.join(requested))
    };

    if let Some(roots) = allowed_roots(input) {
        if !roots.iter().any(|root| absolute.starts_with(root)) {
            return Err(format!(
                "patch target '{}' is outside the current workspace scope",
                absolute.display()
            ));
        }
    }
    Ok(absolute)
}

fn preferred_base(input: &ToolDispatchInput<'_>) -> Option<PathBuf> {
    input.context.project.as_ref().and_then(|project| {
        project
            .cwd
            .as_ref()
            .or(project.project_root.as_ref())
            .map(PathBuf::from)
    })
}

fn allowed_roots(input: &ToolDispatchInput<'_>) -> Option<Vec<PathBuf>> {
    let mut roots = Vec::new();
    if let Some(project) = input.context.project.as_ref() {
        if let Some(cwd) = project.cwd.as_deref() {
            roots.push(normalize_path(Path::new(cwd)));
        }
        if let Some(project_root) = project.project_root.as_deref() {
            let normalized = normalize_path(Path::new(project_root));
            if !roots.contains(&normalized) {
                roots.push(normalized);
            }
        }
    }
    (!roots.is_empty()).then_some(roots)
}

fn normalize_path(path: &Path) -> PathBuf {
    let mut normalized = if path.is_absolute() {
        PathBuf::from("/")
    } else {
        PathBuf::new()
    };
    for component in path.components() {
        match component {
            Component::RootDir | Component::Prefix(_) | Component::CurDir => {}
            Component::ParentDir => {
                normalized.pop();
            }
            Component::Normal(segment) => normalized.push(segment),
        }
    }
    normalized
}

fn input_summary(arguments: &Value, mode: &str) -> String {
    if mode == "patch" {
        return format!(
            "mode=patch, patch={}",
            short_text(
                read_string(arguments, "patch")
                    .or_else(|| read_string(arguments, "patch_content"))
                    .unwrap_or_default()
                    .as_str(),
                120
            )
        );
    }

    format!(
        "mode=replace, path={}, replace_all={}, old={}, new={}",
        read_string(arguments, "path").unwrap_or_default(),
        read_bool(arguments, "replace_all").unwrap_or(false),
        short_text(
            read_string(arguments, "old_string")
                .unwrap_or_default()
                .as_str(),
            60
        ),
        short_text(
            read_string(arguments, "new_string")
                .unwrap_or_default()
                .as_str(),
            60
        )
    )
}

fn persist_patch_receipt(
    input: &ToolDispatchInput<'_>,
    tool_call_id: &str,
    arguments: &Value,
    mode: &str,
    applied: &PatchApplyResult,
) -> Result<Vec<String>, String> {
    let Some(runtime_home) = runtime_home_from_context(input.context) else {
        return Ok(Vec::new());
    };
    let path = runtime_home.join(format!("runtime/tools/patch_receipts/{tool_call_id}.json"));
    write_parent(&path)?;
    let receipt = PatchReceipt {
        tool_call_id: tool_call_id.into(),
        mode: mode.into(),
        arguments: arguments.clone(),
        files_modified: applied.files_modified.clone(),
        files_created: applied.files_created.clone(),
        files_deleted: applied.files_deleted.clone(),
        files_moved: applied.files_moved.clone(),
        replacement_count: applied.replacement_count,
    };
    fs::write(
        &path,
        serde_json::to_vec_pretty(&receipt).map_err(|err| err.to_string())?,
    )
    .map_err(|err| {
        format!(
            "failed to persist patch receipt '{}': {err}",
            path.display()
        )
    })?;
    Ok(vec![display_artifact(input, &path)])
}

fn primary_target_ref(applied: &PatchApplyResult) -> Option<String> {
    applied
        .files_modified
        .first()
        .cloned()
        .or_else(|| applied.files_created.first().cloned())
        .or_else(|| applied.files_deleted.first().cloned())
        .or_else(|| applied.files_moved.first().cloned())
}

pub(super) fn display_artifact(input: &ToolDispatchInput<'_>, absolute: &Path) -> String {
    runtime_home_from_context(input.context)
        .and_then(|runtime_home| {
            absolute
                .strip_prefix(runtime_home)
                .ok()
                .map(|relative| relative.to_string_lossy().to_string())
        })
        .or_else(|| {
            allowed_roots(input).and_then(|roots| {
                roots.into_iter().find_map(|root| {
                    absolute
                        .strip_prefix(root)
                        .ok()
                        .map(|relative| relative.to_string_lossy().to_string())
                })
            })
        })
        .unwrap_or_else(|| absolute.display().to_string())
}

pub(super) fn write_parent(path: &Path) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|err| format!("failed to create directory '{}': {err}", parent.display()))?;
    }
    Ok(())
}
