use fin_contracts::{MinimalContextView, ToolExecutionRecord};
use std::{fs, path::PathBuf};

pub(crate) fn render_current_tool_execution_history(
    context: &MinimalContextView,
    records: &[ToolExecutionRecord],
) -> Vec<String> {
    records
        .iter()
        .filter(|record| record.tool_name != "provider.call")
        .map(|record| render_tool_record(context, record))
        .collect()
}

pub(super) fn render_tool_record(
    context: &MinimalContextView,
    record: &ToolExecutionRecord,
) -> String {
    let mut lines = vec![format!(
        "tool_call_id={} tool={} status={} kind={}",
        record.tool_call_id, record.tool_name, record.status, record.tool_kind
    )];

    if !record.title.trim().is_empty() {
        lines.push(format!("title={}", record.title));
    }
    if !record.purpose.trim().is_empty() {
        lines.push(format!("purpose={}", record.purpose));
    }
    if let Some(target_kind) = record
        .target_kind
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    {
        lines.push(format!("target_kind={target_kind}"));
    }
    if let Some(target_ref) = record
        .target_ref
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    {
        lines.push(format!("target_ref={target_ref}"));
    }
    if let Some(input_summary) = record
        .input_summary
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    {
        lines.push(format!("record_input_summary={input_summary}"));
    }
    if let Some(output_summary) = record
        .output_summary
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    {
        lines.push(format!("record_output_summary={output_summary}"));
    }
    if let Some(error_summary) = record
        .error_summary
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    {
        lines.push(format!("record_error_summary={error_summary}"));
    }
    if let Some(duration_ms) = record.duration_ms {
        lines.push(format!("duration_ms={duration_ms}"));
    }
    if !record.side_effects.is_empty() {
        lines.push(format!("side_effects={}", record.side_effects.join(", ")));
    }
    if !record.artifact_refs.is_empty() {
        lines.push(format!("artifact_refs={}", record.artifact_refs.join(", ")));
    }
    if let Some(receipt) = load_authoritative_receipt(context, record) {
        lines.push("authoritative_receipt:".into());
        lines.push(indent_block(receipt.trim_end(), "  "));
    }

    lines.join("\n")
}

fn load_authoritative_receipt(
    context: &MinimalContextView,
    record: &ToolExecutionRecord,
) -> Option<String> {
    record.artifact_refs.iter().find_map(|artifact_ref| {
        if !is_authoritative_receipt_ref(artifact_ref) {
            return None;
        }
        resolve_artifact_path(context, artifact_ref)
            .and_then(|path| fs::read_to_string(path).ok())
            .map(|content| {
                format!(
                    "artifact_ref={artifact_ref}\n{}",
                    indent_block(content.trim_end(), "  ")
                )
            })
    })
}

fn is_authoritative_receipt_ref(artifact_ref: &str) -> bool {
    artifact_ref.contains("/exec_receipts/")
        || artifact_ref.contains("/write_stdin_receipts/")
        || artifact_ref.contains("/patch_receipts/")
}

fn resolve_artifact_path(context: &MinimalContextView, artifact_ref: &str) -> Option<PathBuf> {
    let as_path = PathBuf::from(artifact_ref);
    if as_path.is_absolute() {
        return Some(as_path);
    }
    context
        .project
        .as_ref()
        .and_then(|project| project.runtime_home.as_deref())
        .map(PathBuf::from)
        .map(|runtime_home| runtime_home.join(artifact_ref))
}

fn indent_block(value: &str, indent: &str) -> String {
    value
        .lines()
        .map(|line| format!("{indent}{line}"))
        .collect::<Vec<_>>()
        .join("\n")
}
