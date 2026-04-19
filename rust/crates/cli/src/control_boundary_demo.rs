use crate::{CliError, runtime_home::init_runtime_home, web_debug::CliDebugActionHandler};
use fin_config::{ProviderCredential, ProviderProtocol, ResolvedProviderConfig, SystemConfig};
use fin_debug_server::{ChatSendRequest, ChatSendResponse, DebugActionHandler};
use fin_provider::{
    InferenceProvider, PreparedRequest, ProviderDescriptor, ProviderRequest, ProviderResponse,
};
use std::{collections::BTreeMap, path::Path, path::PathBuf};

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ControlBoundaryDemoRun {
    pub(crate) runtime_home: PathBuf,
    pub(crate) session_id: String,
    pub(crate) task_id: String,
    pub(crate) responses: Vec<ChatSendResponse>,
}

#[derive(Debug, Clone)]
struct ControlBoundaryProvider {
    descriptor: ProviderDescriptor,
}

impl ControlBoundaryProvider {
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

impl InferenceProvider for ControlBoundaryProvider {
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
        let output_text = if request.input.contains("queued control task") {
            stop_output(
                "queued control task handled",
                "processed queued control boundary work",
                92,
                8,
            )
        } else {
            stop_output(
                "control boundary seed handled",
                "seeded control boundary session",
                85,
                15,
            )
        };
        Ok(ProviderResponse {
            provider_name: request.provider_name.clone(),
            model: request.model.clone(),
            output_text,
            response_id: Some("control-boundary-demo-response".into()),
            stop_reason: Some("end_turn".into()),
            status: 200,
        })
    }
}

pub(crate) fn run_control_boundary_demo(
    user_toml: &str,
    system: &SystemConfig,
    override_path: Option<&Path>,
) -> Result<ControlBoundaryDemoRun, CliError> {
    let runtime_home = init_runtime_home(user_toml, system, override_path)?;
    let handler = CliDebugActionHandler::new(user_toml.into(), system.clone())?;
    let provider = ControlBoundaryProvider::new();
    let mut binding = handler
        .read_binding(&runtime_home)
        .map_err(|value| CliError::ChannelConnectivity(value.to_string()))?;
    let mut responses = Vec::new();

    for message in [
        "/new",
        "seed control boundary demo",
        "/pause control-boundary-demo",
        "queued control task 1",
        "queued control task 2",
        "/resume-run",
        "/status control boundary demo status",
    ] {
        let response = handler.send_message_for_provider(
            &runtime_home,
            ChatSendRequest {
                message: message.into(),
                input_kind: None,
            },
            &provider,
        )?;
        binding = response.binding.clone();
        responses.push(response);
    }

    Ok(ControlBoundaryDemoRun {
        runtime_home,
        session_id: binding.session_id.clone().ok_or(CliError::Usage)?,
        task_id: binding.task_id.clone().ok_or(CliError::Usage)?,
        responses,
    })
}

fn stop_output(
    user_response: &str,
    summary: &str,
    continuity_confidence: u8,
    topic_shift_confidence: u8,
) -> String {
    format!(
        "<fin_user_response>{user_response}</fin_user_response>\n<fin_control_feedback>{{\"origin\":\"model_output_contract_v1\",\"is_continuation\":true,\"is_simple_query\":false,\"candidate_task_id\":\"task-control-boundary-demo\",\"candidate_topic_thread_id\":\"topic-control-boundary-demo\",\"continuity_confidence\":{continuity_confidence},\"topic_shift_confidence\":{topic_shift_confidence},\"simple_query_confidence\":5,\"previous_topic_summary\":\"control boundary demo\",\"current_topic_summary\":\"control boundary demo\",\"note_candidate\":\"{summary}\",\"digest_candidate\":\"{summary}\",\"reason\":\"deterministic control-boundary demo\"}}</fin_control_feedback>\n<fin_tool_calls>[{{\"tool_name\":\"reasoning.stop\",\"arguments\":{{\"summary\":\"{summary}\"}}}}]</fin_tool_calls>"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::map_system_config;
    use std::{
        fs,
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
            "fin-control-boundary-demo-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("time should work")
                .as_nanos()
        ))
    }

    fn find_session_dir(home: &Path, session_id: &str) -> Option<PathBuf> {
        let root = home.join("sessions");
        let years = fs::read_dir(root).ok()?;
        for year in years.flatten() {
            let months = fs::read_dir(year.path()).ok()?;
            for month in months.flatten() {
                let dir = month.path().join(session_id);
                if dir.exists() {
                    return Some(dir);
                }
            }
        }
        None
    }

    #[test]
    fn control_boundary_demo_persists_queue_interrupt_and_scheduler_truth() {
        let user_toml = sample_user_toml();
        let system = map_system_config(&user_toml).expect("system config");
        let home = temp_runtime_home();
        let run =
            run_control_boundary_demo(&user_toml, &system, Some(home.as_path())).expect("demo");
        let session_dir = find_session_dir(&home, &run.session_id).expect("session dir");
        assert_eq!(run.responses.len(), 7);
        assert!(session_dir.join("control/execution_state.json").exists());
        assert!(session_dir.join("control/pause_checkpoint.json").exists());
        assert!(session_dir.join("control/scheduler/latest.json").exists());
        assert!(
            session_dir
                .join("control/scheduler/latest_tick.json")
                .exists()
        );
        assert!(session_dir.join("control/supervisor/latest.json").exists());
        assert!(session_dir.join("interrupts/recent_segments.json").exists());
        assert!(session_dir.join("interrupts/recent_merges.json").exists());
        let latest_tick =
            fs::read_to_string(session_dir.join("control/scheduler/latest_tick.json"))
                .expect("latest tick");
        assert!(latest_tick.contains("\"source\": \"resume_run\""));
        assert!(latest_tick.contains("\"initial_pending_input_count\": 2"));
        let recent_merges = fs::read_to_string(session_dir.join("interrupts/recent_merges.json"))
            .expect("recent merges");
        assert!(recent_merges.contains("\"strategy\": \"resume_as_new_closure\""));
        let pending = fs::read_to_string(session_dir.join("queue/pending_inputs.json"))
            .expect("pending inputs");
        assert_eq!(pending.trim(), "[]");
    }
}
