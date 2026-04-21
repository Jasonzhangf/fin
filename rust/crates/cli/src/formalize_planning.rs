use crate::{
    CliError, execution_state::enqueue_framework_pending_input, session_binding::find_session_dir,
};
use fin_config::SystemConfig;
use fin_contracts::{DebugVisibility, EntityRefs, EventEnvelope, RoutingActionRecord, Severity};
use fin_debug_server::DebugBinding;
use fin_runtime::append_framework_events;
use serde_json::{Value, json};
use std::path::Path;

pub(crate) const FRAMEWORK_PLANNING_INPUT_KIND: &str = "framework_planning";
pub(crate) const FRAMEWORK_PLANNING_SOURCE: &str = "framework.task_kickoff.plan";

pub(crate) fn enqueue_formalized_planning_kickoff(
    runtime_home: &Path,
    system: &SystemConfig,
    binding: &DebugBinding,
    action: &RoutingActionRecord,
    topic_thread_id: &str,
    goal_summary: &str,
    routing_reason: &str,
    now: &str,
) -> Result<bool, CliError> {
    let Some(session_id) = binding.session_id.as_deref() else {
        return Ok(false);
    };
    let Some(task_id) = binding.task_id.as_deref() else {
        return Ok(false);
    };
    let Some((_, _, session_dir)) = find_session_dir(runtime_home, session_id) else {
        return Ok(false);
    };

    let prompt = framework_planning_prompt(
        session_id,
        task_id,
        topic_thread_id,
        goal_summary,
        routing_reason,
    );
    let kickoff = enqueue_framework_pending_input(
        runtime_home,
        binding,
        FRAMEWORK_PLANNING_INPUT_KIND,
        FRAMEWORK_PLANNING_SOURCE,
        &prompt,
        "formalized task requires framework planning kickoff",
        now,
    )?;

    let refs = EntityRefs {
        session_id: Some(session_id.into()),
        task_id: Some(task_id.into()),
        topic_thread_id: Some(topic_thread_id.into()),
        ..EntityRefs::default()
    };
    let events = vec![
        framework_event(
            "session.formalized",
            1,
            now,
            &refs,
            Some(action.action_id.clone()),
            json!({
                "task_id": task_id,
                "topic_thread_id": topic_thread_id,
                "resolution_kind": action.action_kind,
                "routing_reason": routing_reason,
                "goal_summary": goal_summary,
            }),
        ),
        framework_event(
            "framework.task_kickoff_enqueued",
            2,
            now,
            &refs,
            kickoff.as_ref().map(|value| value.pending_input_id.clone()),
            json!({
                "task_id": task_id,
                "topic_thread_id": topic_thread_id,
                "input_kind": FRAMEWORK_PLANNING_INPUT_KIND,
                "source": FRAMEWORK_PLANNING_SOURCE,
                "enqueue_reason": "formalized task requires framework planning kickoff",
                "goal_summary": goal_summary,
                "routing_reason": routing_reason,
            }),
        ),
    ];
    append_framework_events(
        runtime_home,
        &session_dir,
        &events,
        &system.runtime.retention,
    )
    .map_err(|error| CliError::InvalidInstallState(error.to_string()))?;
    Ok(kickoff.is_some())
}

pub(crate) fn framework_planning_prompt(
    session_id: &str,
    task_id: &str,
    topic_thread_id: &str,
    goal_summary: &str,
    routing_reason: &str,
) -> String {
    format!(
        concat!(
            "Framework planning kickoff: the task is now formalized.\n",
            "priority=highest\n",
            "session_id={session_id}\n",
            "task_id={task_id}\n",
            "topic_thread_id={topic_thread_id}\n",
            "goal_summary={goal_summary}\n",
            "routing_reason={routing_reason}\n",
            "required_behavior=decide whether this work should stay on the direct path or enter the managed project path.\n",
            "direct_path_rule=if the work can close with one bounded execution slice, use update_plan to persist a minimal direct plan.\n",
            "managed_path_rule=if the work needs decomposition, parallelism, review, unblock analysis, or long-running coordination, create managed tasks with project.task.create before dispatch.\n",
            "constraints=do not ask the user to formalize again; use framework-owned task truth instead of chat-only planning."
        ),
        session_id = session_id,
        task_id = task_id,
        topic_thread_id = topic_thread_id,
        goal_summary = goal_summary,
        routing_reason = routing_reason,
    )
}

fn framework_event(
    event_type: &str,
    sequence: u64,
    occurred_at: &str,
    refs: &EntityRefs,
    operation_id: Option<String>,
    payload: Value,
) -> EventEnvelope<Value> {
    let event_id = format!(
        "formalize-{}-{}-evt-{sequence:02}",
        refs.session_id.as_deref().unwrap_or("tentative"),
        sanitize_id(occurred_at),
    );
    let mut event = EventEnvelope::new(
        event_id,
        event_type,
        occurred_at.to_string(),
        "cli.session_routing",
        format!(
            "formalize-{}",
            refs.session_id.as_deref().unwrap_or("tentative")
        ),
        sequence,
        payload,
    );
    event.refs = refs.clone();
    event.operation_id = operation_id;
    event.severity = Severity::Info;
    event.debug_visibility = DebugVisibility::Important;
    event
}

fn sanitize_id(raw: &str) -> String {
    raw.chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                ch
            } else {
                '-'
            }
        })
        .collect()
}
