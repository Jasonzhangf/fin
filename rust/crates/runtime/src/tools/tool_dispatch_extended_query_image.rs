use super::tool_dispatch::{
    ToolDispatchInput, ToolDispatchOutcome, failed_record, read_string, short_text,
};
use super::tool_dispatch as tool_dispatch;
use super::tool_dispatch_extended_patch::resolve_workspace_path;
use fin_contracts::{InputAttachmentSummary, ToolExecutionRecord};
use serde_json::{Value, json};
use std::{fs, path::Path};

pub(super) fn handle_view_image(
    outcome: &mut ToolDispatchOutcome,
    input: &ToolDispatchInput<'_>,
    tool_call_id: &str,
    arguments: &Value,
) -> bool {
    let requested = read_string(arguments, "path")
        .or_else(|| read_string(arguments, "attachment"))
        .or_else(|| read_string(arguments, "name"));
    let image = select_image_attachment(input, requested.as_deref())
        .map(|item| image_from_attachment(input, item))
        .unwrap_or_else(|| match requested.as_deref() {
            Some(path) => image_from_path(input, path),
            None => Err("no image attachment or path available for view_image".into()),
        });

    match image {
        Ok(summary) => {
            let output = format!(
                "image={} kind={} size={} dimensions={}",
                summary.label,
                summary.kind,
                summary
                    .size_bytes
                    .map(|value| format!("{value}B"))
                    .unwrap_or_else(|| "unknown".into()),
                summary
                    .dimensions
                    .clone()
                    .unwrap_or_else(|| "unknown".into())
            );
            outcome.tool_records.push(ToolExecutionRecord {
                tool_call_id: tool_call_id.into(),
                operation_id: input.operation_id.into(),
                trace_id: input.trace_id.into(),
                refs: input.refs.clone(),
                tool_name: "view_image".into(),
                tool_kind: "agent_tool".into(),
                title: "View Image Reference".into(),
                purpose: "inspect image attachment or local image file metadata".into(),
                target_kind: Some("image_reference".into()),
                target_ref: Some(summary.target_ref.clone()),
                input_summary: Some(requested.unwrap_or_else(|| "first_image_attachment".into())),
                output_summary: Some(output.clone()),
                status: "completed".into(),
                started_at: input.occurred_at.into(),
                ended_at: Some(input.occurred_at.into()),
                duration_ms: Some(0),
                side_effects: vec!["read_image_metadata".into()],
                artifact_refs: summary.artifact_refs,
                error_summary: None,
            });
            outcome.events.push((
                "tool.view_image_completed".into(),
                json!({
                    "tool_call_id": tool_call_id,
                    "label": summary.label,
                    "kind": summary.kind,
                    "size_bytes": summary.size_bytes,
                    "dimensions": summary.dimensions,
                    "target_ref": summary.target_ref,
                }),
            ));
            outcome
                .note_hints
                .push(format!("view_image {}", short_text(output.as_str(), 160)));
        }
        Err(error) => outcome.tool_records.push(failed_record(
            input,
            tool_call_id.into(),
            "view_image",
            error.as_str(),
        )),
    }
    true
}

#[derive(Debug, Clone)]
struct ImageSummary {
    label: String,
    kind: String,
    size_bytes: Option<u64>,
    dimensions: Option<String>,
    target_ref: String,
    artifact_refs: Vec<String>,
}

fn select_image_attachment<'a>(
    input: &'a ToolDispatchInput<'_>,
    requested: Option<&str>,
) -> Option<&'a InputAttachmentSummary> {
    let attachments = input.context.current_input.as_ref()?.attachments.as_slice();
    match requested {
        Some(value) => attachments
            .iter()
            .find(|item| attachment_matches(item, value)),
        None => attachments.iter().find(|item| is_image_attachment(item)),
    }
}

fn attachment_matches(item: &InputAttachmentSummary, requested: &str) -> bool {
    item.name.as_deref() == Some(requested)
        || item.local_path.as_deref() == Some(requested)
        || item.url.as_deref() == Some(requested)
        || item.attachment_id.as_deref() == Some(requested)
}

fn is_image_attachment(item: &InputAttachmentSummary) -> bool {
    item.kind.starts_with("image")
        || item
            .content_type
            .as_deref()
            .map(|value| value.starts_with("image/"))
            .unwrap_or(false)
        || item.name.as_deref().map(is_image_name).unwrap_or(false)
}

fn is_image_name(name: &str) -> bool {
    let lowered = name.to_ascii_lowercase();
    [".png", ".jpg", ".jpeg", ".gif", ".webp", ".bmp"]
        .iter()
        .any(|suffix| lowered.ends_with(suffix))
}

fn image_from_attachment(
    input: &ToolDispatchInput<'_>,
    item: &InputAttachmentSummary,
) -> Result<ImageSummary, String> {
    let target_ref = item
        .local_path
        .clone()
        .or_else(|| item.url.clone())
        .or_else(|| item.name.clone())
        .unwrap_or_else(|| "image_attachment".into());
    let size_bytes = item.size_bytes.or_else(|| {
        item.local_path.as_deref().and_then(|path| {
            image_from_path(input, path)
                .ok()
                .and_then(|value| value.size_bytes)
        })
    });
    Ok(ImageSummary {
        label: item.name.clone().unwrap_or_else(|| target_ref.clone()),
        kind: item
            .content_type
            .clone()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| item.kind.clone()),
        size_bytes,
        dimensions: match (item.width, item.height) {
            (Some(width), Some(height)) => Some(format!("{width}x{height}")),
            _ => None,
        },
        target_ref: target_ref.clone(),
        artifact_refs: vec![target_ref],
    })
}

fn image_from_path(input: &ToolDispatchInput<'_>, raw_path: &str) -> Result<ImageSummary, String> {
    let resolved = resolve_workspace_path(input, raw_path)?;
    let metadata = fs::metadata(&resolved).map_err(|error| {
        format!(
            "failed to stat image path '{}': {error}",
            resolved.display()
        )
    })?;
    Ok(ImageSummary {
        label: resolved
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("image")
            .to_string(),
        kind: guessed_image_kind(&resolved),
        size_bytes: Some(metadata.len()),
        dimensions: None,
        target_ref: resolved.display().to_string(),
        artifact_refs: vec![resolved.display().to_string()],
    })
}

fn guessed_image_kind(path: &Path) -> String {
    path.extension()
        .and_then(|value| value.to_str())
        .map(|ext| format!("image/{}", ext.to_ascii_lowercase()))
        .unwrap_or_else(|| "image/unknown".into())
}
