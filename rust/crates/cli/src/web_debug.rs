use crate::{
    CliError,
    config::default_provider_facade,
    demo::{DemoRequest, demo_identity, sanitize_id_fragment, run_demo_request},
    runtime_home::{
        read_last_run_value, read_recent_digests, resolved_runtime_home, persist_runtime_demo,
    },
    time::local_timestamp_now,
    transcript::{rebuild_context_from_digests, scope_from_session_id},
};
use fin_config::SystemConfig;
use fin_debug_server::{
    ChatSendRequest, ChatSendResponse, DebugActionHandler, DebugBinding,
    serve_debug_mvp_with_handler,
};
use fin_provider::ProviderFacade;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub(crate) struct CliDebugActionHandler {
    user_toml: String,
    system: SystemConfig,
    provider: ProviderFacade,
}

impl CliDebugActionHandler {
    fn new(user_toml: String, system: SystemConfig) -> Result<Self, CliError> {
        let provider = default_provider_facade(&system)?;
        Ok(Self {
            user_toml,
            system,
            provider,
        })
    }

    fn read_binding_internal(&self, runtime_home: &Path) -> Result<DebugBinding, CliError> {
        let last_run = read_last_run_value(runtime_home).ok();
        let project_label = runtime_home
            .file_name()
            .and_then(|value| value.to_str())
            .filter(|value| !value.trim().is_empty())
            .unwrap_or("fin")
            .to_string();
        let project_id = sanitize_id_fragment(&project_label);

        Ok(DebugBinding {
            project_id: if project_id.is_empty() {
                "fin".into()
            } else {
                project_id
            },
            project_label,
            runtime_home: runtime_home.display().to_string(),
            session_id: last_run_field(&last_run, "session_id"),
            task_id: last_run_field(&last_run, "task_id"),
            session_messages_path: last_run_field(&last_run, "session_messages_path"),
            recent_contexts_path: last_run_field(&last_run, "session_recent_contexts_path"),
            recent_digests_path: last_run_field(&last_run, "session_recent_digests_path"),
        })
    }

    fn send_message_internal(
        &self,
        runtime_home: &Path,
        request: ChatSendRequest,
    ) -> Result<ChatSendResponse, CliError> {
        let message = request.message.trim();
        if message.is_empty() {
            return Err(CliError::Usage);
        }

        let existing_binding = self.read_binding_internal(runtime_home)?;
        let default_identity = demo_identity(Some("web-debug"));
        let session_id = existing_binding
            .session_id
            .clone()
            .unwrap_or(default_identity.session_id);
        let task_id = existing_binding
            .task_id
            .clone()
            .unwrap_or(default_identity.task_id);
        let digests = existing_binding
            .recent_digests_path
            .as_deref()
            .map(|relative| read_recent_digests(&runtime_home.join(relative)))
            .transpose()?
            .unwrap_or_default();
        let scope = scope_from_session_id(&session_id);
        let turn_index = digests.len();

        let run = run_demo_request(
            &self.system,
            &self.provider,
            DemoRequest {
                operation_id: format!("op-{scope}-{:04}", turn_index + 1),
                trace_id: format!("trace-{scope}-{:04}", turn_index + 1),
                session_id,
                task_id,
                input: message.to_string(),
                context: rebuild_context_from_digests(&digests),
                submitted_at: local_timestamp_now(),
            },
        )?;
        persist_runtime_demo(&self.user_toml, &self.system, &run, Some(runtime_home))?;
        let binding = self.read_binding_internal(runtime_home)?;

        Ok(ChatSendResponse {
            binding,
            answer: run.provider_response.output_text,
            digest_id: run.digest.digest_id,
            events_count: run.events.len(),
        })
    }
}

impl DebugActionHandler for CliDebugActionHandler {
    fn read_binding(&self, runtime_home: &Path) -> Result<DebugBinding, String> {
        self.read_binding_internal(runtime_home)
            .map_err(|err| err.to_string())
    }

    fn send_chat_message(
        &self,
        runtime_home: &Path,
        request: ChatSendRequest,
    ) -> Result<ChatSendResponse, String> {
        self.send_message_internal(runtime_home, request)
            .map_err(|err| err.to_string())
    }
}

fn last_run_field(last_run: &Option<serde_json::Value>, key: &str) -> Option<String> {
    last_run
        .as_ref()
        .and_then(|value| value.get(key))
        .and_then(serde_json::Value::as_str)
        .map(str::to_string)
}

pub(crate) fn serve_web_debug(
    user_toml: String,
    system: SystemConfig,
    runtime_home: PathBuf,
    bind_addr: &str,
) -> Result<(), CliError> {
    let handler = CliDebugActionHandler::new(user_toml, system)?;
    serve_debug_mvp_with_handler(&runtime_home, bind_addr, &handler)?;
    Ok(())
}

pub(crate) fn web_debug_runtime_home(
    system: &SystemConfig,
    runtime_home_override: Option<&Path>,
) -> PathBuf {
    resolved_runtime_home(system, runtime_home_override)
}
