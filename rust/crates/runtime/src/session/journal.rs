use super::materializer::{read_json_or_empty, trim_head, write_json_file};
use crate::{ClosureRun, RuntimeError};
use fin_config::RuntimeRetentionConfig;
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct SessionJournalPaths {
    pub(super) session_recent_provider_requests_path: String,
    pub(super) session_recent_provider_responses_path: String,
    pub(super) session_recent_rounds_path: String,
    pub(super) session_recent_step_records_path: String,
    pub(super) session_recent_turns_path: String,
    pub(super) session_recent_routing_decisions_path: String,
    pub(super) session_recent_routing_actions_path: String,
}

pub(crate) fn persist_extended_records(
    runtime_home: &Path,
    session_dir: &Path,
    year: &str,
    month: &str,
    session_id: &str,
    run: &ClosureRun,
    retention: &RuntimeRetentionConfig,
) -> Result<SessionJournalPaths, RuntimeError> {
    persist_recent_records(
        &runtime_home.join("runtime/current/current_provider_requests.json"),
        &session_dir.join("provider/recent_provider_requests.json"),
        &session_dir.join("provider/latest_requests.json"),
        &run.provider_request_records,
        retention.recent_provider_request_limit,
    )?;
    persist_recent_records(
        &runtime_home.join("runtime/current/current_provider_responses.json"),
        &session_dir.join("provider/recent_provider_responses.json"),
        &session_dir.join("provider/latest_responses.json"),
        &run.provider_response_records,
        retention.recent_provider_response_limit,
    )?;
    persist_recent_records(
        &runtime_home.join("runtime/current/current_rounds.json"),
        &session_dir.join("rounds/recent_rounds.json"),
        &session_dir.join("rounds/latest.json"),
        &run.round_records,
        retention.recent_round_limit,
    )?;
    persist_recent_records(
        &runtime_home.join("runtime/current/current_step_records.json"),
        &session_dir.join("steps/recent_steps.json"),
        &session_dir.join("steps/latest.json"),
        &run.step_records,
        retention.recent_step_record_limit,
    )?;
    persist_single_with_recent(
        &runtime_home.join("runtime/current/current_turn.json"),
        &session_dir.join("turns/recent_turns.json"),
        &session_dir.join("turns/latest.json"),
        &run.turn_record,
        retention.recent_turn_limit,
    )?;
    persist_single_with_recent(
        &runtime_home.join("runtime/current/current_routing_decision.json"),
        &session_dir.join("tasks/routing/recent_decisions.json"),
        &session_dir.join("tasks/routing/latest.json"),
        &run.routing_decision,
        retention.recent_routing_decision_limit,
    )?;
    persist_single_with_recent(
        &runtime_home.join("runtime/current/current_routing_action.json"),
        &session_dir.join("tasks/routing/recent_actions.json"),
        &session_dir.join("tasks/routing/latest_action.json"),
        &run.routing_action,
        retention.recent_routing_decision_limit,
    )?;

    Ok(SessionJournalPaths {
        session_recent_provider_requests_path: format!(
            "sessions/{year}/{month}/{session_id}/provider/recent_provider_requests.json"
        ),
        session_recent_provider_responses_path: format!(
            "sessions/{year}/{month}/{session_id}/provider/recent_provider_responses.json"
        ),
        session_recent_rounds_path: format!(
            "sessions/{year}/{month}/{session_id}/rounds/recent_rounds.json"
        ),
        session_recent_step_records_path: format!(
            "sessions/{year}/{month}/{session_id}/steps/recent_steps.json"
        ),
        session_recent_turns_path: format!(
            "sessions/{year}/{month}/{session_id}/turns/recent_turns.json"
        ),
        session_recent_routing_decisions_path: format!(
            "sessions/{year}/{month}/{session_id}/tasks/routing/recent_decisions.json"
        ),
        session_recent_routing_actions_path: format!(
            "sessions/{year}/{month}/{session_id}/tasks/routing/recent_actions.json"
        ),
    })
}

fn persist_recent_records<T: Clone + serde::Serialize + for<'de> serde::Deserialize<'de>>(
    current_path: &Path,
    recent_path: &Path,
    latest_path: &Path,
    records: &[T],
    limit: usize,
) -> Result<(), RuntimeError> {
    write_json_file(current_path, records)?;
    let mut recent = read_json_or_empty::<T>(recent_path)?;
    recent.extend(records.iter().cloned());
    trim_head(&mut recent, limit);
    write_json_file(recent_path, &recent)?;
    write_json_file(latest_path, records)
}

fn persist_single_with_recent<T: Clone + serde::Serialize + for<'de> serde::Deserialize<'de>>(
    current_path: &Path,
    recent_path: &Path,
    latest_path: &Path,
    record: &T,
    limit: usize,
) -> Result<(), RuntimeError> {
    write_json_file(current_path, record)?;
    let mut recent = read_json_or_empty::<T>(recent_path)?;
    recent.push(record.clone());
    trim_head(&mut recent, limit);
    write_json_file(recent_path, &recent)?;
    write_json_file(latest_path, record)
}
