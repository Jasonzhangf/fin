use crate::{
    CliError,
    demo::{
        DemoRequest, demo_identity, demo_namespace_from_env, run_demo_request, sanitize_id_fragment,
    },
    fs_utils::read_file,
};
use fin_config::SystemConfig;
use fin_contracts::{DigestRecord, MinimalContextView};
use fin_provider::InferenceProvider;
use fin_runtime::ClosureRun;
use serde::{Deserialize, Serialize};
use std::path::Path;

const RECENT_CLOSURE_WINDOW: usize = 3;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct TranscriptScenario {
    pub(crate) session_id: Option<String>,
    pub(crate) task_id: Option<String>,
    pub(crate) turns: Vec<TranscriptTurn>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct TranscriptTurn {
    pub(crate) input: String,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct TranscriptRun {
    pub(crate) session_id: String,
    pub(crate) task_id: String,
    pub(crate) runs: Vec<ClosureRun>,
}

pub(crate) fn load_transcript_scenario(path: &Path) -> Result<TranscriptScenario, CliError> {
    let scenario: TranscriptScenario = serde_json::from_str(&read_file(path)?)?;
    Ok(normalize_scenario(scenario))
}

pub(crate) fn run_transcript_demo(
    system: &SystemConfig,
    provider: &impl InferenceProvider,
    scenario: &TranscriptScenario,
) -> Result<TranscriptRun, CliError> {
    let normalized = normalize_scenario(scenario.clone());
    let ids = transcript_ids(&normalized);
    let mut runs: Vec<ClosureRun> = Vec::with_capacity(normalized.turns.len());

    for (index, turn) in normalized.turns.iter().enumerate() {
        let digests = runs.iter().map(|run| run.digest.clone()).collect::<Vec<_>>();
        let request = DemoRequest {
            operation_id: format!("op-{}-{:04}", ids.scope, index + 1),
            trace_id: format!("trace-{}-{:04}", ids.scope, index + 1),
            session_id: ids.session_id.clone(),
            task_id: ids.task_id.clone(),
            input: turn.input.clone(),
            context: rebuild_context_from_digests(&digests),
            submitted_at: submitted_at_for_turn(index),
        };
        runs.push(run_demo_request(system, provider, request)?);
    }

    Ok(TranscriptRun {
        session_id: ids.session_id,
        task_id: ids.task_id,
        runs,
    })
}

fn normalize_scenario(mut scenario: TranscriptScenario) -> TranscriptScenario {
    scenario.session_id = scenario
        .session_id
        .take()
        .map(|value| sanitize_identifier(&value, "session"));
    scenario.task_id = scenario
        .task_id
        .take()
        .map(|value| sanitize_identifier(&value, "task"));
    scenario.turns.retain(|turn| !turn.input.trim().is_empty());
    scenario
}

fn sanitize_identifier(raw: &str, prefix: &str) -> String {
    let cleaned = sanitize_id_fragment(raw);
    if cleaned.is_empty() {
        format!("{prefix}-transcript-demo")
    } else if cleaned.starts_with(&format!("{prefix}-")) {
        cleaned
    } else {
        format!("{prefix}-{cleaned}")
    }
}

pub(crate) fn rebuild_context_from_digests(history: &[DigestRecord]) -> MinimalContextView {
    let recent = history
        .iter()
        .rev()
        .take(RECENT_CLOSURE_WINDOW)
        .collect::<Vec<_>>();
    let mut continuity_tail = Vec::new();
    for digest in recent.into_iter().rev() {
        continuity_tail.extend(digest.continuity_tail.iter().cloned());
    }
    let summary = history.last().map(|digest| digest.summary.clone());
    MinimalContextView {
        continuity_tail,
        summary,
    }
}

pub(crate) fn submitted_at_for_turn(index: usize) -> String {
    let minute = index / 60;
    let second = index % 60;
    format!("2026-04-17T00:{minute:02}:{second:02}Z")
}

pub(crate) fn scope_from_session_id(session_id: &str) -> String {
    session_id
        .strip_prefix("session-")
        .unwrap_or(session_id)
        .to_string()
}

struct TranscriptIds {
    scope: String,
    session_id: String,
    task_id: String,
}

fn transcript_ids(scenario: &TranscriptScenario) -> TranscriptIds {
    let namespace = demo_namespace_from_env();
    let default_scope = namespace.as_deref().unwrap_or("transcript-demo");
    let base = demo_identity(Some(default_scope));
    let session_id = scenario.session_id.clone().unwrap_or(base.session_id);
    let task_id = scenario.task_id.clone().unwrap_or(base.task_id);
    let scope = scope_from_session_id(&session_id);
    TranscriptIds {
        scope,
        session_id,
        task_id,
    }
}
