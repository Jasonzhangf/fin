use crate::{CliError, runtime_home::read_last_run_value};
use fin_contracts::{ControlFeedback, ExecutionNote, ProgressBlock};
use fin_debug_server::{ChatSendRequest, ChatSendResponse, DebugBinding};
use serde::de::DeserializeOwned;
use serde_json::Value;
use std::{fs, path::{Path, PathBuf}};

pub(crate) fn build_status_probe_response(
    runtime_home: &Path,
    binding: DebugBinding,
    request: &ChatSendRequest,
) -> Result<ChatSendResponse, CliError> {
    let last_run = read_last_run_value(runtime_home).ok();
    let progress = sibling_json::<ProgressBlock>(
        runtime_home,
        &last_run,
        "session_messages_path",
        "conversation/messages.json",
        "progress/latest.json",
    )?;
    let note = sibling_json::<ExecutionNote>(
        runtime_home,
        &last_run,
        "session_messages_path",
        "conversation/messages.json",
        "notes/latest.json",
    )?;
    let control_feedback = runtime_json::<ControlFeedback>(
        runtime_home,
        &last_run,
        "session_control_feedback_path",
    )?
    .or(runtime_json::<ControlFeedback>(
        runtime_home,
        &last_run,
        "current_control_feedback_path",
    )?);

    let freshness = probe_freshness(progress.as_ref(), note.as_ref(), control_feedback.as_ref());
    let digest_id = last_run
        .as_ref()
        .and_then(|value| value.get("digest_id"))
        .and_then(Value::as_str)
        .unwrap_or("status-probe")
        .to_string();

    Ok(ChatSendResponse {
        binding: binding.clone(),
        answer: render_status_answer(
            &binding,
            &request.normalized_message(),
            &freshness,
            progress.as_ref(),
            note.as_ref(),
            control_feedback.as_ref(),
        ),
        digest_id,
        events_count: 0,
        response_kind: "status_probe".into(),
        freshness: Some(freshness),
        control_feedback,
        progress,
        note,
    })
}

fn render_status_answer(
    binding: &DebugBinding,
    probe_message: &str,
    freshness: &str,
    progress: Option<&ProgressBlock>,
    note: Option<&ExecutionNote>,
    control_feedback: Option<&ControlFeedback>,
) -> String {
    let phase = progress
        .map(|value| value.phase.as_str())
        .unwrap_or("unavailable");
    let blocker = progress
        .and_then(|value| value.blocker.as_deref())
        .unwrap_or("none");
    let next_step = progress
        .and_then(|value| value.next_step.as_deref())
        .or_else(|| note.and_then(|value| value.next_step.as_deref()))
        .unwrap_or("unknown");
    let note_summary = note
        .map(|value| value.summary.as_str())
        .unwrap_or("no execution note yet");
    let control_summary = control_feedback
        .map(|value| {
            format!(
                "continuity={} shift={} simple={} continuation={} reason={}",
                value.continuity_confidence,
                value.topic_shift_confidence,
                value.simple_query_confidence,
                value.is_continuation,
                if value.reason.trim().is_empty() {
                    "unknown"
                } else {
                    value.reason.as_str()
                }
            )
        })
        .unwrap_or_else(|| "control unavailable".into());

    format!(
        "status probe ({freshness})\nrequest={probe_message}\nsession={}\ntask={}\nphase={phase}\nblocker={blocker}\nnext_step={next_step}\nnote={note_summary}\ncontrol={control_summary}",
        binding.session_id.as_deref().unwrap_or("tentative"),
        binding.task_id.as_deref().unwrap_or("-"),
    )
}

fn probe_freshness(
    progress: Option<&ProgressBlock>,
    note: Option<&ExecutionNote>,
    control_feedback: Option<&ControlFeedback>,
) -> String {
    if progress
        .map(|value| matches!(value.phase.as_str(), "running" | "inference_started"))
        .unwrap_or(false)
    {
        return "live".into();
    }
    if progress.is_some() || note.is_some() || control_feedback.is_some() {
        return "recent".into();
    }
    "unavailable".into()
}

fn sibling_json<T: DeserializeOwned>(
    runtime_home: &Path,
    last_run: &Option<Value>,
    field: &str,
    source_suffix: &str,
    target_suffix: &str,
) -> Result<Option<T>, CliError> {
    let Some(path) = sibling_path(runtime_home, last_run, field, source_suffix, target_suffix) else {
        return Ok(None);
    };
    read_json_file(&path).map(Some)
}

fn runtime_json<T: DeserializeOwned>(
    runtime_home: &Path,
    last_run: &Option<Value>,
    field: &str,
) -> Result<Option<T>, CliError> {
    let Some(path) = runtime_path(runtime_home, last_run, field) else {
        return Ok(None);
    };
    read_json_file(&path).map(Some)
}

fn runtime_path(runtime_home: &Path, last_run: &Option<Value>, field: &str) -> Option<PathBuf> {
    last_run
        .as_ref()
        .and_then(|value| value.get(field))
        .and_then(Value::as_str)
        .map(|relative| runtime_home.join(relative))
}

fn sibling_path(
    runtime_home: &Path,
    last_run: &Option<Value>,
    field: &str,
    source_suffix: &str,
    target_suffix: &str,
) -> Option<PathBuf> {
    let relative = last_run
        .as_ref()
        .and_then(|value| value.get(field))
        .and_then(Value::as_str)?;
    relative
        .strip_suffix(source_suffix)
        .map(|prefix| runtime_home.join(format!("{prefix}{target_suffix}")))
}

fn read_json_file<T: DeserializeOwned>(path: &Path) -> Result<T, CliError> {
    serde_json::from_str(
        &fs::read_to_string(path).map_err(|source| CliError::ReadFile {
            path: path.display().to_string(),
            source,
        })?,
    )
    .map_err(CliError::Serialize)
}
