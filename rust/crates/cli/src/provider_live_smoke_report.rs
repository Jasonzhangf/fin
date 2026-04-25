use super::provider_live_smoke_timeline::{build_conformance, build_turn_timelines};
use super::{ProviderLiveSmokeLayout, ProviderLiveSmokeReport};
use crate::{CliError, fs_utils::write_file, time::local_timestamp_now};
use fin_runtime::ClosureRun;
use serde_json::json;
use std::{fs, path::Path};

pub(super) fn build_report(
    layout: &ProviderLiveSmokeLayout,
    runtime_home: &Path,
    transcript: &LiveTranscriptRun,
    projection_json: &Path,
    snapshot_json: &Path,
    last_run: &serde_json::Value,
) -> Result<ProviderLiveSmokeReport, CliError> {
    let current_context_path = runtime_home.join("runtime/current/current_context.json");
    let current_provider_requests_path =
        runtime_home.join("runtime/current/current_provider_requests.json");
    let current_provider_responses_path =
        runtime_home.join("runtime/current/current_provider_responses.json");
    let current_rounds_path = runtime_home.join("runtime/current/current_rounds.json");
    let current_steps_path = runtime_home.join("runtime/current/current_step_records.json");
    let current_control_feedback_path =
        runtime_home.join("runtime/current/current_control_feedback.json");
    let current_tool_records_path = runtime_home.join("runtime/current/current_tool_records.json");
    let session_recent_contexts_path = runtime_home.join(required_last_run_path(
        last_run,
        "session_recent_contexts_path",
    )?);
    let session_messages_path =
        runtime_home.join(required_last_run_path(last_run, "session_messages_path")?);
    let session_recent_rounds_path = runtime_home.join(required_last_run_path(
        last_run,
        "session_recent_rounds_path",
    )?);
    let session_recent_steps_path = runtime_home.join(required_last_run_path(
        last_run,
        "session_recent_step_records_path",
    )?);
    let session_recent_tool_records_path = runtime_home.join(required_last_run_path(
        last_run,
        "session_recent_tool_records_path",
    )?);
    let session_root = session_root_from_messages_path(&session_messages_path)?;
    let session_recent_provider_requests_path =
        session_root.join("provider/recent_provider_requests.json");
    let session_recent_provider_responses_path =
        session_root.join("provider/recent_provider_responses.json");

    for path in [
        projection_json,
        snapshot_json,
        &current_context_path,
        &current_provider_requests_path,
        &current_provider_responses_path,
        &current_rounds_path,
        &current_steps_path,
        &current_control_feedback_path,
        &current_tool_records_path,
        &session_recent_contexts_path,
        &session_messages_path,
        &session_recent_rounds_path,
        &session_recent_steps_path,
        &session_recent_tool_records_path,
        &session_recent_provider_requests_path,
        &session_recent_provider_responses_path,
    ] {
        if !path.exists() {
            return Err(CliError::MissingInstallTarget(path.display().to_string()));
        }
    }

    let request_records: Vec<serde_json::Value> = read_json_array(&current_provider_requests_path)?;
    let response_records: Vec<serde_json::Value> =
        read_json_array(&current_provider_responses_path)?;
    let round_records: Vec<serde_json::Value> = read_json_array(&current_rounds_path)?;
    let step_records: Vec<serde_json::Value> = read_json_array(&current_steps_path)?;
    let session_messages: Vec<serde_json::Value> = read_json_array(&session_messages_path)?;
    let control_feedback: serde_json::Value = read_json_value(&current_control_feedback_path)?;
    let tool_records: Vec<serde_json::Value> = read_json_array(&current_tool_records_path)?;
    let session_tool_records: Vec<serde_json::Value> =
        read_json_array(&session_recent_tool_records_path)?;
    let session_step_records: Vec<serde_json::Value> = read_json_array(&session_recent_steps_path)?;
    let session_round_records: Vec<serde_json::Value> =
        read_json_array(&session_recent_rounds_path)?;
    let session_provider_request_records: Vec<serde_json::Value> =
        read_json_array(&session_recent_provider_requests_path)?;
    let session_provider_response_records: Vec<serde_json::Value> =
        read_json_array(&session_recent_provider_responses_path)?;

    if request_records.is_empty()
        || response_records.is_empty()
        || round_records.is_empty()
        || step_records.is_empty()
    {
        return Err(CliError::InvalidInstallState(
            "provider live smoke wrote incomplete current runtime artifacts".into(),
        ));
    }
    if session_messages.len() < transcript.runs.len() * 2 {
        return Err(CliError::InvalidInstallState(format!(
            "expected at least {} session messages, got {}",
            transcript.runs.len() * 2,
            session_messages.len()
        )));
    }

    let reasoning_stop_present =
        session_tool_records
            .iter()
            .chain(tool_records.iter())
            .any(|record| {
                record.get("tool_name").and_then(serde_json::Value::as_str)
                    == Some("reasoning.stop")
                    && record.get("status").and_then(serde_json::Value::as_str) == Some("completed")
            });
    let turn_timelines = build_turn_timelines(
        transcript,
        &session_step_records,
        &session_round_records,
        &session_tool_records,
        &session_provider_request_records,
    );
    let mut conformance = build_conformance(
        transcript,
        &session_messages,
        &turn_timelines,
        &session_provider_response_records,
    );
    if control_feedback
        .get("origin")
        .and_then(serde_json::Value::as_str)
        .is_some_and(|origin| {
            origin == "runtime_heuristic" || origin == "runtime_observation_only_v1"
        })
    {
        conformance
            .issues
            .push("control feedback stayed in runtime observation-only mode".into());
        if conformance.attribution_hint == "framework_conformance_ok" {
            conformance.attribution_hint = "likely_model_or_prompt_gap".into();
        }
    }
    let last_closure = transcript.runs.last().ok_or(CliError::Usage)?;
    Ok(ProviderLiveSmokeReport {
        run_id: layout.run_id.clone(),
        session_id: transcript.session_id.clone(),
        task_id: transcript.task_id.clone(),
        turn_count: transcript.runs.len(),
        provider_name: last_closure.prepared_request.provider_name.clone(),
        protocol: format!("{:?}", last_closure.prepared_request.protocol),
        model: last_closure.prepared_request.model.clone(),
        runtime_home: runtime_home.display().to_string(),
        run_root: layout.run_root.display().to_string(),
        regression_log: layout.regression_log.display().to_string(),
        receipt_path: layout.receipt_path.display().to_string(),
        projection_json: projection_json.display().to_string(),
        snapshot_json: snapshot_json.display().to_string(),
        current_context_path: current_context_path.display().to_string(),
        current_provider_requests_path: current_provider_requests_path.display().to_string(),
        current_provider_responses_path: current_provider_responses_path.display().to_string(),
        current_rounds_path: current_rounds_path.display().to_string(),
        current_steps_path: current_steps_path.display().to_string(),
        current_control_feedback_path: current_control_feedback_path.display().to_string(),
        current_tool_records_path: current_tool_records_path.display().to_string(),
        control_feedback_origin: control_feedback
            .get("origin")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .to_string(),
        reasoning_stop_present,
        session_recent_contexts_path: session_recent_contexts_path.display().to_string(),
        session_messages_path: session_messages_path.display().to_string(),
        session_recent_rounds_path: session_recent_rounds_path.display().to_string(),
        session_recent_steps_path: session_recent_steps_path.display().to_string(),
        session_recent_tool_records_path: session_recent_tool_records_path.display().to_string(),
        session_recent_provider_requests_path: session_recent_provider_requests_path
            .display()
            .to_string(),
        session_recent_provider_responses_path: session_recent_provider_responses_path
            .display()
            .to_string(),
        conformance,
        turn_timelines,
        assistant_outputs_preview: transcript
            .runs
            .iter()
            .map(|run| truncate_preview(run.assistant_response_text.as_str()))
            .collect(),
        verified_at: local_timestamp_now(),
    })
}

fn session_root_from_messages_path(messages_path: &Path) -> Result<&Path, CliError> {
    messages_path
        .parent()
        .and_then(Path::parent)
        .ok_or_else(|| {
            CliError::InvalidInstallState(format!(
                "failed to derive session root from {}",
                messages_path.display()
            ))
        })
}

fn required_last_run_path<'a>(
    last_run: &'a serde_json::Value,
    key: &str,
) -> Result<&'a str, CliError> {
    last_run
        .get(key)
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| CliError::InvalidInstallState(format!("missing last_run key '{key}'")))
}

fn read_json_array(path: &Path) -> Result<Vec<serde_json::Value>, CliError> {
    serde_json::from_str(
        &fs::read_to_string(path).map_err(|source| CliError::ReadFile {
            path: path.display().to_string(),
            source,
        })?,
    )
    .map_err(CliError::Serialize)
}

fn read_json_value(path: &Path) -> Result<serde_json::Value, CliError> {
    serde_json::from_str(
        &fs::read_to_string(path).map_err(|source| CliError::ReadFile {
            path: path.display().to_string(),
            source,
        })?,
    )
    .map_err(CliError::Serialize)
}

fn truncate_preview(text: &str) -> String {
    const MAX_CHARS: usize = 120;
    let trimmed = text.trim();
    let mut collected = String::new();
    for (index, ch) in trimmed.chars().enumerate() {
        if index >= MAX_CHARS {
            collected.push('…');
            break;
        }
        collected.push(ch);
    }
    collected
}

pub(super) fn write_report(report: &ProviderLiveSmokeReport, path: &Path) -> Result<(), CliError> {
    write_file(
        path,
        serde_json::to_vec_pretty(&json!(report))
            .map_err(CliError::Serialize)?
            .as_slice(),
    )
}

#[derive(Debug, Clone, PartialEq)]
pub(super) struct LiveTranscriptRun {
    pub(super) session_id: String,
    pub(super) task_id: String,
    pub(super) runs: Vec<ClosureRun>,
}
