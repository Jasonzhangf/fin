use crate::{
    CliError,
    channel_peer::{ensure_builtin_qqbot_binding, ensure_builtin_qqbot_peer},
    chat_policy::{ChatDisposition, classify_request},
    config::default_provider_facade,
    daemon_state::refresh_attached_daemon_state,
    demo::{DemoRequest, demo_identity, run_demo_request, sanitize_id_fragment},
    execution_segments::{create_interrupted_segment, latest_open_segment, merge_segment_into_run},
    execution_state::{
        clear_waiting_if_due, enqueue_pending_input, finalize_after_run, load_execution_state,
        mark_failed, mark_running, pause_execution,
    },
    reminder_scheduler::inject_due_reminders,
    runtime_home::{
        persist_runtime_demo, read_last_run_value, read_recent_digests,
        read_recent_reasoning_views, read_recent_tool_records, read_session_messages,
        resolved_runtime_home,
    },
    session_commands::try_handle_local_command,
    status_probe::build_status_probe_response,
    supervisor_cycle::run_supervisor_cycle,
    supervisor_heartbeat::run_supervisor_heartbeat,
    time::local_timestamp_now,
    transcript::scope_from_session_id,
    turn_ids::next_turn_index,
};
use fin_config::SystemConfig;
use fin_debug_server::{
    ChatSendRequest, ChatSendResponse, DebugActionHandler, DebugBinding,
    serve_debug_mvp_with_handler,
};
use fin_provider::{InferenceProvider, ProviderFacade};
use std::path::{Path, PathBuf};

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
        self.send_message_internal_with_provider(runtime_home, request, &self.provider)
    }

    pub(crate) fn send_message_for_provider(
        &self,
        runtime_home: &Path,
        request: ChatSendRequest,
        provider: &impl InferenceProvider,
    ) -> Result<ChatSendResponse, CliError> {
        self.send_message_internal_with_provider(runtime_home, request, provider)
    }

    fn send_message_internal_with_provider(
        &self,
        runtime_home: &Path,
        request: ChatSendRequest,
        provider: &impl InferenceProvider,
    ) -> Result<ChatSendResponse, CliError> {
        if request.message.trim().is_empty() {
            return Err(CliError::Usage);
        }

        let mut existing_binding = self.read_binding_internal(runtime_home)?;
        let _ = ensure_builtin_qqbot_binding(runtime_home, existing_binding.session_id.as_deref())?;
        let _ = run_supervisor_heartbeat(
            runtime_home,
            &existing_binding,
            "web_debug_request",
            self.system.runtime.heartbeat_interval_ms,
            &self.system.runtime.retention,
            self.system.runtime.retention.recent_routing_decision_limit,
            |binding, message, merge_segment| {
                self.run_chat_turn_with_provider(
                    runtime_home,
                    binding,
                    message,
                    provider,
                    merge_segment,
                )
            },
        )?;
        let fired = inject_due_reminders(runtime_home, &existing_binding)?;
        if fired > 0 {
            clear_waiting_if_due(runtime_home, &existing_binding, &local_timestamp_now())?;
            existing_binding = self.read_binding_internal(runtime_home)?;
            let _ = run_supervisor_cycle(
                runtime_home,
                &existing_binding,
                "reminder_fired",
                self.system.runtime.heartbeat_interval_ms,
                &self.system.runtime.retention,
                self.system.runtime.retention.recent_routing_decision_limit,
                |binding, message, merge_segment| {
                    self.run_chat_turn_with_provider(
                        runtime_home,
                        binding,
                        message,
                        provider,
                        merge_segment,
                    )
                },
            )?;
        }
        let _ = refresh_attached_daemon_state(
            runtime_home,
            &existing_binding,
            "web_debug_request",
            &self.system.runtime.retention,
            self.system.runtime.retention.recent_routing_decision_limit,
        )?;
        existing_binding = self.read_binding_internal(runtime_home)?;
        if request.is_status_probe() {
            return build_status_probe_response(runtime_home, existing_binding, &request);
        }
        if let Some(response) =
            try_handle_local_command(runtime_home, &self.system, &request, &existing_binding)?
        {
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
                    |binding, message, merge_segment| {
                        self.run_chat_turn_with_provider(
                            runtime_home,
                            binding,
                            message,
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
            ChatDisposition::Queue { reason } => self.enqueue_request_notice(
                runtime_home,
                existing_binding,
                &request,
                state.as_ref(),
                &reason,
            ),
            ChatDisposition::RunNow => self.run_chat_turn_with_provider(
                runtime_home,
                existing_binding,
                request.message.trim().to_string(),
                provider,
                None,
            ),
        }
    }

    fn enqueue_request_notice(
        &self,
        runtime_home: &Path,
        binding: DebugBinding,
        request: &ChatSendRequest,
        state: Option<&fin_contracts::ExecutionStateRecord>,
        reason: &str,
    ) -> Result<ChatSendResponse, CliError> {
        let queued = enqueue_pending_input(
            runtime_home,
            &binding,
            request,
            &local_timestamp_now(),
            reason,
        )?;
        let status = state.map(|value| value.status.as_str()).unwrap_or("queued");
        let pending_count =
            state.map_or(0, |value| value.pending_input_count) + usize::from(queued.is_some());
        Ok(ChatSendResponse {
            binding,
            answer: format!(
                "input queued: status={status} pending_inputs={pending_count}{}",
                queued
                    .as_ref()
                    .map(|value| format!(" id={}", value.pending_input_id))
                    .unwrap_or_default()
            ),
            digest_id: format!(
                "digest-queued-{}",
                queued
                    .as_ref()
                    .map(|value| value.pending_input_id.as_str())
                    .unwrap_or("pending")
            ),
            events_count: 0,
            response_kind: "system_notice".into(),
            freshness: Some("instant".into()),
            control_feedback: None,
            progress: None,
            note: None,
            routing_action: None,
        })
    }

    fn run_interrupt_request(
        &self,
        runtime_home: &Path,
        binding: DebugBinding,
        message: &str,
        provider: &impl InferenceProvider,
        state: Option<fin_contracts::ExecutionStateRecord>,
    ) -> Result<ChatSendResponse, CliError> {
        let mut open_segment = latest_open_segment(runtime_home, &binding)?;
        let should_create_segment = open_segment.is_none()
            && state
                .as_ref()
                .is_some_and(|value| matches!(value.status.as_str(), "running" | "paused"));
        if should_create_segment {
            if let Some((_, checkpoint)) = pause_execution(
                runtime_home,
                &binding,
                &local_timestamp_now(),
                Some("interrupt_request".into()),
            )? {
                open_segment = create_interrupted_segment(runtime_home, &binding, &checkpoint)?;
            }
        }
        let _ = open_segment;
        self.run_chat_turn_with_provider(runtime_home, binding, message.to_string(), provider, None)
    }

    fn run_chat_turn_with_provider(
        &self,
        runtime_home: &Path,
        binding: DebugBinding,
        message: String,
        provider: &impl InferenceProvider,
        merge_segment: Option<&fin_contracts::InterruptedSegmentRecord>,
    ) -> Result<ChatSendResponse, CliError> {
        let default_identity = demo_identity(Some("web-debug"));
        let last_run = read_last_run_value(runtime_home).ok();
        let session_id = binding
            .session_id
            .clone()
            .unwrap_or(default_identity.session_id);
        let task_id = binding.task_id.clone().unwrap_or(default_identity.task_id);
        let digests = binding
            .recent_digests_path
            .as_deref()
            .map(|relative| read_recent_digests(&runtime_home.join(relative)))
            .transpose()?
            .unwrap_or_default();
        let recent_messages = binding
            .session_messages_path
            .as_deref()
            .map(|relative| read_session_messages(&runtime_home.join(relative)))
            .transpose()?
            .unwrap_or_default()
            .into_iter()
            .map(|message| format!("{}: {}", message.role, message.content))
            .collect::<Vec<_>>();
        let mut observed_ops = binding
            .session_messages_path
            .as_deref()
            .map(|relative| read_session_messages(&runtime_home.join(relative)))
            .transpose()?
            .unwrap_or_default()
            .into_iter()
            .filter_map(|message| message.operation_id)
            .collect::<Vec<_>>();
        observed_ops.extend(digests.iter().map(|digest| {
            digest
                .digest_id
                .strip_prefix("digest-")
                .unwrap_or(digest.digest_id.as_str())
                .to_string()
        }));
        let recent_reasoning_views = last_run_field(&last_run, "session_recent_reasoning_path")
            .as_deref()
            .map(|relative| read_recent_reasoning_views(&runtime_home.join(relative)))
            .transpose()?
            .unwrap_or_default();
        let recent_tool_records = last_run_field(&last_run, "session_recent_tool_records_path")
            .as_deref()
            .map(|relative| read_recent_tool_records(&runtime_home.join(relative)))
            .transpose()?
            .unwrap_or_default();
        let scope = scope_from_session_id(&session_id);
        let turn_index = next_turn_index(
            last_run.as_ref(),
            binding.session_id.as_deref().and_then(|_| {
                last_run
                    .as_ref()
                    .and_then(|value| value.get("operation_id"))
                    .and_then(serde_json::Value::as_str)
            }),
            &observed_ops,
        );
        let submitted_at = local_timestamp_now();
        let operation_id = format!("op-{scope}-{turn_index:04}");
        let trace_id = format!("trace-{scope}-{turn_index:04}");
        mark_running(runtime_home, &binding, &operation_id, &submitted_at)?;

        let run = match run_demo_request(
            &self.system,
            provider,
            DemoRequest {
                operation_id: operation_id.clone(),
                trace_id: trace_id.clone(),
                session_id,
                task_id,
                input: message,
                recent_messages,
                recent_digests: digests,
                recent_reasoning_views,
                recent_tool_records,
                project_label: Some(binding.project_label.clone()),
                runtime_home: Some(runtime_home.display().to_string()),
                cwd: std::env::current_dir()
                    .ok()
                    .map(|path| path.display().to_string()),
                selected_paths: Vec::new(),
                submitted_at,
            },
        ) {
            Ok(run) => run,
            Err(err) => {
                mark_failed(
                    runtime_home,
                    &binding,
                    &operation_id,
                    &local_timestamp_now(),
                    err.to_string().as_str(),
                )?;
                return Err(err);
            }
        };
        persist_runtime_demo(&self.user_toml, &self.system, &run, Some(runtime_home))?;
        let binding = self.read_binding_internal(runtime_home)?;
        finalize_after_run(runtime_home, &binding, &run)?;
        if let Some(segment) = merge_segment {
            let _ = merge_segment_into_run(runtime_home, &binding, segment, &run)?;
        }

        Ok(ChatSendResponse {
            binding,
            answer: run.assistant_response_text,
            digest_id: run.digest.digest_id,
            events_count: run.events.len(),
            response_kind: "assistant_message".into(),
            freshness: None,
            control_feedback: None,
            progress: None,
            note: None,
            routing_action: Some(run.routing_action),
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
    ensure_builtin_qqbot_peer(&runtime_home)?;
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

#[cfg(test)]
#[path = "web_debug_tests.rs"]
mod web_debug_tests;
