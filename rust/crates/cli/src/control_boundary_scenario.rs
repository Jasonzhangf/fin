use crate::{CliError, runtime_home::init_runtime_home, web_debug::CliDebugActionHandler};
use chrono::{Duration, Local};
use fin_config::{ProviderCredential, ProviderProtocol, ResolvedProviderConfig, SystemConfig};
use fin_debug_server::{ChatSendRequest, ChatSendResponse};
use fin_provider::{
    InferenceProvider, PreparedRequest, ProviderDescriptor, ProviderRequest, ProviderResponse,
};
use serde_json::Value;
use std::{collections::BTreeMap, fs, path::Path, path::PathBuf};

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ControlBoundaryScenarioRun {
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
        let output_text = if request.input.contains("wait for external logs") {
            wait_output()
        } else if request.input.contains("queued control task") {
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
            response_id: Some("control-boundary-scenario-response".into()),
            stop_reason: Some("end_turn".into()),
            status: 200,
            usage: None,
        })
    }
}

pub(crate) fn run_control_boundary_scenario(
    user_toml: &str,
    system: &SystemConfig,
    override_path: Option<&Path>,
) -> Result<ControlBoundaryScenarioRun, CliError> {
    let runtime_home = init_runtime_home(user_toml, system, override_path)?;
    let handler = CliDebugActionHandler::new(user_toml.into(), system.clone())?;
    let provider = ControlBoundaryProvider::new();
    let mut responses = Vec::new();

    for request in [
        chat("/new"),
        chat("seed control boundary scenario"),
        chat("/pause control-boundary-scenario"),
        chat("queued control task 1"),
        chat("queued control task 2"),
        chat("/resume-run"),
        status("/status control boundary scenario status"),
        chat("wait for external logs"),
    ] {
        let response = handler.send_message_for_provider(&runtime_home, request, &provider)?;
        responses.push(response);
    }

    force_pending_reminders_due(&runtime_home)?;
    let reminder_response = handler.send_message_for_provider(
        &runtime_home,
        status("/status after reminder fired"),
        &provider,
    )?;
    let binding = reminder_response.binding.clone();
    responses.push(reminder_response);

    force_supervisor_heartbeat_due(&runtime_home, &binding)?;
    let heartbeat_response = handler.send_message_for_provider(
        &runtime_home,
        status("/status after heartbeat due"),
        &provider,
    )?;
    let binding = heartbeat_response.binding.clone();
    responses.push(heartbeat_response);

    Ok(ControlBoundaryScenarioRun {
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
        "<fin_user_response>{user_response}</fin_user_response>\n<fin_control_feedback>{{\"origin\":\"model_output_contract_v1\",\"is_continuation\":true,\"is_simple_query\":false,\"candidate_task_id\":\"task-control-boundary-scenario\",\"candidate_topic_thread_id\":\"topic-control-boundary-scenario\",\"continuity_confidence\":{continuity_confidence},\"topic_shift_confidence\":{topic_shift_confidence},\"simple_query_confidence\":5,\"previous_topic_summary\":\"control boundary scenario\",\"current_topic_summary\":\"control boundary scenario\",\"note_candidate\":\"{summary}\",\"digest_candidate\":\"{summary}\",\"reason\":\"deterministic control-boundary scenario\"}}</fin_control_feedback>\n<fin_tool_calls>[{{\"tool_name\":\"reasoning.stop\",\"arguments\":{{\"summary\":\"{summary}\"}}}}]</fin_tool_calls>"
    )
}

fn wait_output() -> String {
    "<fin_user_response>先等待外部日志。</fin_user_response>\n<fin_control_feedback>{\"origin\":\"model_output_contract_v1\",\"is_continuation\":true,\"is_simple_query\":false,\"candidate_task_id\":\"task-control-boundary-scenario\",\"candidate_topic_thread_id\":\"topic-control-boundary-scenario\",\"continuity_confidence\":90,\"topic_shift_confidence\":10,\"simple_query_confidence\":8,\"previous_topic_summary\":\"control boundary scenario\",\"current_topic_summary\":\"control boundary scenario\",\"note_candidate\":\"scheduled async wait\",\"digest_candidate\":\"scheduled async wait\",\"reason\":\"waiting external result\"}</fin_control_feedback>\n<fin_tool_calls>[{\"tool_name\":\"wait.remind\",\"arguments\":{\"wait_minutes\":2,\"reminder\":\"check external logs\"}}]</fin_tool_calls>".into()
}

fn chat(message: &str) -> ChatSendRequest {
    ChatSendRequest {
        message: message.into(),
        input_kind: None,
        attachments: Vec::new(),
    }
}

fn status(message: &str) -> ChatSendRequest {
    ChatSendRequest {
        message: message.into(),
        input_kind: Some("status_probe".into()),
        attachments: Vec::new(),
    }
}

fn force_pending_reminders_due(runtime_home: &Path) -> Result<(), CliError> {
    let pending_path = runtime_home.join("runtime/reminders/pending.json");
    let mut reminders = read_json_value(&pending_path)?;
    let due_at = (Local::now() - Duration::minutes(3))
        .format("%Y-%m-%dT%H:%M:%S%:z")
        .to_string();
    if let Some(items) = reminders.as_array_mut() {
        for item in items {
            if !matches!(item.get("status").and_then(Value::as_str), Some("pending")) {
                continue;
            }
            item["scheduled_at"] = Value::String(due_at.clone());
        }
    }
    write_json_value(&pending_path, &reminders)
}

fn force_supervisor_heartbeat_due(
    runtime_home: &Path,
    binding: &fin_debug_server::DebugBinding,
) -> Result<(), CliError> {
    let Some(session_dir) = session_dir_from_binding(runtime_home, binding) else {
        return Err(CliError::Usage);
    };
    let latest_path = session_dir.join("control/supervisor/latest.json");
    let recent_path = session_dir.join("control/supervisor/recent_cycles.json");
    let next_check_at = (Local::now() - Duration::minutes(2))
        .format("%Y-%m-%dT%H:%M:%S%:z")
        .to_string();
    let completed_at = (Local::now() - Duration::minutes(4))
        .format("%Y-%m-%dT%H:%M:%S%:z")
        .to_string();

    let mut latest = read_json_value(&latest_path)?;
    patch_cycle_due(&mut latest, &completed_at, &next_check_at);
    write_json_value(&latest_path, &latest)?;

    let mut recent = read_json_value(&recent_path)?;
    if let Some(last) = recent.as_array_mut().and_then(|items| items.last_mut()) {
        patch_cycle_due(last, &completed_at, &next_check_at);
    }
    write_json_value(&recent_path, &recent)
}

fn patch_cycle_due(value: &mut Value, completed_at: &str, next_check_at: &str) {
    value["completed_at"] = Value::String(completed_at.into());
    value["next_check_at"] = Value::String(next_check_at.into());
    value["lease_ttl_ms"] = Value::from(1_000u64);
    value["heartbeat_interval_ms"] = Value::from(1_000u64);
}

fn session_dir_from_binding(
    runtime_home: &Path,
    binding: &fin_debug_server::DebugBinding,
) -> Option<PathBuf> {
    binding.session_id.as_deref().and_then(|session_id| {
        let root = runtime_home.join("sessions");
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
    })
}

fn read_json_value(path: &Path) -> Result<Value, CliError> {
    serde_json::from_str(
        &fs::read_to_string(path).map_err(|source| CliError::ReadFile {
            path: path.display().to_string(),
            source,
        })?,
    )
    .map_err(CliError::Serialize)
}

fn write_json_value(path: &Path, value: &Value) -> Result<(), CliError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|source| CliError::WriteFile {
            path: parent.display().to_string(),
            source,
        })?;
    }
    fs::write(
        path,
        serde_json::to_vec_pretty(value).map_err(CliError::Serialize)?,
    )
    .map_err(|source| CliError::WriteFile {
        path: path.display().to_string(),
        source,
    })
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
            "fin-control-boundary-scenario-{}",
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
    fn control_boundary_scenario_persists_queue_interrupt_and_scheduler_truth() {
        let user_toml = sample_user_toml();
        let system = map_system_config(&user_toml).expect("system config");
        let home = temp_runtime_home();
        let run = run_control_boundary_scenario(&user_toml, &system, Some(home.as_path()))
            .expect("control boundary scenario");
        let session_dir = find_session_dir(&home, &run.session_id).expect("session dir");
        assert_eq!(run.responses.len(), 10);
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
        assert!(latest_tick.contains("\"source\": \"supervisor_heartbeat_due\""));
        let recent_ticks =
            fs::read_to_string(session_dir.join("control/scheduler/recent_ticks.json"))
                .expect("recent ticks");
        assert!(recent_ticks.contains("\"source\": \"resume_run\""));
        assert!(recent_ticks.contains("\"source\": \"reminder_fired\""));
        assert!(recent_ticks.contains("\"source\": \"supervisor_heartbeat_due\""));
        let recent_merges = fs::read_to_string(session_dir.join("interrupts/recent_merges.json"))
            .expect("recent merges");
        assert!(recent_merges.contains("\"strategy\": \"resume_as_new_closure\""));
        let pending = fs::read_to_string(session_dir.join("queue/pending_inputs.json"))
            .expect("pending inputs");
        assert_eq!(pending.trim(), "[]");
        let reminders =
            fs::read_to_string(home.join("runtime/reminders/pending.json")).expect("reminders");
        assert!(reminders.contains("\"status\": \"fired\""));
        let messages =
            fs::read_to_string(session_dir.join("conversation/messages.json")).expect("messages");
        assert!(messages.contains("Reminder"));
        let heartbeats =
            fs::read_to_string(session_dir.join("control/supervisor/recent_heartbeats.json"))
                .expect("heartbeats");
        assert!(heartbeats.contains("\"due_for_tick\": true"));
        assert!(heartbeats.contains("\"stale_lease\": false"));
    }
}
