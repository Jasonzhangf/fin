use crate::task::store::{StoredTaskRecord, create_task_record, load_task_record, update_task_record};
use super::tool_dispatch::{
        ToolDispatchInput, ToolDispatchOutcome, failed_record, read_string,
        runtime_home_from_context, short_text,
    };
use fin_contracts::ToolExecutionRecord;
use serde_json::{Value, json};
use std::path::Path;

pub(super) fn handle_project_task_create(
    outcome: &mut ToolDispatchOutcome,
    input: &ToolDispatchInput<'_>,
    tool_call_id: &str,
    arguments: &Value,
) -> bool {
    let Some(runtime_home) = runtime_home_from_context(input.context) else {
        outcome.tool_records.push(failed_record(
            input,
            tool_call_id.into(),
            "project.task.create",
            "missing context.project.runtime_home, cannot persist task registry",
        ));
        return true;
    };
    let Some(session_id) = input.refs.session_id.clone() else {
        outcome.tool_records.push(failed_record(
            input,
            tool_call_id.into(),
            "project.task.create",
            "missing refs.session_id, cannot create session-scoped task",
        ));
        return true;
    };

    let title = read_string(arguments, "title")
        .or_else(|| read_string(arguments, "summary"))
        .unwrap_or_else(|| "untitled task".into());
    let summary = read_string(arguments, "summary").unwrap_or_else(|| title.clone());
    let task_id = read_string(arguments, "task_id")
        .unwrap_or_else(|| format!("task-{}-{tool_call_id}", sanitize_id(&title)));
    match load_task_record(&runtime_home, &task_id, Some(&session_id)) {
        Ok(Some(_)) => {
            outcome.tool_records.push(failed_record(
                input,
                tool_call_id.into(),
                "project.task.create",
                format!("task_id already exists in session truth: {task_id}").as_str(),
            ));
            return true;
        }
        Ok(None) => {}
        Err(error) => {
            outcome.tool_records.push(failed_record(
                input,
                tool_call_id.into(),
                "project.task.create",
                error.as_str(),
            ));
            return true;
        }
    }

    let task = StoredTaskRecord {
        task_id: task_id.clone(),
        session_id: session_id.clone(),
        title: title.clone(),
        summary: summary.clone(),
        epic_id: read_string(arguments, "epic_id"),
        status: read_string(arguments, "status").unwrap_or_else(|| "ready".into()),
        creator_worker_id: input.refs.worker_id.clone(),
        creator_role_id: input
            .context
            .role_prompt
            .as_ref()
            .map(|value| value.role_id.clone()),
        review_owner_worker_id: read_string(arguments, "review_owner_worker_id")
            .or_else(|| input.refs.worker_id.clone()),
        created_at: input.occurred_at.into(),
        updated_at: input.occurred_at.into(),
        ..StoredTaskRecord::default()
    };
    let receipt = match create_task_record(&runtime_home, &session_id, task.clone()) {
        Ok(value) => value,
        Err(error) => {
            outcome.tool_records.push(failed_record(
                input,
                tool_call_id.into(),
                "project.task.create",
                error.as_str(),
            ));
            return true;
        }
    };

    outcome.tool_records.push(task_record(
        input,
        tool_call_id,
        "project.task.create",
        "Project Task Create",
        "create one managed task in session task registry",
        task.task_id.clone(),
        format!("task_id={task_id}, title={}", short_text(&title, 80)),
        format!("created task {} with status={}", task_id, task.status),
        vec!["persist_task_registry".into()],
        receipt.artifact_refs,
    ));
    outcome.events.push((
        "project.task.created".into(),
        json!({
            "tool_call_id": tool_call_id,
            "task_id": task_id,
            "session_id": session_id,
            "status": task.status,
            "title": short_text(&title, 160),
            "review_owner_worker_id": task.review_owner_worker_id,
        }),
    ));
    outcome.note_hints.push(format!("created task {task_id}"));
    true
}

pub(super) fn handle_project_task_claim(
    outcome: &mut ToolDispatchOutcome,
    input: &ToolDispatchInput<'_>,
    tool_call_id: &str,
    arguments: &Value,
) -> bool {
    mutate_task(
        outcome,
        input,
        tool_call_id,
        arguments,
        "project.task.claim",
        "Project Task Claim",
        "claim one managed task for execution",
        |task, input, arguments| {
            reject_terminal_task(task)?;
            let worker_id = read_string(arguments, "worker_id")
                .or_else(|| input.refs.worker_id.clone())
                .ok_or_else(|| "missing worker_id and refs.worker_id is unavailable".to_string())?;
            if task
                .claimed_by_worker_id
                .as_deref()
                .is_some_and(|value| value != worker_id)
                && matches!(task.status.as_str(), "claimed" | "working" | "reviewing")
            {
                return Err(format!(
                    "task is already owned by another worker: {}",
                    task.claimed_by_worker_id
                        .as_deref()
                        .unwrap_or("unknown-worker")
                ));
            }
            task.status = read_string(arguments, "status").unwrap_or_else(|| "claimed".into());
            task.claimed_by_worker_id = Some(worker_id.clone());
            task.updated_at = input.occurred_at.into();
            let summary = format!("claimed by {worker_id} with status={}", task.status);
            Ok((
                summary.clone(),
                vec!["persist_task_registry".into()],
                json!({
                    "task_id": task.task_id,
                    "status": task.status,
                    "claimed_by_worker_id": task.claimed_by_worker_id,
                }),
            ))
        },
    )
}

pub(super) fn handle_project_task_submit(
    outcome: &mut ToolDispatchOutcome,
    input: &ToolDispatchInput<'_>,
    tool_call_id: &str,
    arguments: &Value,
) -> bool {
    mutate_task(
        outcome,
        input,
        tool_call_id,
        arguments,
        "project.task.submit",
        "Project Task Submit",
        "submit one managed task result for owner review",
        |task, input, arguments| {
            reject_terminal_task(task)?;
            if task
                .claimed_by_worker_id
                .as_deref()
                .is_some_and(|value| Some(value) != input.refs.worker_id.as_deref())
            {
                return Err(format!(
                    "task is claimed by another worker: {}",
                    task.claimed_by_worker_id
                        .as_deref()
                        .unwrap_or("unknown-worker")
                ));
            }
            let Some(result_summary) = read_string(arguments, "result_summary")
                .or_else(|| read_string(arguments, "summary"))
            else {
                return Err("missing required argument: result_summary".into());
            };
            task.status = "submitted".into();
            task.submitted_by_worker_id = input.refs.worker_id.clone();
            task.latest_submission_summary = Some(result_summary.clone());
            task.updated_at = input.occurred_at.into();
            if let Some(extra) = read_string_array(arguments, "artifact_refs") {
                merge_artifact_refs(&mut task.artifact_refs, &extra);
            }
            Ok((
                format!("submitted for review: {}", short_text(&result_summary, 120)),
                vec!["persist_task_registry".into()],
                json!({
                    "task_id": task.task_id,
                    "status": task.status,
                    "submitted_by_worker_id": task.submitted_by_worker_id,
                    "result_summary": short_text(&result_summary, 240),
                    "artifact_refs": task.artifact_refs,
                }),
            ))
        },
    )
}

pub(super) fn handle_project_task_review(
    outcome: &mut ToolDispatchOutcome,
    input: &ToolDispatchInput<'_>,
    tool_call_id: &str,
    arguments: &Value,
) -> bool {
    mutate_task(
        outcome,
        input,
        tool_call_id,
        arguments,
        "project.task.review",
        "Project Task Review",
        "review one submitted task and decide done/reopen/block/cancel",
        |task, input, arguments| {
            let Some(review_owner) = task.review_owner_worker_id.as_deref() else {
                return Err("task has no review_owner_worker_id".into());
            };
            if Some(review_owner) != input.refs.worker_id.as_deref() {
                return Err(format!(
                    "current worker is not review owner: expected {review_owner}"
                ));
            }
            let Some(decision) = read_string(arguments, "decision") else {
                return Err("missing required argument: decision".into());
            };
            let next_status = match decision.as_str() {
                "approve" | "approved" | "done" => "done",
                "reopen" => "ready",
                "block" | "blocked" => "blocked",
                "cancel" | "cancelled" => "cancelled",
                _ => return Err("decision must be approve|reopen|block|cancel".into()),
            };
            let review_summary = read_string(arguments, "review_summary")
                .or_else(|| read_string(arguments, "summary"));
            task.status = next_status.into();
            task.reviewed_by_worker_id = input.refs.worker_id.clone();
            task.latest_review_decision = Some(decision.clone());
            task.latest_review_summary = review_summary.clone();
            task.updated_at = input.occurred_at.into();
            if decision == "reopen" {
                task.claimed_by_worker_id = None;
                task.submitted_by_worker_id = None;
            }
            Ok((
                format!("review decision={} -> status={}", decision, task.status),
                vec!["persist_task_registry".into()],
                json!({
                    "task_id": task.task_id,
                    "decision": decision,
                    "status": task.status,
                    "reviewed_by_worker_id": task.reviewed_by_worker_id,
                    "review_summary": review_summary,
                    "claimed_by_worker_id": task.claimed_by_worker_id,
                }),
            ))
        },
    )
}

fn mutate_task(
    outcome: &mut ToolDispatchOutcome,
    input: &ToolDispatchInput<'_>,
    tool_call_id: &str,
    arguments: &Value,
    tool_name: &str,
    title: &str,
    purpose: &str,
    mutator: impl FnOnce(
        &mut StoredTaskRecord,
        &ToolDispatchInput<'_>,
        &Value,
    ) -> Result<(String, Vec<String>, Value), String>,
) -> bool {
    let Some(runtime_home) = runtime_home_from_context(input.context) else {
        outcome.tool_records.push(failed_record(
            input,
            tool_call_id.into(),
            tool_name,
            "missing context.project.runtime_home, cannot mutate task registry",
        ));
        return true;
    };
    let Some(task_id) = read_string(arguments, "task_id").or_else(|| input.refs.task_id.clone())
    else {
        outcome.tool_records.push(failed_record(
            input,
            tool_call_id.into(),
            tool_name,
            "missing task_id and refs.task_id is unavailable",
        ));
        return true;
    };
    let preferred_session_id = input.refs.session_id.as_deref();
    let (summary, mut task) = match load_task_record(&runtime_home, &task_id, preferred_session_id)
    {
        Ok(Some(value)) => value,
        Ok(None) => {
            outcome.tool_records.push(failed_record(
                input,
                tool_call_id.into(),
                tool_name,
                format!("task not found in runtime truth: {task_id}").as_str(),
            ));
            return true;
        }
        Err(error) => {
            outcome.tool_records.push(failed_record(
                input,
                tool_call_id.into(),
                tool_name,
                error.as_str(),
            ));
            return true;
        }
    };

    let (output_summary, side_effects, event_payload) = match mutator(&mut task, input, arguments) {
        Ok(value) => value,
        Err(error) => {
            outcome.tool_records.push(failed_record(
                input,
                tool_call_id.into(),
                tool_name,
                error.as_str(),
            ));
            return true;
        }
    };
    let receipt = match update_task_record(&runtime_home, &summary.session_id, task.clone()) {
        Ok(value) => value,
        Err(error) => {
            outcome.tool_records.push(failed_record(
                input,
                tool_call_id.into(),
                tool_name,
                error.as_str(),
            ));
            return true;
        }
    };

    outcome.tool_records.push(task_record(
        input,
        tool_call_id,
        tool_name,
        title,
        purpose,
        task_id.clone(),
        format!("task_id={task_id}"),
        output_summary.clone(),
        side_effects,
        receipt.artifact_refs,
    ));
    outcome.events.push((
        tool_event_name(tool_name),
        json!({
            "tool_call_id": tool_call_id,
            "session_id": summary.session_id,
            "task": event_payload,
        }),
    ));
    outcome
        .note_hints
        .push(format!("{tool_name} {output_summary}"));
    true
}

fn task_record(
    input: &ToolDispatchInput<'_>,
    tool_call_id: &str,
    tool_name: &str,
    title: &str,
    purpose: &str,
    task_id: String,
    input_summary: String,
    output_summary: String,
    side_effects: Vec<String>,
    artifact_refs: Vec<String>,
) -> ToolExecutionRecord {
    ToolExecutionRecord {
        tool_call_id: tool_call_id.into(),
        operation_id: input.operation_id.into(),
        trace_id: input.trace_id.into(),
        refs: input.refs.clone(),
        tool_name: tool_name.into(),
        tool_kind: "agent_tool".into(),
        title: title.into(),
        purpose: purpose.into(),
        target_kind: Some("task_registry".into()),
        target_ref: Some(task_id),
        input_summary: Some(input_summary),
        output_summary: Some(output_summary),
        status: "completed".into(),
        started_at: input.occurred_at.into(),
        ended_at: Some(input.occurred_at.into()),
        duration_ms: Some(0),
        side_effects,
        artifact_refs,
        error_summary: None,
    }
}

fn reject_terminal_task(task: &StoredTaskRecord) -> Result<(), String> {
    if matches!(task.status.as_str(), "done" | "cancelled") {
        Err(format!(
            "task is terminal and cannot be mutated: {}",
            task.status
        ))
    } else {
        Ok(())
    }
}

fn merge_artifact_refs(current: &mut Vec<String>, extra: &[String]) {
    current.extend(extra.iter().cloned());
    current.sort();
    current.dedup();
}

fn read_string_array(arguments: &Value, key: &str) -> Option<Vec<String>> {
    let object = arguments.as_object()?;
    let values = object.get(key)?.as_array()?;
    let parsed = values
        .iter()
        .filter_map(|value| value.as_str().map(str::trim))
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .collect::<Vec<_>>();
    (!parsed.is_empty()).then_some(parsed)
}

fn sanitize_id(raw: &str) -> String {
    let compact = raw
        .chars()
        .map(|ch| match ch {
            'a'..='z' | '0'..='9' => ch,
            'A'..='Z' => ch.to_ascii_lowercase(),
            _ => '-',
        })
        .collect::<String>();
    compact
        .split('-')
        .filter(|part| !part.is_empty())
        .take(4)
        .collect::<Vec<_>>()
        .join("-")
}

fn tool_event_name(tool_name: &str) -> String {
    let suffix = Path::new(tool_name)
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or(tool_name)
        .replace('.', "_");
    format!("{tool_name}_completed").replace("task_", &format!("task.{suffix}"))
}
