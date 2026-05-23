use crate::{
    CliError,
    config::default_provider_facade,
    process_utils::append_log,
    runtime_home::{persist_runtime_session, read_last_run_value, resolved_runtime_home},
    session_run::{
        SessionRequest, run_session_request, sanitize_id_fragment, session_namespace_from_env,
    },
    time::{local_time_base, local_timestamp_for_turn, local_timestamp_now},
    transcript::{
        TranscriptScenario, TranscriptTurn, load_transcript_scenario, scope_from_session_id,
    },
};
use fin_config::SystemConfig;
use fin_provider::InferenceProvider;
use fin_runtime::ClosureRun;
use serde::{Deserialize, Serialize};
use std::{
    env, fs,
    path::{Path, PathBuf},
};

#[path = "provider_live_smoke_report.rs"]
mod provider_live_smoke_report;
use provider_live_smoke_report::{LiveTranscriptRun, build_report, write_report};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct ProviderLiveSmokeReport {
    pub(crate) run_id: String,
    pub(crate) session_id: String,
    pub(crate) task_id: String,
    pub(crate) turn_count: usize,
    pub(crate) provider_name: String,
    pub(crate) protocol: String,
    pub(crate) model: String,
    pub(crate) runtime_home: String,
    pub(crate) run_root: String,
    pub(crate) regression_log: String,
    pub(crate) receipt_path: String,
    pub(crate) projection_json: String,
    pub(crate) snapshot_json: String,
    pub(crate) current_context_path: String,
    pub(crate) current_provider_requests_path: String,
    pub(crate) current_provider_responses_path: String,
    pub(crate) current_rounds_path: String,
    pub(crate) current_steps_path: String,
    pub(crate) current_control_feedback_path: String,
    pub(crate) current_tool_records_path: String,
    pub(crate) control_feedback_origin: String,
    pub(crate) reasoning_stop_present: bool,
    pub(crate) session_recent_contexts_path: String,
    pub(crate) session_messages_path: String,
    pub(crate) session_recent_rounds_path: String,
    pub(crate) session_recent_steps_path: String,
    pub(crate) assistant_outputs_preview: Vec<String>,
    pub(crate) verified_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ProviderLiveSmokeLayout {
    run_id: String,
    run_root: PathBuf,
    runtime_home: PathBuf,
    regression_log: PathBuf,
    receipt_path: PathBuf,
}

pub(crate) fn load_or_default_live_smoke_scenario(
    transcript_path: Option<&Path>,
) -> Result<TranscriptScenario, CliError> {
    match transcript_path {
        Some(path) => load_transcript_scenario(path),
        None => Ok(default_live_smoke_scenario()),
    }
}

pub(crate) fn run_provider_live_smoke(
    user_toml: &str,
    system: &SystemConfig,
    runtime_home_override: Option<&Path>,
    transcript_path: Option<&Path>,
) -> Result<ProviderLiveSmokeReport, CliError> {
    let scenario = load_or_default_live_smoke_scenario(transcript_path)?;
    let provider = default_provider_facade(system)?;
    run_provider_live_smoke_with_provider(
        user_toml,
        system,
        &provider,
        runtime_home_override,
        &scenario,
    )
}

pub(crate) fn run_provider_live_smoke_with_provider(
    user_toml: &str,
    system: &SystemConfig,
    provider: &impl InferenceProvider,
    runtime_home_override: Option<&Path>,
    scenario: &TranscriptScenario,
) -> Result<ProviderLiveSmokeReport, CliError> {
    let layout = provider_live_smoke_layout(system, runtime_home_override);
    if let Some(parent) = layout.regression_log.parent() {
        fs::create_dir_all(parent).map_err(|source| CliError::WriteFile {
            path: parent.display().to_string(),
            source,
        })?;
    }
    append_log(
        &layout.regression_log,
        &format!(
            "[{}] provider-live-smoke start run_id={} runtime_home={}\n",
            local_timestamp_now(),
            layout.run_id,
            layout.runtime_home.display()
        ),
    )?;
    let transcript = run_live_transcript(
        system,
        provider,
        scenario,
        Some(layout.runtime_home.as_path()),
    )?;
    let mut artifacts = None;
    for run in &transcript.runs {
        artifacts = Some(persist_runtime_session(
            user_toml,
            system,
            run,
            Some(layout.runtime_home.as_path()),
        )?);
    }
    let artifacts = artifacts.ok_or(CliError::Usage)?;
    let last_run = read_last_run_value(&layout.runtime_home)?;
    let report = build_report(
        &layout,
        &artifacts.runtime_home,
        &transcript,
        &artifacts.projection_json,
        &artifacts.snapshot_json,
        &last_run,
    )?;
    write_report(&report, &layout.receipt_path)?;
    append_log(
        &layout.regression_log,
        &format!(
            "[{}] provider-live-smoke ok run_id={} session={} task={} turns={} receipt={}\n",
            report.verified_at,
            report.run_id,
            report.session_id,
            report.task_id,
            report.turn_count,
            report.receipt_path
        ),
    )?;
    Ok(report)
}

fn default_live_smoke_scenario() -> TranscriptScenario {
    TranscriptScenario {
        session_id: None,
        task_id: None,
        turns: vec![
            TranscriptTurn {
                input: "这是 fin 的真实 provider smoke。请只回复：已收到。".into(),
            },
            TranscriptTurn {
                input: "继续同一任务。请只回复：连续。".into(),
            },
            TranscriptTurn {
                input: "最后只用不超过20个字总结 smoke 已验证完成。".into(),
            },
        ],
    }
}

fn provider_live_smoke_layout(
    system: &SystemConfig,
    runtime_home_override: Option<&Path>,
) -> ProviderLiveSmokeLayout {
    if let Some(runtime_home) = runtime_home_override {
        let run_root = runtime_home
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| runtime_home.to_path_buf());
        let run_id =
            if runtime_home.file_name().and_then(|value| value.to_str()) == Some("runtime-home") {
                run_root
                    .file_name()
                    .and_then(|value| value.to_str())
                    .map(sanitize_id_fragment)
                    .filter(|value| !value.is_empty())
                    .unwrap_or_else(default_run_id)
            } else {
                runtime_home
                    .file_name()
                    .and_then(|value| value.to_str())
                    .map(sanitize_id_fragment)
                    .filter(|value| !value.is_empty())
                    .unwrap_or_else(default_run_id)
            };
        return ProviderLiveSmokeLayout {
            regression_log: run_root.join("provider-live-smoke.log"),
            receipt_path: run_root.join("provider-live-smoke-report.json"),
            run_id,
            run_root,
            runtime_home: runtime_home.to_path_buf(),
        };
    }

    let run_id = default_run_id();
    let base_runtime_home = resolved_runtime_home(system, None);
    let run_root = base_runtime_home.join("harness/runs").join(&run_id);
    ProviderLiveSmokeLayout {
        regression_log: run_root.join("provider-live-smoke.log"),
        receipt_path: run_root.join("provider-live-smoke-report.json"),
        runtime_home: run_root.join("runtime-home"),
        run_id,
        run_root,
    }
}

fn default_run_id() -> String {
    let namespace = session_namespace_from_env().unwrap_or_else(|| {
        let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S").to_string();
        format!("test-live-provider-{stamp}")
    });
    let run_id = sanitize_id_fragment(&namespace);
    if run_id.starts_with("test-") {
        run_id
    } else {
        format!("test-{run_id}")
    }
}

fn run_live_transcript(
    system: &SystemConfig,
    provider: &impl InferenceProvider,
    scenario: &TranscriptScenario,
    runtime_home: Option<&Path>,
) -> Result<LiveTranscriptRun, CliError> {
    let normalized = normalize_live_scenario(scenario.clone());
    let ids = live_transcript_ids(&normalized);
    let time_base = local_time_base();
    let cwd = env::current_dir()
        .ok()
        .map(|path| path.display().to_string());
    let runtime_home_string = runtime_home.map(|path| path.display().to_string());
    let mut runs: Vec<ClosureRun> = Vec::with_capacity(normalized.turns.len());

    for (index, turn) in normalized.turns.iter().enumerate() {
        let digests = runs
            .iter()
            .map(|run| run.digest.clone())
            .collect::<Vec<_>>();
        let request = SessionRequest {
            operation_id: format!("op-{}-{:04}", ids.scope, index + 1),
            trace_id: format!("trace-{}-{:04}", ids.scope, index + 1),
            session_id: ids.session_id.clone(),
            task_id: Some(ids.task_id.clone()),
            topic_thread_id: None,
            agent_name: Some("cli-live-smoke".into()),
            role_id: Some("system".into()),
            input: turn.input.clone(),
            source: "cli.user".into(),
            recent_messages: live_transcript_history_messages(&runs),
            recent_digests: digests,
            recent_reasoning_views: runs.iter().map(|run| run.reasoning_view.clone()).collect(),
            recent_tool_records: runs
                .iter()
                .flat_map(|run| run.tool_records.iter().cloned())
                .collect(),
            project_label: Some("provider-live-smoke".into()),
            runtime_home: runtime_home_string.clone(),
            cwd: cwd.clone(),
            selected_paths: vec!["rust".into(), "docs".into()],
            attachment_summaries: Vec::new(),
            submitted_at: local_timestamp_for_turn(time_base, index),
        };
        runs.push(run_session_request(system, provider, request)?);
    }

    Ok(LiveTranscriptRun {
        session_id: ids.session_id,
        task_id: ids.task_id,
        runs,
    })
}

fn live_transcript_history_messages(runs: &[ClosureRun]) -> Vec<String> {
    runs.iter()
        .flat_map(|run| {
            [
                format!("user: {}", run.context_snapshot.input),
                format!("assistant: {}", run.assistant_response_text),
            ]
        })
        .collect()
}

fn normalize_live_scenario(mut scenario: TranscriptScenario) -> TranscriptScenario {
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
        format!("{prefix}-provider-live-smoke")
    } else if cleaned.starts_with(&format!("{prefix}-")) {
        cleaned
    } else {
        format!("{prefix}-{cleaned}")
    }
}

struct LiveTranscriptIds {
    scope: String,
    session_id: String,
    task_id: String,
}

fn live_transcript_ids(scenario: &TranscriptScenario) -> LiveTranscriptIds {
    let namespace = session_namespace_from_env();
    let default_scope = namespace.as_deref().unwrap_or("provider-live-smoke");
    let scope = sanitize_id_fragment(default_scope);
    let session_id = scenario
        .session_id
        .clone()
        .unwrap_or_else(|| format!("session-{scope}"));
    let task_id = scenario
        .task_id
        .clone()
        .unwrap_or_else(|| format!("task-{scope}"));
    LiveTranscriptIds {
        scope: scope_from_session_id(&session_id),
        session_id,
        task_id,
    }
}

#[cfg(test)]
#[path = "provider_live_smoke_tests.rs"]
mod tests;
