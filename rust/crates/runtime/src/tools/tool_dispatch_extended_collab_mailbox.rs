use super::tool_dispatch::{
    ToolDispatchInput, ToolDispatchOutcome, failed_record, read_bool, read_string, read_u64,
    runtime_home_from_context, short_text,
};
use super::tool_dispatch as tool_dispatch;
use fin_contracts::ToolExecutionRecord;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{fs, path::Path};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
struct MailboxMessage {
    message_id: String,
    from_peer_id: String,
    target_peer_id: String,
    message: Value,
    created_at: String,
}

pub(super) fn handle_mailbox_send(
    outcome: &mut ToolDispatchOutcome,
    input: &ToolDispatchInput<'_>,
    tool_call_id: &str,
    arguments: &Value,
) -> bool {
    let target_worker_id = read_string(arguments, "target_worker_id");
    let Some(target_peer_id) = read_string(arguments, "target_peer_id")
        .or_else(|| read_string(arguments, "peer_id"))
        .or_else(|| {
            target_worker_id
                .as_ref()
                .map(|value| format!("local-{value}"))
        })
    else {
        outcome.tool_records.push(failed_record(
            input,
            tool_call_id.into(),
            "mailbox.send",
            "missing required argument: target_peer_id or target_worker_id",
        ));
        return true;
    };
    let message = extract_message(arguments)
        .unwrap_or_else(|| Value::String("(empty mailbox message from runtime tool)".into()));
    let Some(runtime_home) = runtime_home_from_context(input.context) else {
        outcome.tool_records.push(failed_record(
            input,
            tool_call_id.into(),
            "mailbox.send",
            "missing context.project.runtime_home, cannot persist mailbox",
        ));
        return true;
    };

    let inbox_path = runtime_home.join(format!("runtime/mailbox/{target_peer_id}/inbox.json"));
    let mut inbox = match read_json::<Vec<MailboxMessage>>(&inbox_path) {
        Ok(Some(items)) => items,
        Ok(None) => Vec::new(),
        Err(err) => {
            outcome.tool_records.push(failed_record(
                input,
                tool_call_id.into(),
                "mailbox.send",
                format!("failed to load inbox: {err}").as_str(),
            ));
            return true;
        }
    };

    let message_id = format!("mbx-{}-{tool_call_id}", input.operation_id);
    let from_peer_id = input
        .refs
        .worker_id
        .clone()
        .unwrap_or_else(|| "peer-local".into());
    inbox.push(MailboxMessage {
        message_id: message_id.clone(),
        from_peer_id: from_peer_id.clone(),
        target_peer_id: target_peer_id.clone(),
        message: message.clone(),
        created_at: input.occurred_at.into(),
    });
    if let Err(err) = write_json(&inbox_path, &inbox) {
        outcome.tool_records.push(failed_record(
            input,
            tool_call_id.into(),
            "mailbox.send",
            format!("failed to persist inbox: {err}").as_str(),
        ));
        return true;
    }

    outcome.tool_records.push(ToolExecutionRecord {
        tool_call_id: tool_call_id.into(),
        operation_id: input.operation_id.into(),
        trace_id: input.trace_id.into(),
        refs: input.refs.clone(),
        tool_name: "mailbox.send".into(),
        tool_kind: "agent_tool".into(),
        title: "Mailbox Send".into(),
        purpose: "enqueue one async collaboration message for target peer".into(),
        target_kind: Some("peer_mailbox".into()),
        target_ref: Some(target_peer_id.clone()),
        input_summary: Some(format!(
            "target_peer_id={target_peer_id}{}{}, message={}",
            target_worker_id
                .as_ref()
                .map(|value| format!(", target_worker_id={value}"))
                .unwrap_or_default(),
            if target_worker_id.is_some() && target_peer_id.starts_with("local-") {
                ", route=worker_local_alias"
            } else {
                ""
            },
            short_text(&message.to_string(), 120)
        )),
        output_summary: Some(format!("enqueued mailbox message: {message_id}")),
        status: "completed".into(),
        started_at: input.occurred_at.into(),
        ended_at: Some(input.occurred_at.into()),
        duration_ms: Some(0),
        side_effects: vec!["append_mailbox_queue".into()],
        artifact_refs: vec![relative_artifact(input.context, &inbox_path)],
        error_summary: None,
    });
    outcome.events.push((
        "mailbox.message_enqueued".into(),
        json!({
            "tool_call_id": tool_call_id,
            "message_id": message_id,
            "from_peer_id": from_peer_id,
            "target_peer_id": target_peer_id,
            "target_worker_id": target_worker_id,
        }),
    ));
    outcome
        .note_hints
        .push(format!("mailbox.send queued message for {target_peer_id}"));
    true
}

pub(super) fn handle_mailbox_poll(
    outcome: &mut ToolDispatchOutcome,
    input: &ToolDispatchInput<'_>,
    tool_call_id: &str,
    arguments: &Value,
) -> bool {
    let worker_id = read_string(arguments, "worker_id");
    let peer_id = read_string(arguments, "peer_id")
        .or_else(|| worker_id.as_ref().map(|value| format!("local-{value}")))
        .or_else(|| input.refs.worker_id.clone())
        .unwrap_or_else(|| "peer-local".into());
    let limit = read_u64(arguments, "limit").unwrap_or(20) as usize;
    let consume = read_bool(arguments, "consume").unwrap_or(false);

    let Some(runtime_home) = runtime_home_from_context(input.context) else {
        outcome.tool_records.push(failed_record(
            input,
            tool_call_id.into(),
            "mailbox.poll",
            "missing context.project.runtime_home, cannot read mailbox",
        ));
        return true;
    };
    let inbox_path = runtime_home.join(format!("runtime/mailbox/{peer_id}/inbox.json"));
    let mut inbox = match read_json::<Vec<MailboxMessage>>(&inbox_path) {
        Ok(Some(items)) => items,
        Ok(None) => Vec::new(),
        Err(err) => {
            outcome.tool_records.push(failed_record(
                input,
                tool_call_id.into(),
                "mailbox.poll",
                format!("failed to read inbox: {err}").as_str(),
            ));
            return true;
        }
    };

    let take_count = usize::min(limit, inbox.len());
    let selected = inbox.iter().take(take_count).cloned().collect::<Vec<_>>();
    if consume && take_count > 0 {
        inbox.drain(0..take_count);
        if let Err(err) = write_json(&inbox_path, &inbox) {
            outcome.tool_records.push(failed_record(
                input,
                tool_call_id.into(),
                "mailbox.poll",
                format!("failed to persist consumed inbox: {err}").as_str(),
            ));
            return true;
        }
    }

    let summary_ids = selected
        .iter()
        .take(3)
        .map(|item| item.message_id.clone())
        .collect::<Vec<_>>();
    outcome.tool_records.push(ToolExecutionRecord {
        tool_call_id: tool_call_id.into(),
        operation_id: input.operation_id.into(),
        trace_id: input.trace_id.into(),
        refs: input.refs.clone(),
        tool_name: "mailbox.poll".into(),
        tool_kind: "agent_tool".into(),
        title: "Mailbox Poll".into(),
        purpose: "read pending collaboration messages from peer mailbox".into(),
        target_kind: Some("peer_mailbox".into()),
        target_ref: Some(peer_id.clone()),
        input_summary: Some(format!(
            "peer_id={peer_id}{}{}, limit={limit}, consume={consume}",
            worker_id
                .as_ref()
                .map(|value| format!(", worker_id={value}"))
                .unwrap_or_default(),
            if worker_id.is_some() && peer_id.starts_with("local-") {
                ", route=worker_local_alias"
            } else {
                ""
            }
        )),
        output_summary: Some(format!(
            "messages={}, remaining={}, ids={}{}",
            selected.len(),
            inbox.len(),
            summary_ids.join(", "),
            if selected.len() > summary_ids.len() {
                ", ..."
            } else {
                ""
            }
        )),
        status: "completed".into(),
        started_at: input.occurred_at.into(),
        ended_at: Some(input.occurred_at.into()),
        duration_ms: Some(0),
        side_effects: if consume {
            vec!["consume_mailbox_queue".into()]
        } else {
            vec!["read_mailbox_queue".into()]
        },
        artifact_refs: vec![relative_artifact(input.context, &inbox_path)],
        error_summary: None,
    });
    outcome.events.push((
        "mailbox.polled".into(),
        json!({
            "tool_call_id": tool_call_id,
            "peer_id": peer_id,
            "worker_id": worker_id,
            "returned_count": selected.len(),
            "remaining_count": inbox.len(),
            "consume": consume,
        }),
    ));
    true
}

fn extract_message(arguments: &Value) -> Option<Value> {
    let object = arguments.as_object()?;
    object
        .get("message")
        .cloned()
        .or_else(|| object.get("content").cloned())
}

fn read_json<T: for<'de> Deserialize<'de>>(path: &Path) -> Result<Option<T>, String> {
    match fs::read_to_string(path) {
        Ok(content) => serde_json::from_str::<T>(&content)
            .map(Some)
            .map_err(|err| err.to_string()),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(err) => Err(err.to_string()),
    }
}

fn write_json<T: Serialize>(path: &Path, value: &T) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|err| err.to_string())?;
    }
    fs::write(
        path,
        serde_json::to_vec_pretty(value).map_err(|err| err.to_string())?,
    )
    .map_err(|err| err.to_string())
}

fn relative_artifact(context: &fin_contracts::MinimalContextView, absolute: &Path) -> String {
    runtime_home_from_context(context)
        .and_then(|home| {
            absolute
                .strip_prefix(home)
                .ok()
                .map(|value| value.to_string_lossy().to_string())
        })
        .unwrap_or_else(|| absolute.display().to_string())
}
