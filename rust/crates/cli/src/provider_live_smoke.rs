use crate::{
    CliError,
    config::default_provider_facade,
    demo::{DemoRequest, demo_namespace_from_env, run_demo_request, sanitize_id_fragment},
    fs_utils::write_file,
    process_utils::append_log,
    runtime_home::{persist_runtime_demo, read_last_run_value, resolved_runtime_home},
    time::{local_time_base, local_timestamp_for_turn, local_timestamp_now},
    transcript::{
        TranscriptScenario, TranscriptTurn, load_transcript_scenario, scope_from_session_id,
    },
};
use fin_config::SystemConfig;
use fin_provider::InferenceProvider;
use fin_runtime::ClosureRun;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::{
    env, fs,
    path::{Path, PathBuf},
};

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
        artifacts = Some(persist_runtime_demo(
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
    let namespace = demo_namespace_from_env().unwrap_or_else(|| {
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
        let request = DemoRequest {
            operation_id: format!("op-{}-{:04}", ids.scope, index + 1),
            trace_id: format!("trace-{}-{:04}", ids.scope, index + 1),
            session_id: ids.session_id.clone(),
            task_id: ids.task_id.clone(),
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
        runs.push(run_demo_request(system, provider, request)?);
    }

    Ok(LiveTranscriptRun {
        session_id: ids.session_id,
        task_id: ids.task_id,
        runs,
    })
}

#[derive(Debug, Clone, PartialEq)]
struct LiveTranscriptRun {
    session_id: String,
    task_id: String,
    runs: Vec<ClosureRun>,
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
    let namespace = demo_namespace_from_env();
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

fn build_report(
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

    if request_records.is_empty()
        || response_records.is_empty()
        || round_records.is_empty()
        || step_records.is_empty()
    {
        return Err(CliError::InvalidInstallState(
            "provider live smoke wrote incomplete current runtime artifacts".into(),
        ));
    }
    if control_feedback
        .get("origin")
        .and_then(serde_json::Value::as_str)
        == Some("runtime_heuristic")
    {
        return Err(CliError::InvalidInstallState(
            "provider live smoke fell back to runtime_heuristic control feedback".into(),
        ));
    }
    let reasoning_stop_present = tool_records.iter().any(|record| {
        record.get("tool_name").and_then(serde_json::Value::as_str) == Some("reasoning.stop")
            && record.get("status").and_then(serde_json::Value::as_str) == Some("completed")
    });
    if session_messages.len() < transcript.runs.len() * 2 {
        return Err(CliError::InvalidInstallState(format!(
            "expected at least {} session messages, got {}",
            transcript.runs.len() * 2,
            session_messages.len()
        )));
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
        assistant_outputs_preview: transcript
            .runs
            .iter()
            .map(|run| truncate_preview(run.assistant_response_text.as_str()))
            .collect(),
        verified_at: local_timestamp_now(),
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

fn write_report(report: &ProviderLiveSmokeReport, path: &Path) -> Result<(), CliError> {
    write_file(
        path,
        serde_json::to_vec_pretty(&json!(report))
            .map_err(CliError::Serialize)?
            .as_slice(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::map_system_config;
    use fin_config::{ProviderCredential, ProviderProtocol, ResolvedProviderConfig};
    use fin_provider::{
        InferenceProvider, PreparedRequest, ProviderDescriptor, ProviderRequest, ProviderResponse,
    };
    use std::collections::BTreeMap;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn sample_user_toml() -> String {
        r#"
default_provider = "openai"

[providers.openai]
protocol = "open-ai-compatible"
base_url = "https://api.example.com/v1"
model = "gpt-5"
api_key_env = "OPENAI_API_KEY"
"#
        .into()
    }

    fn temp_runtime_home() -> PathBuf {
        std::env::temp_dir().join(format!(
            "fin-provider-live-smoke-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("time should work")
                .as_nanos()
        ))
    }

    #[derive(Debug, Clone)]
    struct SmokeContractProvider {
        descriptor: ProviderDescriptor,
    }

    impl SmokeContractProvider {
        fn new() -> Self {
            Self {
                descriptor: ProviderDescriptor::from_resolved(&ResolvedProviderConfig {
                    name: "openai".into(),
                    protocol: ProviderProtocol::OpenAiCompatible,
                    base_url: "https://api.example.com/v1".into(),
                    model: "gpt-5".into(),
                    credential: ProviderCredential::ApiKeyEnv {
                        env_var: "OPENAI_API_KEY".into(),
                    },
                    user_agent: None,
                    headers: BTreeMap::new(),
                }),
            }
        }
    }

    impl InferenceProvider for SmokeContractProvider {
        fn descriptor(&self) -> &ProviderDescriptor {
            &self.descriptor
        }

        fn prepare_request(&self, request: &ProviderRequest) -> PreparedRequest {
            self.descriptor.prepare_request(request)
        }

        fn execute_prepared(
            &self,
            request: &PreparedRequest,
        ) -> Result<ProviderResponse, fin_provider::ProviderError> {
            Ok(ProviderResponse {
                provider_name: request.provider_name.clone(),
                model: request.model.clone(),
                output_text: [
                    format!("<fin_user_response>ok:{}</fin_user_response>", request.input),
                    r#"<fin_control_feedback>{"origin":"model_output_contract_v1","is_continuation":true,"is_simple_query":false,"candidate_task_id":"task-provider-live-smoke","candidate_topic_thread_id":"topic-provider-live-smoke","continuity_confidence":95,"topic_shift_confidence":5,"simple_query_confidence":10,"previous_topic_summary":"provider smoke","current_topic_summary":"provider smoke","note_candidate":"smoke ok","digest_candidate":"smoke ok","reason":"contract smoke"}</fin_control_feedback>"#.to_string(),
                    r#"<fin_tool_calls>[{"tool_name":"reasoning.stop","arguments":{"summary":"smoke ok"}}]</fin_tool_calls>"#.to_string(),
                ]
                .join(""),
                response_id: Some("smoke-contract-response".into()),
                stop_reason: Some("end_turn".into()),
                status: 200,
            })
        }
    }

    #[test]
    fn provider_live_smoke_writes_receipt_and_runtime_truth() {
        let user_toml = sample_user_toml();
        let system = map_system_config(&user_toml).expect("system");
        let runtime_home = temp_runtime_home().join("runtime-home");
        let provider = SmokeContractProvider::new();
        let report = run_provider_live_smoke_with_provider(
            &user_toml,
            &system,
            &provider,
            Some(runtime_home.as_path()),
            &TranscriptScenario {
                session_id: Some("session-provider-live-smoke".into()),
                task_id: Some("task-provider-live-smoke".into()),
                turns: vec![
                    TranscriptTurn {
                        input: "first smoke turn".into(),
                    },
                    TranscriptTurn {
                        input: "second smoke turn".into(),
                    },
                    TranscriptTurn {
                        input: "third smoke turn".into(),
                    },
                ],
            },
        )
        .expect("smoke should succeed");
        assert_eq!(report.turn_count, 3);
        assert_eq!(report.provider_name, "openai");
        assert!(Path::new(&report.receipt_path).exists());
        assert!(Path::new(&report.current_provider_requests_path).exists());
        assert!(Path::new(&report.current_provider_responses_path).exists());
        assert!(Path::new(&report.session_recent_contexts_path).exists());
        assert!(Path::new(&report.session_messages_path).exists());
        assert_eq!(report.assistant_outputs_preview.len(), 3);
        assert_eq!(report.control_feedback_origin, "model_output_contract_v1");
        assert!(report.reasoning_stop_present);
        let receipt: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&report.receipt_path).expect("receipt json"))
                .expect("receipt parse");
        assert_eq!(receipt["turn_count"].as_u64(), Some(3));
        assert_eq!(
            receipt["session_id"].as_str(),
            Some("session-provider-live-smoke")
        );
    }
}
