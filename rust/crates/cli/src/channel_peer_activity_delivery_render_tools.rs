use fin_contracts::ToolSemanticView;

pub(super) fn friendly_tool_action(
    action: &ToolSemanticView,
    detail: &str,
    disclose_task_list_signature: Option<&str>,
) -> Option<String> {
    match action.tool_name.as_str() {
        "project.task.list" => Some(format!(
            "查看: {}",
            shorten(
                humanize_project_task_list(action, detail, disclose_task_list_signature).as_str(),
                200
            )
        )),
        "agent.assign" => Some(format!(
            "派发: {}",
            shorten(humanize_agent_assign(detail).as_str(), 120)
        )),
        "mailbox.send" => Some(format!(
            "协作: {}",
            shorten(humanize_mailbox_send(detail).as_str(), 120)
        )),
        "mailbox.poll" => Some(format!(
            "协作: {}",
            shorten(humanize_mailbox_poll(detail).as_str(), 120)
        )),
        "daemon.ensure_peer" => Some(format!(
            "Peer: {}",
            shorten(humanize_ensure_peer(detail).as_str(), 120)
        )),
        "update_plan" => Some(format!(
            "计划: {}",
            shorten(humanize_update_plan(detail).as_str(), 120)
        )),
        _ => None,
    }
}

pub(super) fn task_list_signature(action: &ToolSemanticView) -> Option<String> {
    if action.tool_name != "project.task.list" || action.status != "completed" {
        return None;
    }
    let detail = action
        .detail
        .as_deref()
        .unwrap_or(action.object_label.as_str());
    let normalized = detail
        .rsplit("→")
        .next()
        .unwrap_or(detail)
        .trim()
        .to_string();
    (!normalized.is_empty()).then_some(normalized)
}

fn humanize_project_task_list(
    action: &ToolSemanticView,
    detail: &str,
    disclose_task_list_signature: Option<&str>,
) -> String {
    let count = named_value(detail, "tasks=")
        .or_else(|| named_value(action.object_label.as_str(), "tasks="))
        .unwrap_or("?");
    let tasks = extract_task_ids(detail);
    let should_disclose = disclose_task_list_signature
        .zip(task_list_signature(action))
        .is_some_and(|(expected, current)| expected == current);
    if !should_disclose || tasks.is_empty() {
        return format!("任务 {count} 条");
    }
    format!("任务 {count} 条：{}", tasks.join(", "))
}

fn extract_task_ids(detail: &str) -> Vec<String> {
    let Some(raw) = between(detail, "[", "]") else {
        return Vec::new();
    };
    raw.split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.split('@').next().unwrap_or(value).trim().to_string())
        .filter(|value| !value.is_empty())
        .collect()
}

fn humanize_agent_assign(detail: &str) -> String {
    let target = named_value(detail, "agent_name=")
        .or_else(|| named_value(detail, "target_agent="))
        .or_else(|| named_value(detail, "worker_alias="))
        .or_else(|| named_value(detail, "peer_alias="))
        .or_else(|| named_value(detail, "peer_id="))
        .or_else(|| last_arrow_target(detail))
        .unwrap_or("target");
    let task = between(detail, "task=", " →").unwrap_or_default();
    if task.is_empty() {
        format!("给 {target}")
    } else {
        format!("给 {target}：{task}")
    }
}

fn humanize_mailbox_send(detail: &str) -> String {
    let target = named_value(detail, "target_agent=")
        .or_else(|| named_value(detail, "agent_name="))
        .or_else(|| named_value(detail, "target_peer_id="))
        .or_else(|| named_value(detail, "peer_alias="))
        .or_else(|| last_arrow_target(detail))
        .unwrap_or("target");
    let message = message_text(detail);
    if message.is_empty() {
        format!("发给 {target}")
    } else {
        format!("发给 {target}：{message}")
    }
}

fn humanize_mailbox_poll(detail: &str) -> String {
    let target = named_value(detail, "agent_name=")
        .or_else(|| named_value(detail, "peer_id="))
        .unwrap_or("mailbox");
    let messages = named_value(detail, "messages=").unwrap_or("0");
    let remaining = named_value(detail, "remaining=").unwrap_or("0");
    format!("收取 {target}：{messages} 条，余 {remaining}")
}

fn humanize_ensure_peer(detail: &str) -> String {
    let target = named_value(detail, "agent_name=")
        .or_else(|| named_value(detail, "peer_alias="))
        .or_else(|| named_value(detail, "peer_id="))
        .or_else(|| last_arrow_target(detail))
        .unwrap_or("peer");
    format!("确保 {target} 在线")
}

fn humanize_update_plan(detail: &str) -> String {
    let steps = named_value(detail, "steps=").unwrap_or("?");
    let explanation = between(detail, "explanation=", " →")
        .or_else(|| between(detail, "explanation=", ","))
        .unwrap_or_default();
    if explanation.is_empty() {
        format!("{steps} 步")
    } else {
        format!("{steps} 步：{explanation}")
    }
}

fn message_text(detail: &str) -> String {
    let raw = between(detail, "message=", " →")
        .or_else(|| between(detail, "message_preview=", " →"))
        .unwrap_or_default();
    if raw.is_empty() {
        return String::new();
    }
    if let Ok(value) = serde_json::from_str::<serde_json::Value>(&raw) {
        if let Some(text) = value.get("text").and_then(serde_json::Value::as_str) {
            return text.trim().to_string();
        }
        if let Some(kind) = value.get("kind").and_then(serde_json::Value::as_str) {
            return kind.trim().to_string();
        }
    }
    if raw.starts_with('{') || raw.starts_with('[') {
        if let Some(text) = quoted_field(raw.as_str(), "text") {
            return text;
        }
        if let Some(kind) = quoted_field(raw.as_str(), "kind") {
            return kind;
        }
        return String::new();
    }
    raw
}

fn quoted_field(raw: &str, field: &str) -> Option<String> {
    let marker = format!("\"{field}\":\"");
    let tail = raw.split(&marker).nth(1)?;
    let value = tail.split('"').next()?.trim();
    (!value.is_empty()).then(|| value.to_string())
}

fn named_value<'a>(detail: &'a str, key: &str) -> Option<&'a str> {
    let tail = detail.split(key).nth(1)?;
    let end = tail.find([',', ';', ' ', '\n']).unwrap_or(tail.len());
    let value = tail[..end].trim();
    (!value.is_empty()).then_some(value)
}

fn between(detail: &str, start: &str, end: &str) -> Option<String> {
    let tail = detail.split(start).nth(1)?;
    let value = tail.split(end).next().unwrap_or(tail).trim();
    (!value.is_empty()).then_some(value.to_string())
}

fn last_arrow_target(detail: &str) -> Option<&str> {
    let target = detail.rsplit("->").next()?.trim();
    (!target.is_empty()).then_some(target)
}

fn shorten(value: &str, limit: usize) -> String {
    let trimmed = value.trim();
    let mut chars = trimmed.chars();
    let shortened = chars.by_ref().take(limit).collect::<String>();
    if chars.next().is_some() {
        format!("{shortened}…")
    } else if shortened.is_empty() {
        "-".into()
    } else {
        shortened
    }
}
