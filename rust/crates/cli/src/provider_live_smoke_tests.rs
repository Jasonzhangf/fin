use super::*;
use crate::config::map_system_config;
use fin_config::{ProviderCredential, ProviderProtocol, ResolvedProviderConfig};
use fin_provider::{
    InferenceProvider, PreparedRequest, ProviderDescriptor, ProviderRequest, ProviderResponse,
};
use std::{
    collections::BTreeMap,
    fs,
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};

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
            ].join(""),
            response_id: Some("smoke-contract-response".into()),
            stop_reason: Some("end_turn".into()),
            status: 200,
            usage: None,
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

#[test]
fn provider_live_smoke_report_uses_session_dir_truth_without_last_run_projection_paths() {
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
            session_id: Some("session-provider-live-smoke-ledger".into()),
            task_id: Some("task-provider-live-smoke-ledger".into()),
            turns: vec![TranscriptTurn {
                input: "ledger-first report".into(),
            }],
        },
    )
    .expect("smoke should succeed");
    let last_run_path = runtime_home.join("runtime/current/last_run.json");
    let mut last_run: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&last_run_path).expect("last_run"))
            .expect("last_run json");
    let object = last_run.as_object_mut().expect("object");
    object.remove("session_messages_path");
    object.remove("session_recent_contexts_path");
    object.remove("session_recent_rounds_path");
    object.remove("session_recent_step_records_path");
    fs::write(
        &last_run_path,
        serde_json::to_vec_pretty(&last_run).expect("serialize"),
    )
    .expect("rewrite last_run");

    let receipt: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&report.receipt_path).expect("receipt json"))
            .expect("receipt parse");
    assert_eq!(
        receipt["session_id"].as_str(),
        Some("session-provider-live-smoke-ledger")
    );
    assert!(Path::new(&report.session_messages_path).exists());
    assert!(Path::new(&report.session_recent_contexts_path).exists());
    assert!(Path::new(&report.session_recent_rounds_path).exists());
    assert!(Path::new(&report.session_recent_steps_path).exists());
}
