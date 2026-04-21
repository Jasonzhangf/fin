use crate::{
    CliError,
    attached_control_plane::run_attached_control_plane_cycle,
    channel_peer::ensure_builtin_qqbot_binding,
    chat_policy::{ChatDisposition, classify_request},
    config::default_provider_facade,
    demo::{DemoRequest, demo_identity, run_demo_request},
    execution_segments::{create_interrupted_segment, latest_open_segment},
    execution_state::{load_execution_state, pause_execution},
    runtime_home::{
        read_last_run_value, read_recent_digests, read_recent_reasoning_views,
        read_recent_tool_records, read_session_messages,
    },
    session_binding::resolve_binding_for_session,
    session_commands::try_handle_local_command,
    status_probe::build_status_probe_response,
    supervisor_cycle::run_supervisor_cycle,
    time::local_timestamp_now,
    transcript::scope_from_session_id,
    turn_ids::next_turn_index,
    web_debug_support::{
        build_debug_binding, last_run_field, queued_notice_response, request_source,
    },
};
use fin_config::SystemConfig;
use fin_contracts::InputAttachmentSummary;
use fin_debug_server::{ChatSendRequest, ChatSendResponse, DebugActionHandler, DebugBinding};
use fin_provider::{InferenceProvider, ProviderFacade};
use std::path::Path;

#[path = "web_debug_turns.rs"]
mod turns;

#[derive(Debug, Clone)]
pub(crate) struct CliDebugActionHandler {
    user_toml: String,
    system: SystemConfig,
    provider: ProviderFacade,
}

impl CliDebugActionHandler {
    pub(crate) fn new(user_toml: String, system: SystemConfig) -> Result<Self, CliError> {
        let provider = default_provider_facade(&system)?;
        Ok(Self {
            user_toml,
            system,
            provider,
        })
    }

    fn read_binding_internal(&self, runtime_home: &Path) -> Result<DebugBinding, CliError> {
        let last_run = read_last_run_value(runtime_home).ok();
        Ok(build_debug_binding(runtime_home, &last_run))
    }

    fn send_message_internal(
        &self,
        runtime_home: &Path,
        request: ChatSendRequest,
    ) -> Result<ChatSendResponse, CliError> {
        self.send_message_internal_with_provider_on_binding(
            runtime_home,
            request,
            None,
            &self.provider,
        )
    }

    pub(crate) fn send_message_for_provider(
        &self,
        runtime_home: &Path,
        request: ChatSendRequest,
        provider: &impl InferenceProvider,
    ) -> Result<ChatSendResponse, CliError> {
        self.send_message_internal_with_provider_on_binding(runtime_home, request, None, provider)
    }

    #[cfg(test)]
    fn send_message_internal_with_provider(
        &self,
        runtime_home: &Path,
        request: ChatSendRequest,
        provider: &impl InferenceProvider,
    ) -> Result<ChatSendResponse, CliError> {
        self.send_message_internal_with_provider_on_binding(runtime_home, request, None, provider)
    }

    fn send_message_internal_with_provider_on_binding(
        &self,
        runtime_home: &Path,
        request: ChatSendRequest,
        initial_binding: Option<DebugBinding>,
        provider: &impl InferenceProvider,
    ) -> Result<ChatSendResponse, CliError> {
        if request.message.trim().is_empty() {
            return Err(CliError::Usage);
        }

        let mut existing_binding =
            initial_binding.unwrap_or(self.read_binding_internal(runtime_home)?);
        let _ = ensure_builtin_qqbot_binding(runtime_home, existing_binding.session_id.as_deref())?;
        let _ = run_attached_control_plane_cycle(
            runtime_home,
            &self.system,
            &existing_binding,
            "web_debug_request",
            |binding, message, source, attachments, merge_segment| {
                self.run_chat_turn_with_provider(
                    runtime_home,
                    binding,
                    message,
                    &source,
                    attachments,
                    provider,
                    merge_segment,
                )
            },
            |binding, message, source, attachments, merge_segment| {
                self.run_project_turn_with_provider(
                    runtime_home,
                    binding,
                    message,
                    &source,
                    attachments,
                    provider,
                    merge_segment,
                )
            },
        )?;
        existing_binding = self.read_binding_internal(runtime_home)?;
        if request.is_status_probe() {
            return build_status_probe_response(runtime_home, existing_binding, &request);
        }
        if let Some(response) = try_handle_local_command(
            runtime_home,
            &self.system,
            Some(Path::new(&self.user_toml)),
            &request,
            &existing_binding,
        )? {
            if request.message.trim_start().starts_with("/resume-run")
                || request.message.trim_start().starts_with("/tick")
            {
                let source = if request.message.trim_start().starts_with("/resume-run") {
                    "resume_run"
                } else {
                    "manual_tick"
                };
                let cycle = run_supervisor_cycle(
                    runtime_home,
                    &response.binding,
                    source,
                    self.system.runtime.heartbeat_interval_ms,
                    &self.system.runtime.retention,
                    self.system.runtime.retention.recent_routing_decision_limit,
                    |binding, message, source, attachments, merge_segment| {
                        self.run_chat_turn_with_provider(
                            runtime_home,
                            binding,
                            message,
                            &source,
                            attachments,
                            provider,
                            merge_segment,
                        )
                    },
                )?;
                if let Some(drained) = cycle.tick.drive.last_response {
                    return Ok(drained);
                }
            }
            return Ok(response);
        }
        let state = load_execution_state(runtime_home, &existing_binding)?;
        match classify_request(&request, state.as_ref()) {
            ChatDisposition::StatusProbe => {
                build_status_probe_response(runtime_home, existing_binding, &request)
            }
            ChatDisposition::InterruptRequest { message } => self.run_interrupt_request(
                runtime_home,
                existing_binding,
                &message,
                provider,
                state,
            ),
            ChatDisposition::Queue {
                reason,
                parallel_candidate,
            } => self.enqueue_request_notice(
                runtime_home,
                existing_binding,
                &request,
                state.as_ref(),
                &reason,
                parallel_candidate,
                provider,
            ),
            ChatDisposition::RunNow => self.run_chat_turn_with_provider(
                runtime_home,
                existing_binding,
                request.message.trim().to_string(),
                request_source(&request),
                request.attachments.clone(),
                provider,
                None,
            ),
        }
    }

    pub(crate) fn resolve_session_binding(
        &self,
        runtime_home: &Path,
        session_id: &str,
    ) -> Result<DebugBinding, CliError> {
        let base = self.read_binding_internal(runtime_home)?;
        resolve_binding_for_session(runtime_home, &base, session_id)
    }

    pub(crate) fn send_chat_message_on_binding(
        &self,
        runtime_home: &Path,
        binding: DebugBinding,
        request: ChatSendRequest,
    ) -> Result<ChatSendResponse, CliError> {
        self.send_message_internal_with_provider_on_binding(
            runtime_home,
            request,
            Some(binding),
            &self.provider,
        )
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

#[cfg(test)]
#[path = "web_debug_tests.rs"]
mod web_debug_tests;
