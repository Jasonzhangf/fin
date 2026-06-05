use super::tool_dispatch_extended_patch::{
    PatchApplyResult, display_artifact, replace_exact, resolve_workspace_path, write_parent,
};
use super::tool_dispatch as tool_dispatch;
use super::tool_dispatch::{ToolDispatchInput, read_string};
use serde_json::Value;
use std::fs;

#[derive(Debug, Clone)]
enum PatchOperation {
    Add {
        path: String,
        lines: Vec<PatchLine>,
    },
    Update {
        path: String,
        move_to: Option<String>,
        hunks: Vec<PatchHunk>,
    },
    Delete {
        path: String,
    },
    Move {
        from: String,
        to: String,
    },
}

#[derive(Debug, Clone, Default)]
struct PatchHunk {
    lines: Vec<PatchLine>,
}

#[derive(Debug, Clone)]
struct PatchLine {
    prefix: char,
    content: String,
}

pub(super) fn apply_v4a_mode(
    input: &ToolDispatchInput<'_>,
    arguments: &Value,
) -> Result<PatchApplyResult, String> {
    let Some(patch_text) =
        read_string(arguments, "patch").or_else(|| read_string(arguments, "patch_content"))
    else {
        return Err("missing required argument: patch".into());
    };
    let operations = parse_patch_operations(patch_text.as_str())?;
    if operations.is_empty() {
        return Err("patch did not contain any operations".into());
    }

    let mut applied = PatchApplyResult::default();
    for operation in operations {
        match operation {
            PatchOperation::Add { path, lines } => {
                let resolved = resolve_workspace_path(input, path.as_str())?;
                if resolved.exists() {
                    return Err(format!(
                        "patch add target already exists: {}",
                        resolved.display()
                    ));
                }
                let content = render_add_lines(&lines)?;
                write_parent(&resolved)?;
                fs::write(&resolved, content).map_err(|err| {
                    format!(
                        "failed to write added patch target '{}': {err}",
                        resolved.display()
                    )
                })?;
                applied
                    .files_created
                    .push(display_artifact(input, &resolved));
            }
            PatchOperation::Update {
                path,
                move_to,
                hunks,
            } => {
                let resolved = resolve_workspace_path(input, path.as_str())?;
                let mut content = fs::read_to_string(&resolved).map_err(|err| {
                    format!(
                        "failed to read patch target '{}': {err}",
                        resolved.display()
                    )
                })?;
                for hunk in &hunks {
                    let (old_block, new_block) = render_hunk_blocks(hunk)?;
                    let (next, count) = replace_exact(
                        content.as_str(),
                        old_block.as_str(),
                        new_block.as_str(),
                        false,
                    )
                    .or_else(|_| {
                        replace_exact_with_trailing_newline(
                            content.as_str(),
                            &old_block,
                            &new_block,
                        )
                    })?;
                    content = next;
                    applied.replacement_count += count;
                }
                fs::write(&resolved, content).map_err(|err| {
                    format!(
                        "failed to write updated patch target '{}': {err}",
                        resolved.display()
                    )
                })?;
                let rendered_path = display_artifact(input, &resolved);
                if !applied.files_modified.contains(&rendered_path) {
                    applied.files_modified.push(rendered_path);
                }
                if let Some(move_target) = move_to {
                    let resolved_target = resolve_workspace_path(input, move_target.as_str())?;
                    write_parent(&resolved_target)?;
                    fs::rename(&resolved, &resolved_target).map_err(|err| {
                        format!(
                            "failed to move patch target '{}' -> '{}': {err}",
                            resolved.display(),
                            resolved_target.display()
                        )
                    })?;
                    applied.files_moved.push(format!(
                        "{} -> {}",
                        display_artifact(input, &resolved),
                        display_artifact(input, &resolved_target)
                    ));
                }
            }
            PatchOperation::Delete { path } => {
                let resolved = resolve_workspace_path(input, path.as_str())?;
                fs::remove_file(&resolved).map_err(|err| {
                    format!(
                        "failed to delete patch target '{}': {err}",
                        resolved.display()
                    )
                })?;
                applied
                    .files_deleted
                    .push(display_artifact(input, &resolved));
            }
            PatchOperation::Move { from, to } => {
                let resolved_from = resolve_workspace_path(input, from.as_str())?;
                let resolved_to = resolve_workspace_path(input, to.as_str())?;
                write_parent(&resolved_to)?;
                fs::rename(&resolved_from, &resolved_to).map_err(|err| {
                    format!(
                        "failed to move patch target '{}' -> '{}': {err}",
                        resolved_from.display(),
                        resolved_to.display()
                    )
                })?;
                applied.files_moved.push(format!(
                    "{} -> {}",
                    display_artifact(input, &resolved_from),
                    display_artifact(input, &resolved_to)
                ));
            }
        }
    }
    Ok(applied)
}

fn replace_exact_with_trailing_newline(
    content: &str,
    old_block: &str,
    new_block: &str,
) -> Result<(String, u64), String> {
    let old_with_newline = format!("{old_block}\n");
    let new_with_newline = format!("{new_block}\n");
    replace_exact(
        content,
        old_with_newline.as_str(),
        new_with_newline.as_str(),
        false,
    )
}

fn render_hunk_blocks(hunk: &PatchHunk) -> Result<(String, String), String> {
    let mut old_lines = Vec::new();
    let mut new_lines = Vec::new();
    for line in &hunk.lines {
        match line.prefix {
            ' ' => {
                old_lines.push(line.content.clone());
                new_lines.push(line.content.clone());
            }
            '-' => old_lines.push(line.content.clone()),
            '+' => new_lines.push(line.content.clone()),
            other => return Err(format!("unsupported patch line prefix: {other}")),
        }
    }
    if old_lines.is_empty() {
        return Err("update hunk must include context or removed lines".into());
    }
    Ok((old_lines.join("\n"), new_lines.join("\n")))
}

fn render_add_lines(lines: &[PatchLine]) -> Result<String, String> {
    let mut rendered = Vec::new();
    for line in lines {
        match line.prefix {
            '+' | ' ' => rendered.push(line.content.clone()),
            '-' => return Err("add file patch cannot contain removed lines".into()),
            other => return Err(format!("unsupported patch line prefix: {other}")),
        }
    }
    Ok(format!("{}\n", rendered.join("\n")))
}

fn parse_patch_operations(patch_text: &str) -> Result<Vec<PatchOperation>, String> {
    let mut operations = Vec::new();
    let mut active: Option<ActiveOp> = None;

    for raw_line in patch_text.lines() {
        if raw_line == "*** Begin Patch" || raw_line == "*** End Patch" {
            continue;
        }
        if let Some(path) = raw_line.strip_prefix("*** Add File: ") {
            flush_active(&mut active, &mut operations)?;
            active = Some(ActiveOp::Add {
                path: path.trim().into(),
                lines: Vec::new(),
            });
            continue;
        }
        if let Some(path) = raw_line.strip_prefix("*** Update File: ") {
            flush_active(&mut active, &mut operations)?;
            active = Some(ActiveOp::Update {
                path: path.trim().into(),
                move_to: None,
                hunks: Vec::new(),
                current_hunk: None,
            });
            continue;
        }
        if let Some(path) = raw_line.strip_prefix("*** Delete File: ") {
            flush_active(&mut active, &mut operations)?;
            operations.push(PatchOperation::Delete {
                path: path.trim().into(),
            });
            continue;
        }
        if let Some(rest) = raw_line.strip_prefix("*** Move File: ") {
            flush_active(&mut active, &mut operations)?;
            let Some((from, to)) = rest.split_once("->") else {
                return Err("invalid move file line; expected '*** Move File: old -> new'".into());
            };
            operations.push(PatchOperation::Move {
                from: from.trim().into(),
                to: to.trim().into(),
            });
            continue;
        }
        if let Some(target) = raw_line.strip_prefix("*** Move to: ") {
            match active.as_mut() {
                Some(ActiveOp::Update { move_to, .. }) => *move_to = Some(target.trim().into()),
                _ => return Err("found '*** Move to:' without active update operation".into()),
            }
            continue;
        }
        if raw_line.starts_with("@@") {
            match active.as_mut() {
                Some(ActiveOp::Update {
                    hunks,
                    current_hunk,
                    ..
                }) => {
                    if let Some(hunk) = current_hunk.take() {
                        hunks.push(hunk);
                    }
                    *current_hunk = Some(PatchHunk::default());
                }
                _ => return Err("found hunk marker outside update operation".into()),
            }
            continue;
        }

        match active.as_mut() {
            Some(ActiveOp::Add { lines, .. }) => lines.push(parse_patch_line(raw_line)),
            Some(ActiveOp::Update { current_hunk, .. }) => {
                current_hunk
                    .get_or_insert_with(PatchHunk::default)
                    .lines
                    .push(parse_patch_line(raw_line));
            }
            None if raw_line.trim().is_empty() => {}
            None => {
                return Err(format!(
                    "unexpected patch line outside operation: {raw_line}"
                ));
            }
        }
    }

    flush_active(&mut active, &mut operations)?;
    Ok(operations)
}

enum ActiveOp {
    Add {
        path: String,
        lines: Vec<PatchLine>,
    },
    Update {
        path: String,
        move_to: Option<String>,
        hunks: Vec<PatchHunk>,
        current_hunk: Option<PatchHunk>,
    },
}

fn flush_active(
    active: &mut Option<ActiveOp>,
    operations: &mut Vec<PatchOperation>,
) -> Result<(), String> {
    let Some(active_op) = active.take() else {
        return Ok(());
    };
    match active_op {
        ActiveOp::Add { path, lines } => operations.push(PatchOperation::Add { path, lines }),
        ActiveOp::Update {
            path,
            move_to,
            mut hunks,
            current_hunk,
        } => {
            if let Some(hunk) = current_hunk {
                hunks.push(hunk);
            }
            if hunks.is_empty() {
                return Err(format!("update patch has no hunks: {path}"));
            }
            operations.push(PatchOperation::Update {
                path,
                move_to,
                hunks,
            });
        }
    }
    Ok(())
}

fn parse_patch_line(raw_line: &str) -> PatchLine {
    if let Some(content) = raw_line.strip_prefix('+') {
        return PatchLine {
            prefix: '+',
            content: content.into(),
        };
    }
    if let Some(content) = raw_line.strip_prefix('-') {
        return PatchLine {
            prefix: '-',
            content: content.into(),
        };
    }
    if let Some(content) = raw_line.strip_prefix(' ') {
        return PatchLine {
            prefix: ' ',
            content: content.into(),
        };
    }
    PatchLine {
        prefix: ' ',
        content: raw_line.into(),
    }
}
