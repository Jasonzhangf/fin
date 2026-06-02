use crate::{
    CliError,
    demo::{
        DemoRequest, demo_identity, demo_namespace_from_env, run_demo_request, sanitize_id_fragment,
    },
    fs_utils::read_file,
    time::{local_time_base, local_timestamp_for_turn},
};
use fin_config::SystemConfig;
use fin_provider::InferenceProvider;
use fin_runtime::ClosureRun;
use serde::{Deserialize, Serialize};
use std::path::Path;

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
    let time_base = local_time_base();
    let mut runs: Vec<ClosureRun> = Vec::with_capacity(normalized.turns.len());

    for (index, turn) in normalized.turns.iter().enumerate() {
        let digests = runs
            .iter()
            .map(|run| run.digest.clone())
            .collect::<Vec<_>>();
        let request = DemoRequest {
            operation_id: format!("op-{}-{:04}", ids.scope, index + 1),
            trace_id: format!("trace-{}-{:04}", ids.scope, index + 1),
            session_id: ids.session_id.clone(),
            task_id: Some(ids.task_id.clone()),
            topic_thread_id: None,
            agent_name: Some("cli-demo".into()),
            role_id: None,
            input: turn.input.clone(),
            source: "cli.user".into(),
            recent_messages: transcript_history_messages(&runs),
            recent_digests: digests,
            recent_reasoning_views: runs.iter().map(|run| run.reasoning_view.clone()).collect(),
            recent_tool_records: runs
                .iter()
                .flat_map(|run| run.tool_records.iter().cloned())
                .collect(),
            project_label: Some("transcript-demo".into()),
            runtime_home: None,
            cwd: None,
            selected_paths: Vec::new(),
            attachment_summaries: Vec::new(),
            submitted_at: local_timestamp_for_turn(time_base, index),
        };
        runs.push(run_demo_request(system, provider, request)?);
    }

    Ok(TranscriptRun {
        session_id: ids.session_id,
        task_id: ids.task_id,
        runs,
    })
}

fn transcript_history_messages(runs: &[ClosureRun]) -> Vec<String> {
    runs.iter()
        .flat_map(|run| {
            [
                format!("user: {}", run.context_snapshot.input),
                format!("assistant: {}", run.assistant_response_text),
            ]
        })
        .collect()
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
