use fin_contracts::{ToolExecutionRecord, ToolSemanticView};

pub fn semantic_views(records: &[ToolExecutionRecord]) -> Vec<ToolSemanticView> {
    records.iter().map(semantic_view).collect()
}

fn semantic_view(record: &ToolExecutionRecord) -> ToolSemanticView {
    if record.tool_name == "provider.call" {
        return provider_semantic_view(record);
    }
    let (category, verb) = classify_tool(record.tool_name.as_str());
    let object_kind = preferred(
        record.target_kind.as_deref(),
        inferred_object_kind(record.tool_name.as_str()),
    );
    let object_label = preferred(
        record.target_ref.as_deref(),
        inferred_object_label(record, object_kind.as_str()),
    );
    let summary = build_summary(record, verb.as_str(), object_label.as_str());
    let detail = build_detail(record);
    ToolSemanticView {
        tool_call_id: record.tool_call_id.clone(),
        operation_id: record.operation_id.clone(),
        tool_name: record.tool_name.clone(),
        category,
        verb,
        object_kind,
        object_label,
        summary,
        detail,
        status: record.status.clone(),
        started_at: record.started_at.clone(),
        ended_at: record.ended_at.clone(),
        artifact_refs: record.artifact_refs.clone(),
    }
}

fn provider_semantic_view(record: &ToolExecutionRecord) -> ToolSemanticView {
    let model_label = provider_model_label(record.target_ref.as_deref());
    let status = record.status.clone();
    let summary = match status.as_str() {
        "failed" => format!("调用模型 {} (failed)", short_text(&model_label)),
        "cancelled" => format!("调用模型 {} (cancelled)", short_text(&model_label)),
        "timed_out" => format!("调用模型 {} (timed out)", short_text(&model_label)),
        _ => format!("调用模型 {}", short_text(&model_label)),
    };
    let detail = Some(match status.as_str() {
        "failed" => "模型请求失败".into(),
        "cancelled" => "模型请求已取消".into(),
        "timed_out" => "模型请求超时".into(),
        _ => "模型响应已返回".into(),
    });
    ToolSemanticView {
        tool_call_id: record.tool_call_id.clone(),
        operation_id: record.operation_id.clone(),
        tool_name: record.tool_name.clone(),
        category: "model".into(),
        verb: "调用".into(),
        object_kind: "model".into(),
        object_label: model_label,
        summary,
        detail,
        status,
        started_at: record.started_at.clone(),
        ended_at: record.ended_at.clone(),
        artifact_refs: record.artifact_refs.clone(),
    }
}

fn build_detail(record: &ToolExecutionRecord) -> Option<String> {
    if record.status == "failed" {
        if let Some(detail) = record
            .output_summary
            .as_deref()
            .and_then(humanize_structured_failure_summary)
        {
            return Some(detail);
        }
        return record
            .error_summary
            .as_deref()
            .map(humanize_error_summary)
            .or_else(|| record.output_summary.as_deref().map(short_text))
            .or_else(|| record.input_summary.as_deref().map(short_text));
    }
    join_optional(
        record.input_summary.as_deref(),
        record.output_summary.as_deref(),
    )
}

fn classify_tool(tool_name: &str) -> (String, String) {
    let name = tool_name.to_ascii_lowercase();
    if matches_any(&name, &["read", "find", "list", "open", "cat"]) {
        return ("read".into(), "Explored".into());
    }
    if matches_any(&name, &["write", "edit", "patch", "apply"]) {
        return ("write".into(), "Edited".into());
    }
    if matches_any(&name, &["search", "grep", "query"]) {
        return ("search".into(), "Searched".into());
    }
    if matches_any(&name, &["exec", "command", "stdin"]) {
        return ("command".into(), "Ran".into());
    }
    if matches_any(&name, &["plan"]) {
        return ("plan".into(), "Updated".into());
    }
    if matches_any(&name, &["assign", "delegate"]) {
        return ("delegation".into(), "Delegated".into());
    }
    if matches_any(&name, &["peer", "daemon", "capability"]) {
        return ("peer".into(), "Inspected".into());
    }
    if matches_any(&name, &["mailbox"]) {
        return ("mailbox".into(), "Synced".into());
    }
    if matches_any(&name, &["wait", "remind", "sleep"]) {
        return ("wait".into(), "Scheduled".into());
    }
    if matches_any(&name, &["reasoning.stop", "stop"]) {
        return ("reasoning".into(), "Stopped".into());
    }
    ("other".into(), "Ran".into())
}

fn inferred_object_kind(tool_name: &str) -> &'static str {
    let name = tool_name.to_ascii_lowercase();
    if matches_any(
        &name,
        &["read", "find", "open", "write", "edit", "patch", "apply"],
    ) {
        "file"
    } else if matches_any(&name, &["search", "grep", "query"]) {
        "search_scope"
    } else if matches_any(&name, &["exec", "command", "stdin"]) {
        "command"
    } else if matches_any(&name, &["assign", "delegate", "peer", "daemon"]) {
        "peer"
    } else if matches_any(&name, &["wait", "remind", "sleep"]) {
        "timer"
    } else {
        "target"
    }
}

fn inferred_object_label(record: &ToolExecutionRecord, object_kind: &str) -> String {
    record
        .input_summary
        .as_deref()
        .or(record.output_summary.as_deref())
        .filter(|value| !value.trim().is_empty())
        .map(short_text)
        .unwrap_or_else(|| object_kind.to_string())
}

fn build_summary(record: &ToolExecutionRecord, verb: &str, object_label: &str) -> String {
    let target = short_text(object_label);
    let title = record.title.trim();
    match record.status.as_str() {
        "failed" => format!("{verb} {target} (失败)"),
        "cancelled" => format!("{verb} {target} (cancelled)"),
        "timed_out" => format!("{verb} {target} (timed out)"),
        _ if !title.is_empty() && title != "-" => format!("{verb} {target}"),
        _ => format!("{verb} {target}"),
    }
}

fn humanize_error_summary(value: &str) -> String {
    let trimmed = value.trim();
    if let Some(task_id) = trimmed.strip_prefix("task not found in runtime truth:") {
        return short_text(format!("任务不存在：{}", task_id.trim()).as_str());
    }
    short_text(trimmed)
}

fn humanize_structured_failure_summary(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if !trimmed.contains("failure_kind=") {
        return None;
    }
    let correction = trimmed
        .split("correction=")
        .nth(1)
        .and_then(|tail| tail.split(" · retry_hint=").next())
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let retry_hint = trimmed
        .split("retry_hint=")
        .nth(1)
        .map(str::trim)
        .filter(|value| !value.is_empty());
    match (correction, retry_hint) {
        (Some(correction), Some(retry_hint)) => Some(short_text(
            format!("{correction}；重试：{retry_hint}").as_str(),
        )),
        (Some(correction), None) => Some(short_text(correction)),
        (None, Some(retry_hint)) => Some(short_text(format!("重试：{retry_hint}").as_str())),
        (None, None) => Some(short_text(trimmed)),
    }
}

fn preferred(primary: Option<&str>, default_value: impl Into<String>) -> String {
    primary
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| default_value.into())
}

fn short_text(value: &str) -> String {
    let trimmed = value.trim();
    let mut chars = trimmed.chars();
    let shortened = chars.by_ref().take(96).collect::<String>();
    if chars.next().is_some() {
        format!("{shortened}…")
    } else if shortened.is_empty() {
        "-".into()
    } else {
        shortened
    }
}

fn join_optional(left: Option<&str>, right: Option<&str>) -> Option<String> {
    let left = left
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(short_text);
    let right = right
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(short_text);
    match (left, right) {
        (Some(a), Some(b)) if a == b => Some(a),
        (Some(a), Some(b)) => Some(format!("{a} → {b}")),
        (Some(a), None) => Some(a),
        (None, Some(b)) => Some(b),
        (None, None) => None,
    }
}

fn matches_any(name: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| name.contains(needle))
}

fn provider_model_label(target_ref: Option<&str>) -> String {
    let raw = target_ref
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("model");
    let without_endpoint = raw.split('@').next().unwrap_or(raw).trim();
    if without_endpoint.is_empty() {
        "model".into()
    } else {
        without_endpoint.into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn semantic_view_translates_exec_command_into_summary() {
        let record = ToolExecutionRecord {
            tool_call_id: "tool-1".into(),
            operation_id: "op-1".into(),
            trace_id: "trace-1".into(),
            tool_name: "exec_command".into(),
            title: "Execute Local Command".into(),
            purpose: "run local command".into(),
            input_summary: Some("cmd=cargo test".into()),
            output_summary: Some("exit_code=0".into()),
            status: "completed".into(),
            started_at: "2026-04-19T12:00:00+08:00".into(),
            ..ToolExecutionRecord::default()
        };
        let semantic = semantic_view(&record);
        assert_eq!(semantic.category, "command");
        assert_eq!(semantic.verb, "Ran");
        assert!(semantic.summary.contains("Ran"));
        assert!(semantic.object_label.contains("cmd=cargo test"));
    }

    #[test]
    fn provider_semantic_view_hides_prompt_and_base_url() {
        let record = ToolExecutionRecord {
            tool_call_id: "tool-provider-1".into(),
            operation_id: "op-1".into(),
            trace_id: "trace-1".into(),
            tool_name: "provider.call".into(),
            target_ref: Some(
                "ali-coding-plan.qwen3.6-plus @ https://coding.dashscope.aliyuncs.com/apps/anthropic/v1/messages"
                    .into(),
            ),
            input_summary: Some("用户原始提示词".into()),
            output_summary: Some("模型输出".into()),
            status: "completed".into(),
            started_at: "2026-04-20T09:00:00+08:00".into(),
            ..ToolExecutionRecord::default()
        };
        let semantic = semantic_view(&record);
        assert_eq!(semantic.category, "model");
        assert_eq!(semantic.object_label, "ali-coding-plan.qwen3.6-plus");
        assert!(!semantic.summary.contains("https://"));
        assert!(!semantic.summary.contains("用户原始提示词"));
        assert_eq!(semantic.detail.as_deref(), Some("模型响应已返回"));
    }

    #[test]
    fn failed_task_lookup_uses_error_summary_as_detail() {
        let record = ToolExecutionRecord {
            tool_call_id: "tool-task-1".into(),
            operation_id: "op-1".into(),
            trace_id: "trace-1".into(),
            tool_name: "project.task.status".into(),
            status: "failed".into(),
            input_summary: Some("task=task-missing".into()),
            error_summary: Some("task not found in runtime truth: task-missing".into()),
            ..ToolExecutionRecord::default()
        };
        let semantic = semantic_view(&record);
        assert!(semantic.summary.contains("失败"));
        assert_eq!(semantic.detail.as_deref(), Some("任务不存在：task-missing"));
    }

    #[test]
    fn failed_semantic_view_prefers_structured_retry_hint() {
        let record = ToolExecutionRecord {
            tool_call_id: "tool-fail-1".into(),
            operation_id: "op-1".into(),
            trace_id: "trace-1".into(),
            tool_name: "exec_command".into(),
            status: "failed".into(),
            input_summary: Some("cmd=exit 7".into()),
            output_summary: Some("failure_kind=command_non_zero_exit · correction=shell command completed with a non-zero exit status · retry_hint=inspect stdout/stderr in the receipt, correct the command or environment, and retry only after the non-zero exit cause is addressed".into()),
            error_summary: Some("command exited with non-zero status: 7".into()),
            ..ToolExecutionRecord::default()
        };
        let semantic = semantic_view(&record);
        assert!(semantic.summary.contains("失败"));
        assert!(
            semantic
                .detail
                .as_deref()
                .unwrap_or_default()
                .contains("重试")
        );
        assert!(
            semantic
                .detail
                .as_deref()
                .unwrap_or_default()
                .contains("stdout/stderr")
        );
    }
}
