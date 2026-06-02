use super::*;

pub(super) fn string_field(value: &Value, keys: &[&str]) -> Option<String> {
    keys.iter().find_map(|key| {
        value
            .get(*key)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
    })
}

pub(super) fn decorate_channel_notice(
    runtime_home: &Path,
    text: &str,
    notice_kind: &str,
) -> Result<String, CliError> {
    let cleaned = text.trim();
    if cleaned.is_empty() {
        return Ok(String::new());
    }
    let (header, debug_kind) = match notice_kind {
        "instant_response" => ("⚙️ 系统通知", "system_notice"),
        _ => ("📡 QQ Channel Peer", "channel_notice"),
    };
    Ok(apply_sender_header(
        header,
        cleaned,
        qqbot_debug_enabled(runtime_home)?,
        format!(
            "[debug sender_kind={} notice_kind={}]",
            debug_kind, notice_kind
        )
        .as_str(),
    ))
}

pub(super) fn render_session_message_for_channel(
    message: &SessionMessageRecord,
    debug_enabled: bool,
) -> String {
    let cleaned = sanitize_text_channel_output(&message.content);
    if cleaned.trim().is_empty() {
        return String::new();
    }
    let debug_line = format!(
        "[debug sender_kind={} role_id={} agent_name={} source_kind={} role={} operation_id={}]",
        message.sender_kind.as_deref().unwrap_or("-"),
        message.role_id.as_deref().unwrap_or("-"),
        message.agent_name.as_deref().unwrap_or("-"),
        message.source_kind.as_deref().unwrap_or("-"),
        message.role,
        message.operation_id.as_deref().unwrap_or("-"),
    );
    apply_sender_header(
        sender_header_for_message(message).as_str(),
        cleaned.as_str(),
        debug_enabled,
        debug_line.as_str(),
    )
}

fn sender_header_for_message(message: &SessionMessageRecord) -> String {
    match message.sender_kind.as_deref() {
        Some("agent_reply") => format!("🤖 {}", message.display_name.as_deref().unwrap_or("Agent")),
        Some("system_notice") => "⚙️ 系统通知".into(),
        Some("system_command") => "⚙️ 系统命令".into(),
        Some("user_message") => format!("👤 {}", message.display_name.as_deref().unwrap_or("User")),
        _ => match message.role.as_str() {
            "assistant" => format!("🤖 {}", message.display_name.as_deref().unwrap_or("Agent")),
            "system" | "local_command" => "⚙️ 系统通知".into(),
            "user" => "👤 User".into(),
            _ => "💬 消息".into(),
        },
    }
}

fn apply_sender_header(header: &str, text: &str, debug_enabled: bool, debug_line: &str) -> String {
    let mut parts = vec![header.trim().to_string()];
    if debug_enabled {
        parts.push(debug_line.trim().to_string());
    }
    parts.push(text.trim().to_string());
    parts.join("\n")
}

pub(super) fn u64_field(value: &Value, keys: &[&str]) -> Option<u64> {
    keys.iter().find_map(|key| {
        value.get(*key).and_then(|field| {
            field
                .as_u64()
                .or_else(|| field.as_str().and_then(|raw| raw.parse::<u64>().ok()))
        })
    })
}

pub(super) fn u32_field(value: &Value, keys: &[&str]) -> Option<u32> {
    u64_field(value, keys).and_then(|value| u32::try_from(value).ok())
}

pub(super) fn deliver_pending_messages_for_all(
    runtime_home: &std::path::Path,
    stdin: &Arc<Mutex<Option<ChildStdin>>>,
) -> Result<(), CliError> {
    for conversation in list_conversations(runtime_home)? {
        let _ = deliver_pending_messages_for_target(
            runtime_home,
            stdin,
            &conversation.target,
            None,
            "periodic_scan",
        )?;
    }
    Ok(())
}

pub(super) fn remove_tag_block(input: &str, tag: &str) -> String {
    let open = format!("<{tag}>");
    let close = format!("</{tag}>");
    let mut output = input.to_string();
    while let Some(start) = output.find(&open) {
        let Some(rel_end) = output[start..].find(&close) else {
            output.replace_range(start.., "");
            break;
        };
        let end = start + rel_end + close.len();
        output.replace_range(start..end, "");
    }
    output
}
