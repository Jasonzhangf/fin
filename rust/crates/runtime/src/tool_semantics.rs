use fin_contracts::{ToolExecutionRecord, ToolSemanticView};

pub(crate) fn semantic_views(records: &[ToolExecutionRecord]) -> Vec<ToolSemanticView> {
    records.iter().map(semantic_view).collect()
}

fn semantic_view(record: &ToolExecutionRecord) -> ToolSemanticView {
    if record.tool_name == "provider.call" {
        return provider_semantic_view(record);
    }
    let (category, verb) = classify_tool(record.tool_name.as_str());
    let object_kind = preferred(
        record.target_kind.as_deref(),
        fallback_object_kind(record.tool_name.as_str()),
    );
    let object_label = preferred(
        record.target_ref.as_deref(),
        fallback_object_label(record, object_kind.as_str()),
    );
    let summary = build_summary(record, verb.as_str(), object_label.as_str());
    let detail = join_optional(
        record.input_summary.as_deref(),
        record.output_summary.as_deref(),
    );
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
        "failed" => join_provider_detail("模型请求失败", record.output_summary.as_deref()),
        "cancelled" => join_provider_detail("模型请求已取消", record.output_summary.as_deref()),
        "timed_out" => join_provider_detail("模型请求超时", record.output_summary.as_deref()),
        _ => join_provider_detail("模型响应已返回", record.output_summary.as_deref()),
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

fn join_provider_detail(prefix: &str, usage_summary: Option<&str>) -> String {
    usage_summary
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| format!("{prefix} · {value}"))
        .unwrap_or_else(|| prefix.to_string())
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

fn fallback_object_kind(tool_name: &str) -> &'static str {
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

fn fallback_object_label(record: &ToolExecutionRecord, object_kind: &str) -> String {
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
        "failed" => format!("{verb} {target} (failed)"),
        "cancelled" => format!("{verb} {target} (cancelled)"),
        "timed_out" => format!("{verb} {target} (timed out)"),
        _ if !title.is_empty() && title != "-" => format!("{verb} {target}"),
        _ => format!("{verb} {target}"),
    }
}

fn preferred(primary: Option<&str>, fallback: impl Into<String>) -> String {
    primary
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| fallback.into())
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
        assert_eq!(
            semantic.detail.as_deref(),
            Some("模型响应已返回 · 模型输出")
        );
    }
}
