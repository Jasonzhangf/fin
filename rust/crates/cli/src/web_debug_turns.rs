use super::*;
use crate::{
    agent_presence::{mark_entry_agent_busy, mark_entry_agent_failed, mark_entry_agent_idle},
    execution_segments::merge_segment_into_run,
    execution_state::{enqueue_pending_input, finalize_after_run, mark_failed, mark_running},
    runtime_current_snapshot::with_runtime_current_snapshot,
    runtime_home::persist_runtime_demo,
};

impl CliDebugActionHandler {
    pub(super) fn enqueue_request_notice(
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
        Ok(queued_notice_response(
            binding,
            status,
            pending_count,
            queued.as_ref().map(|value| value.pending_input_id.as_str()),
        ))
    }

    pub(super) fn run_interrupt_request(
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
        self.run_chat_turn_with_provider(
            runtime_home,
            binding,
            message.to_string(),
            "cli.user",
            Vec::new(),
            provider,
            None,
        )
    }

    pub(super) fn run_chat_turn_with_provider(
        &self,
        runtime_home: &Path,
        binding: DebugBinding,
        message: String,
        source: &str,
        attachment_summaries: Vec<InputAttachmentSummary>,
        provider: &impl InferenceProvider,
        merge_segment: Option<&fin_contracts::InterruptedSegmentRecord>,
    ) -> Result<ChatSendResponse, CliError> {
        self.run_turn_with_role(
            runtime_home,
            binding,
            message,
            source,
            attachment_summaries,
            provider,
            merge_segment,
            self.system.policy.entry_role.as_str(),
            true,
        )
    }

    pub(crate) fn run_project_turn_with_provider(
        &self,
        runtime_home: &Path,
        binding: DebugBinding,
        message: String,
        source: &str,
        attachment_summaries: Vec<InputAttachmentSummary>,
        provider: &impl InferenceProvider,
        merge_segment: Option<&fin_contracts::InterruptedSegmentRecord>,
    ) -> Result<ChatSendResponse, CliError> {
        with_runtime_current_snapshot(runtime_home, || {
            self.run_turn_with_role(
                runtime_home,
                binding.clone(),
                message,
                source,
                attachment_summaries,
                provider,
                merge_segment,
                "project",
                false,
            )
        })
    }

    fn run_turn_with_role(
        &self,
        runtime_home: &Path,
        binding: DebugBinding,
        message: String,
        source: &str,
        attachment_summaries: Vec<InputAttachmentSummary>,
        provider: &impl InferenceProvider,
        merge_segment: Option<&fin_contracts::InterruptedSegmentRecord>,
        role_id: &str,
        track_entry_presence: bool,
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
        if track_entry_presence {
            let _ = mark_entry_agent_busy(
                &self.system,
                runtime_home,
                &session_id,
                &task_id,
                &operation_id,
                &submitted_at,
                "received request; building context and starting inference",
            )?;
        }
        mark_running(runtime_home, &binding, &operation_id, &submitted_at)?;

        let run = match run_demo_request(
            &self.system,
            provider,
            DemoRequest {
                operation_id: operation_id.clone(),
                trace_id: trace_id.clone(),
                session_id: session_id.clone(),
                task_id: task_id.clone(),
                agent_name: None,
                role_id: Some(role_id.into()),
                input: message,
                source: source.into(),
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
                attachment_summaries,
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
                if track_entry_presence {
                    let _ = mark_entry_agent_failed(
                        &self.system,
                        runtime_home,
                        Some(session_id.as_str()),
                        Some(task_id.as_str()),
                        Some(operation_id.as_str()),
                        &local_timestamp_now(),
                        err.to_string().as_str(),
                    )?;
                }
                return Err(err);
            }
        };
        persist_runtime_demo(&self.user_toml, &self.system, &run, Some(runtime_home))?;
        let binding = if track_entry_presence {
            self.read_binding_internal(runtime_home)?
        } else {
            crate::session_binding::build_binding_for_session(
                runtime_home,
                &binding,
                &session_id,
                Some(task_id.as_str()),
            )?
        };
        finalize_after_run(runtime_home, &binding, &run)?;
        if track_entry_presence {
            let _ = mark_entry_agent_idle(
                &self.system,
                runtime_home,
                binding.session_id.as_deref(),
                binding.task_id.as_deref(),
                Some(operation_id.as_str()),
                &local_timestamp_now(),
                "frontstage idle; awaiting next request or worker feedback",
            )?;
        }
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
