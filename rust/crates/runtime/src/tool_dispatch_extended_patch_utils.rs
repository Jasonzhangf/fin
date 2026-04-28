use super::*;

pub(crate) fn patch_mode(arguments: &Value) -> String {
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

pub(crate) fn write_file_input_summary(arguments: &Value) -> String {
    let path = read_string(arguments, "path").unwrap_or_else(|| "<missing path>".into());
    let content = read_patch_string(arguments, "content").unwrap_or_default();
    format!("path={}, content_chars={}", path, content.chars().count())
}

pub(crate) fn read_patch_string(arguments: &Value, key: &str) -> Option<String> {
    let object = arguments.as_object()?;
    object.get(key).and_then(|value| match value {
        Value::String(raw) => Some(raw.clone()),
        _ => None,
    })
}

pub(crate) fn replace_exact(
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

pub(crate) fn resolve_workspace_path(
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

pub(crate) fn input_summary(arguments: &Value, mode: &str) -> String {
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

pub(crate) fn persist_patch_receipt(
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

pub(crate) fn primary_target_ref(applied: &PatchApplyResult) -> Option<String> {
    applied
        .files_modified
        .first()
        .cloned()
        .or_else(|| applied.files_created.first().cloned())
        .or_else(|| applied.files_deleted.first().cloned())
        .or_else(|| applied.files_moved.first().cloned())
}

pub(crate) fn display_artifact(input: &ToolDispatchInput<'_>, absolute: &Path) -> String {
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

pub(crate) fn write_parent(path: &Path) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|err| format!("failed to create directory '{}': {err}", parent.display()))?;
    }
    Ok(())
}
