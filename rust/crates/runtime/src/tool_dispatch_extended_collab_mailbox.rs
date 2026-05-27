use crate::tool_dispatch::{
    ToolDispatchInput, ToolDispatchOutcome, failed_record, read_bool, read_string, read_u64,
    runtime_home_from_context, short_text,
};
use crate::{AgentControlStore, SendAgentInput};
use fin_contracts::ToolExecutionRecord;
use serde_json::{Value, json};
use std::path::Path;

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

    let message_id = format!("mbx-{}-{tool_call_id}", input.operation_id);
    let from_peer_id = input
        .refs
        .worker_id
        .clone()
        .unwrap_or_else(|| "peer-local".into());
    let store = AgentControlStore::new(&runtime_home);
    if let Err(err) = ensure_mailbox_identity(&store, &from_peer_id, "subagent")
        .and_then(|_| ensure_mailbox_identity(&store, &target_peer_id, "subagent"))
        .and_then(|_| {
            store.send_agent_input(SendAgentInput {
                message_id: message_id.clone(),
                from_agent_id: from_peer_id.clone(),
                to_agent_id: target_peer_id.clone(),
                thread_id: None,
                task_id: input.refs.task_id.clone(),
                trigger_turn: false,
                payload: message.clone(),
            })
        })
    {
        outcome.tool_records.push(failed_record(
            input,
            tool_call_id.into(),
            "mailbox.send",
            format!("failed to enqueue agent mailbox message: {err}").as_str(),
        ));
        return true;
    }
    let inbox_path = runtime_home.join(format!(
        "runtime/agents/control/mailbox/{target_peer_id}/inbox.json"
    ));

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
    let store = AgentControlStore::new(&runtime_home);
    if let Err(err) = ensure_mailbox_identity(&store, &peer_id, "subagent") {
        outcome.tool_records.push(failed_record(
            input,
            tool_call_id.into(),
            "mailbox.poll",
            format!("failed to ensure mailbox identity: {err}").as_str(),
        ));
        return true;
    }
    let inbox_path = runtime_home.join(format!(
        "runtime/agents/control/mailbox/{peer_id}/inbox.json"
    ));
    let inbox = match store.read_mailbox(&peer_id) {
        Ok(items) => items,
        Err(err) => {
            outcome.tool_records.push(failed_record(
                input,
                tool_call_id.into(),
                "mailbox.poll",
                format!("failed to read agent mailbox: {err}").as_str(),
            ));
            return true;
        }
    };

    let selected = inbox.iter().take(limit).cloned().collect::<Vec<_>>();
    if consume {
        let consume_count = selected
            .iter()
            .filter(|item| item.consumed_at.is_none())
            .count();
        for _ in 0..consume_count {
            if let Err(err) = store.consume_next_mailbox_message(&peer_id, input.occurred_at) {
                outcome.tool_records.push(failed_record(
                    input,
                    tool_call_id.into(),
                    "mailbox.poll",
                    format!("failed to consume agent mailbox: {err}").as_str(),
                ));
                return true;
            }
        }
    }
    let remaining = match store.read_mailbox(&peer_id) {
        Ok(items) => items
            .into_iter()
            .filter(|item| item.consumed_at.is_none())
            .count(),
        Err(_) => 0,
    };

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
            remaining,
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
            "remaining_count": remaining,
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

fn ensure_mailbox_identity(
    store: &AgentControlStore,
    agent_id: &str,
    auth_subject_suffix: &str,
) -> Result<(), String> {
    store
        .register_primary_agent(crate::RegisterPrimaryAgentInput {
            agent_id: agent_id.to_string(),
            kind: crate::AgentKind::ProjectAgent,
            project_id: Some("mailbox".into()),
            device_binding: "runtime-tool".into(),
            auth_subject: format!("runtime-tool:{auth_subject_suffix}:{agent_id}"),
            auth_lease_id: format!("mailbox-lease-{agent_id}"),
            capability_descriptor: crate::CapabilityDescriptor {
                capability_ids: vec!["mailbox".into()],
                tool_allowlist: vec!["mailbox.send".into(), "mailbox.poll".into()],
            },
            now: "1970-01-01T00:00:00Z".into(),
        })
        .or_else(|err| {
            if err.contains("unknown") || err.contains("requires") {
                Err(err)
            } else {
                Ok(crate::AgentIdentity {
                    agent_id: agent_id.to_string(),
                    kind: crate::AgentKind::ProjectAgent,
                    parent_agent_id: None,
                    project_id: Some("mailbox".into()),
                    device_binding: "runtime-tool".into(),
                    auth_subject: format!("runtime-tool:{auth_subject_suffix}:{agent_id}"),
                    capability_descriptor: crate::CapabilityDescriptor {
                        capability_ids: vec!["mailbox".into()],
                        tool_allowlist: vec!["mailbox.send".into(), "mailbox.poll".into()],
                    },
                    auth_lease_id: format!("mailbox-lease-{agent_id}"),
                    path: format!("project:mailbox:{agent_id}"),
                })
            }
        })?;
    Ok(())
}
