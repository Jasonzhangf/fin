use crate::context_project_support::{
    load_agent_presence_snapshot, load_project_supervision_snapshot,
};
use crate::tool_dispatch::{
    ToolDispatchInput, ToolDispatchOutcome, failed_record, read_string, read_u64,
    runtime_home_from_context, short_text,
};
use fin_contracts::ToolExecutionRecord;
use serde_json::{Value, json};

pub(super) fn handle_agent_presence_list(
    outcome: &mut ToolDispatchOutcome,
    input: &ToolDispatchInput<'_>,
    tool_call_id: &str,
    arguments: &Value,
) -> bool {
    let Some(runtime_home) = runtime_home_from_context(input.context) else {
        outcome.tool_records.push(failed_record(
            input,
            tool_call_id.into(),
            "agent.presence.list",
            "missing context.project.runtime_home, cannot inspect agent presence",
        ));
        return true;
    };
    let Some(snapshot) = load_agent_presence_snapshot(runtime_home.to_string_lossy().as_ref())
    else {
        outcome.tool_records.push(failed_record(
            input,
            tool_call_id.into(),
            "agent.presence.list",
            "current agent presence registry is unavailable",
        ));
        return true;
    };

    let limit = read_u64(arguments, "limit").unwrap_or(10).clamp(1, 50) as usize;
    let status_filter = read_string(arguments, "status")
        .map(|value| value.to_ascii_lowercase())
        .filter(|value| !value.trim().is_empty());
    let mut agents = snapshot
        .agents
        .into_iter()
        .filter(|entry| {
            status_filter
                .as_deref()
                .is_none_or(|expected| entry.status.eq_ignore_ascii_case(expected))
        })
        .collect::<Vec<_>>();
    agents.sort_by(|left, right| left.agent_id.cmp(&right.agent_id));
    let total_count = agents.len();
    agents.truncate(limit);
    let preview = agents
        .iter()
        .map(|entry| {
            let worker_suffix = if entry.worker_id.trim().is_empty() {
                "worker=unknown".to_string()
            } else {
                format!("worker={}", entry.worker_id)
            };
            if !entry.device_name.trim().is_empty() && !entry.agent_name.trim().is_empty() {
                format!(
                    "{}.{}:{} ({})",
                    entry.device_name, entry.agent_name, entry.status, worker_suffix
                )
            } else {
                format!("{}:{} ({})", entry.agent_id, entry.status, worker_suffix)
            }
        })
        .collect::<Vec<_>>();
    let output_summary = if preview.is_empty() {
        match status_filter.as_deref() {
            Some(status) => format!("agents=0 status_filter={status}"),
            None => "agents=0".into(),
        }
    } else {
        format!(
            "agents={} [{}]",
            total_count,
            short_text(preview.join(", ").as_str(), 180)
        )
    };

    outcome.tool_records.push(ToolExecutionRecord {
        tool_call_id: tool_call_id.into(),
        operation_id: input.operation_id.into(),
        trace_id: input.trace_id.into(),
        refs: input.refs.clone(),
        tool_name: "agent.presence.list".into(),
        tool_kind: "agent_tool".into(),
        title: "Agent Presence List".into(),
        purpose: "inspect current framework-owned agent presence registry before routing or recovery decisions".into(),
        target_kind: Some("agent_presence_registry".into()),
        target_ref: Some("runtime/current/current_agent_presence_registry.json".into()),
        input_summary: Some(match status_filter.as_deref() {
            Some(status) => format!("limit={limit} status={status}"),
            None => format!("limit={limit}"),
        }),
        output_summary: Some(output_summary.clone()),
        status: "completed".into(),
        started_at: input.occurred_at.into(),
        ended_at: Some(input.occurred_at.into()),
        duration_ms: Some(0),
        side_effects: vec!["read_agent_presence_registry".into()],
        artifact_refs: vec!["runtime/current/current_agent_presence_registry.json".into()],
        error_summary: None,
    });
    outcome.events.push((
        "agent.presence_list_completed".into(),
        json!({
            "tool_call_id": tool_call_id,
            "count": total_count,
            "status_filter": status_filter,
            "agents": agents.into_iter().map(|entry| json!({
                "agent_id": entry.agent_id,
                "worker_id": entry.worker_id,
                "device_name": entry.device_name,
                "agent_name": entry.agent_name,
                "status": entry.status,
            })).collect::<Vec<_>>(),
        }),
    ));
    outcome
        .note_hints
        .push(format!("agent.presence.list {}", output_summary));
    true
}

pub(super) fn handle_project_supervision_list(
    outcome: &mut ToolDispatchOutcome,
    input: &ToolDispatchInput<'_>,
    tool_call_id: &str,
    arguments: &Value,
) -> bool {
    let Some(runtime_home) = runtime_home_from_context(input.context) else {
        outcome.tool_records.push(failed_record(
            input,
            tool_call_id.into(),
            "project.supervision.list",
            "missing context.project.runtime_home, cannot inspect project supervision",
        ));
        return true;
    };
    let Some(snapshot) = load_project_supervision_snapshot(runtime_home.to_string_lossy().as_ref())
    else {
        outcome.tool_records.push(failed_record(
            input,
            tool_call_id.into(),
            "project.supervision.list",
            "current project supervision snapshot is unavailable",
        ));
        return true;
    };

    let limit = read_u64(arguments, "limit").unwrap_or(10).clamp(1, 50) as usize;
    let desired_action_filter = read_string(arguments, "desired_action")
        .map(|value| value.to_ascii_lowercase())
        .filter(|value| !value.trim().is_empty());
    let mut projects = snapshot
        .projects
        .into_iter()
        .filter(|entry| {
            desired_action_filter
                .as_deref()
                .is_none_or(|expected| entry.desired_action.eq_ignore_ascii_case(expected))
        })
        .collect::<Vec<_>>();
    projects.sort_by(|left, right| left.project_id.cmp(&right.project_id));
    let total_count = projects.len();
    projects.truncate(limit);
    let preview = projects
        .iter()
        .map(|entry| format!("{}:{}", entry.project_id, entry.desired_action))
        .collect::<Vec<_>>();
    let counts_summary = snapshot.summary.unwrap_or_else(|| {
        format!(
            "ready={} resume_ready={} busy={} waiting={} recover_needed={}",
            snapshot.ready_count,
            snapshot.resume_ready_count,
            snapshot.busy_count,
            snapshot.waiting_count,
            snapshot.recover_needed_count
        )
    });
    let output_summary = if preview.is_empty() {
        match desired_action_filter.as_deref() {
            Some(action) => format!("{counts_summary} · projects=0 desired_action={action}"),
            None => format!("{counts_summary} · projects=0"),
        }
    } else {
        format!(
            "{counts_summary} · projects={} [{}]",
            total_count,
            short_text(preview.join(", ").as_str(), 200)
        )
    };

    outcome.tool_records.push(ToolExecutionRecord {
        tool_call_id: tool_call_id.into(),
        operation_id: input.operation_id.into(),
        trace_id: input.trace_id.into(),
        refs: input.refs.clone(),
        tool_name: "project.supervision.list".into(),
        tool_kind: "agent_tool".into(),
        title: "Project Supervision List".into(),
        purpose: "inspect current framework-owned project supervision snapshot before routing, wakeup, or recovery decisions".into(),
        target_kind: Some("project_supervision_snapshot".into()),
        target_ref: Some("runtime/current/current_project_supervision.json".into()),
        input_summary: Some(match desired_action_filter.as_deref() {
            Some(action) => format!("limit={limit} desired_action={action}"),
            None => format!("limit={limit}"),
        }),
        output_summary: Some(output_summary.clone()),
        status: "completed".into(),
        started_at: input.occurred_at.into(),
        ended_at: Some(input.occurred_at.into()),
        duration_ms: Some(0),
        side_effects: vec!["read_project_supervision_snapshot".into()],
        artifact_refs: vec!["runtime/current/current_project_supervision.json".into()],
        error_summary: None,
    });
    outcome.events.push((
        "project.supervision_list_completed".into(),
        json!({
            "tool_call_id": tool_call_id,
            "count": total_count,
            "desired_action_filter": desired_action_filter,
            "summary": counts_summary,
            "projects": projects.into_iter().map(|entry| json!({
                "project_id": entry.project_id,
                "desired_action": entry.desired_action,
            })).collect::<Vec<_>>(),
        }),
    ));
    outcome
        .note_hints
        .push(format!("project.supervision.list {}", output_summary));
    true
}
