use crate::{
    CliError,
    demo::{
        DemoRequest, demo_namespace_from_env, run_demo_request, runtime_home_override_from_env,
        sanitize_id_fragment,
    },
    time::{local_time_base, local_timestamp_for_turn},
};
use fin_config::{ProviderCredential, ProviderProtocol, ResolvedProviderConfig, SystemConfig};
use fin_contracts::DigestRecord;
use fin_provider::{
    InferenceProvider, PreparedRequest, ProviderDescriptor, ProviderRequest, ProviderResponse,
};
use fin_runtime::ClosureRun;
use std::{collections::BTreeMap, env};

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct MainlineDemoRun {
    pub(crate) session_id: String,
    pub(crate) task_id: String,
    pub(crate) runs: Vec<ClosureRun>,
}

#[derive(Debug, Clone)]
struct MainlineReceiptProvider {
    descriptor: ProviderDescriptor,
}

impl MainlineReceiptProvider {
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

impl InferenceProvider for MainlineReceiptProvider {
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
        let output_text = if request
            .input
            .starts_with("Continue the same turn with the latest tool results.")
        {
            stop_output(
                "工具结果已确认，现在收口。",
                "task-mainline-demo",
                "topic-mainline-demo",
                90,
                10,
                "tool followup done",
                "tool followup done",
                "tool result inspected",
                "tool-backed followup finished",
            )
        } else if request.input.contains("BANANA-42") {
            stop_output(
                "记住了",
                "task-mainline-demo",
                "topic-mainline-demo",
                95,
                5,
                "remembered banana code",
                "remembered banana code",
                "acknowledged memory request",
                "memory captured",
            )
        } else if request.input.contains("只回复 继续") {
            stop_output(
                "继续",
                "task-mainline-demo",
                "topic-mainline-demo",
                94,
                6,
                "short followup acknowledged",
                "short followup acknowledged",
                "continued same task",
                "followup acknowledged",
            )
        } else if request.input.contains("先检查 peer 再结束") {
            peer_round_output()
        } else {
            stop_output(
                "mainline demo completed",
                "task-mainline-demo",
                "topic-mainline-demo",
                60,
                40,
                "fallback path",
                "fallback path",
                "fallback scenario output",
                "fallback stop",
            )
        };
        Ok(ProviderResponse {
            provider_name: request.provider_name.clone(),
            model: request.model.clone(),
            output_text,
            response_id: Some("mainline-demo-response".into()),
            stop_reason: Some("end_turn".into()),
            status: 200,
        })
    }
}

pub(crate) fn run_mainline_demo(system: &SystemConfig) -> Result<MainlineDemoRun, CliError> {
    let provider = MainlineReceiptProvider::new();
    let ids = mainline_ids();
    let time_base = local_time_base();
    let turns = [
        "请记住代号 BANANA-42，并且只回复 记住了",
        "请保持简短，只回复 继续",
        "先检查 peer 再结束",
    ];
    let mut runs: Vec<ClosureRun> = Vec::with_capacity(turns.len());

    for (index, input) in turns.iter().enumerate() {
        let digests = runs
            .iter()
            .map(|run| run.digest.clone())
            .collect::<Vec<DigestRecord>>();
        let request = DemoRequest {
            operation_id: format!("op-{}-{:04}", ids.scope, index + 1),
            trace_id: format!("trace-{}-{:04}", ids.scope, index + 1),
            session_id: ids.session_id.clone(),
            task_id: ids.task_id.clone(),
            input: (*input).to_string(),
            recent_messages: history_messages(&runs),
            recent_digests: digests,
            recent_reasoning_views: runs.iter().map(|run| run.reasoning_view.clone()).collect(),
            recent_tool_records: runs
                .iter()
                .flat_map(|run| run.tool_records.iter().cloned())
                .collect(),
            project_label: Some("mainline-demo".into()),
            runtime_home: runtime_home_override_from_env().map(|path| path.display().to_string()),
            cwd: env::current_dir()
                .ok()
                .map(|path| path.display().to_string()),
            selected_paths: Vec::new(),
            submitted_at: local_timestamp_for_turn(time_base, index),
        };
        runs.push(run_demo_request(system, &provider, request)?);
    }

    Ok(MainlineDemoRun {
        session_id: ids.session_id,
        task_id: ids.task_id,
        runs,
    })
}

fn history_messages(runs: &[ClosureRun]) -> Vec<String> {
    runs.iter()
        .flat_map(|run| {
            [
                format!("user: {}", run.context_snapshot.input),
                format!("assistant: {}", run.assistant_response_text),
            ]
        })
        .collect()
}

struct MainlineIds {
    scope: String,
    session_id: String,
    task_id: String,
}

fn mainline_ids() -> MainlineIds {
    let namespace = demo_namespace_from_env()
        .unwrap_or_else(|| "mainline-demo".into())
        .trim()
        .to_string();
    let scope = sanitize_id_fragment(&format!("{namespace}-mainline"));
    MainlineIds {
        scope: scope.clone(),
        session_id: format!("session-{scope}"),
        task_id: format!("task-{scope}"),
    }
}

fn stop_output(
    user_response: &str,
    task_id: &str,
    topic_id: &str,
    continuity_confidence: u8,
    topic_shift_confidence: u8,
    note_candidate: &str,
    digest_candidate: &str,
    reason: &str,
    stop_summary: &str,
) -> String {
    format!(
        "<fin_user_response>{user_response}</fin_user_response>\n<fin_control_feedback>{{\"origin\":\"model_output_contract_v1\",\"is_continuation\":true,\"is_simple_query\":false,\"candidate_task_id\":\"{task_id}\",\"candidate_topic_thread_id\":\"{topic_id}\",\"continuity_confidence\":{continuity_confidence},\"topic_shift_confidence\":{topic_shift_confidence},\"simple_query_confidence\":5,\"previous_topic_summary\":\"mainline receipt demo\",\"current_topic_summary\":\"mainline receipt demo\",\"note_candidate\":\"{note_candidate}\",\"digest_candidate\":\"{digest_candidate}\",\"reason\":\"{reason}\"}}</fin_control_feedback>\n<fin_tool_calls>[{{\"tool_name\":\"reasoning.stop\",\"arguments\":{{\"summary\":\"{stop_summary}\"}}}}]</fin_tool_calls>"
    )
}

fn peer_round_output() -> String {
    "<fin_user_response>先查看 peer 列表。</fin_user_response>\n<fin_control_feedback>{\"origin\":\"model_output_contract_v1\",\"is_continuation\":true,\"is_simple_query\":false,\"candidate_task_id\":\"task-mainline-demo\",\"candidate_topic_thread_id\":\"topic-mainline-demo\",\"continuity_confidence\":88,\"topic_shift_confidence\":12,\"simple_query_confidence\":6,\"previous_topic_summary\":\"mainline receipt demo\",\"current_topic_summary\":\"mainline receipt demo\",\"note_candidate\":\"need peer list\",\"digest_candidate\":\"need peer list\",\"reason\":\"inspect peers before stopping\"}</fin_control_feedback>\n<fin_tool_calls>[{\"tool_name\":\"peer.list\",\"arguments\":{}}]</fin_tool_calls>".into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::map_system_config;

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

    #[test]
    fn mainline_demo_builds_multi_turn_history_with_tool_loop_last_turn() {
        let system = map_system_config(&sample_user_toml()).expect("system config");
        let run = run_mainline_demo(&system).expect("mainline demo");
        assert_eq!(run.runs.len(), 3);
        assert_eq!(run.runs[0].assistant_response_text, "记住了");
        assert_eq!(run.runs[1].assistant_response_text, "继续");
        assert_eq!(
            run.runs[2].assistant_response_text,
            "工具结果已确认，现在收口。"
        );
        assert_eq!(run.runs[2].round_records.len(), 2);
        assert_eq!(run.runs[2].provider_request_records.len(), 2);
        assert!(
            run.runs[2]
                .events
                .iter()
                .any(|event| event.event_type == "reasoning.auto_tool_roundtrip_completed")
        );
        assert_eq!(
            run.runs[1].context_snapshot.context.continuity_tail.len(),
            2
        );
        assert_eq!(
            run.runs[2].context_snapshot.context.continuity_tail.len(),
            4
        );
    }
}
