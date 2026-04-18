use crate::{
    CliError,
    config::default_provider_facade,
    demo::{DemoRequest, demo_identity, run_demo_request, sanitize_id_fragment},
    runtime_home::{
        persist_runtime_demo, read_last_run_value, read_recent_digests,
        read_recent_reasoning_views, read_recent_tool_records, read_session_messages,
        resolved_runtime_home,
    },
    status_probe::build_status_probe_response,
    time::local_timestamp_now,
    turn_ids::next_turn_index,
    transcript::scope_from_session_id,
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
        if request.is_status_probe() {
            return build_status_probe_response(runtime_home, existing_binding, &request);
        }
        let default_identity = demo_identity(Some("web-debug"));
        let last_run = read_last_run_value(runtime_home).ok();
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
        let recent_messages = existing_binding
            .session_messages_path
            .as_deref()
            .map(|relative| read_session_messages(&runtime_home.join(relative)))
            .transpose()?
            .unwrap_or_default()
            .into_iter()
            .map(|message| format!("{}: {}", message.role, message.content))
            .collect::<Vec<_>>();
        let mut observed_ops = existing_binding
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
            existing_binding
                .session_id
                .as_deref()
                .and_then(|_| {
                    last_run
                        .as_ref()
                        .and_then(|value| value.get("operation_id"))
                        .and_then(serde_json::Value::as_str)
                }),
            &observed_ops,
        );

        let run = run_demo_request(
            &self.system,
            &self.provider,
            DemoRequest {
                operation_id: format!("op-{scope}-{turn_index:04}"),
                trace_id: format!("trace-{scope}-{turn_index:04}"),
                session_id,
                task_id,
                input: message.to_string(),
                recent_messages,
                recent_digests: digests,
                recent_reasoning_views,
                recent_tool_records,
                project_label: Some(existing_binding.project_label.clone()),
                runtime_home: Some(runtime_home.display().to_string()),
                cwd: std::env::current_dir()
                    .ok()
                    .map(|path| path.display().to_string()),
                selected_paths: Vec::new(),
                submitted_at: local_timestamp_now(),
            },
        )?;
        persist_runtime_demo(&self.user_toml, &self.system, &run, Some(runtime_home))?;
        let binding = self.read_binding_internal(runtime_home)?;

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        config::map_system_config,
        fs_utils::write_file,
        runtime_home::ensure_runtime_home_layout,
    };
    use fin_contracts::{ControlFeedback, ExecutionNote, ProgressBlock};
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
            "fin-status-probe-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("time should work")
                .as_nanos()
        ))
    }

    #[test]
    fn status_probe_returns_latest_framework_state_without_new_closure() {
        let home = temp_runtime_home();
        ensure_runtime_home_layout(&home).expect("runtime home should init");
        let system = map_system_config(&sample_user_toml()).expect("system config");
        let handler =
            CliDebugActionHandler::new(sample_user_toml(), system).expect("handler should build");
        let session_dir = home.join("sessions/2026/04/session-web-debug");
        fs::create_dir_all(session_dir.join("conversation")).expect("conversation dir");
        fs::create_dir_all(session_dir.join("progress")).expect("progress dir");
        fs::create_dir_all(session_dir.join("notes")).expect("notes dir");
        fs::create_dir_all(session_dir.join("control")).expect("control dir");
        fs::create_dir_all(session_dir.join("digests")).expect("digests dir");
        write_file(
            &session_dir.join("conversation/messages.json"),
            br#"[{"message_id":"user-1","role":"user","content":"build it","created_at":"2026-04-18T08:10:00+08:00","session_id":"session-web-debug","task_id":"task-web-debug"}]"#,
        )
        .expect("messages should write");
        write_file(
            &session_dir.join("digests/recent_digests.json"),
            br#"[{"digest_id":"digest-existing","closure_id":"closure-existing","session_id":"session-web-debug","task_id":"task-web-debug","summary":"existing digest","continuity_tail":[],"note_refs":[],"artifact_candidates":[],"created_at":"2026-04-18T08:10:01+08:00"}]"#,
        )
        .expect("digests should write");
        write_file(
            &session_dir.join("progress/latest.json"),
            serde_json::to_vec_pretty(&ProgressBlock {
                progress_id: "progress-1".into(),
                refs: fin_contracts::EntityRefs {
                    session_id: Some("session-web-debug".into()),
                    task_id: Some("task-web-debug".into()),
                    ..fin_contracts::EntityRefs::default()
                },
                phase: "running".into(),
                blocker: None,
                next_step: Some("finish current closure".into()),
                health_hint: Some("healthy".into()),
                tool_snapshots: vec![],
            })
            .expect("progress json")
            .as_slice(),
        )
        .expect("progress should write");
        write_file(
            &session_dir.join("notes/latest.json"),
            serde_json::to_vec_pretty(&ExecutionNote {
                note_id: "note-1".into(),
                refs: fin_contracts::EntityRefs {
                    session_id: Some("session-web-debug".into()),
                    task_id: Some("task-web-debug".into()),
                    ..fin_contracts::EntityRefs::default()
                },
                summary: "currently applying runtime changes".into(),
                decision: None,
                lesson: None,
                blocker: None,
                next_step: Some("wait for verification".into()),
                control_feedback: None,
                created_at: "2026-04-18T08:10:02+08:00".into(),
            })
            .expect("note json")
            .as_slice(),
        )
        .expect("note should write");
        write_file(
            &session_dir.join("control/latest.json"),
            serde_json::to_vec_pretty(&ControlFeedback {
                origin: "runtime_heuristic".into(),
                is_continuation: true,
                continuity_confidence: 93,
                topic_shift_confidence: 7,
                simple_query_confidence: 10,
                reason: "current task still active".into(),
                ..ControlFeedback::default()
            })
            .expect("control json")
            .as_slice(),
        )
        .expect("control should write");
        write_file(
            &home.join("runtime/current/last_run.json"),
            br#"{
  "session_id":"session-web-debug",
  "task_id":"task-web-debug",
  "digest_id":"digest-existing",
  "session_messages_path":"sessions/2026/04/session-web-debug/conversation/messages.json",
  "session_control_feedback_path":"sessions/2026/04/session-web-debug/control/latest.json"
}"#,
        )
        .expect("last_run should write");

        let before_messages =
            fs::read_to_string(session_dir.join("conversation/messages.json")).expect("before");
        let before_digests =
            fs::read_to_string(session_dir.join("digests/recent_digests.json")).expect("before");

        let response = handler
            .send_message_internal(
                &home,
                ChatSendRequest {
                    message: "/status current?".into(),
                    input_kind: Some("status_probe".into()),
                },
            )
            .expect("status probe should work");

        assert_eq!(response.response_kind, "status_probe");
        assert_eq!(response.freshness.as_deref(), Some("live"));
        assert_eq!(response.digest_id, "digest-existing");
        assert_eq!(response.events_count, 0);
        assert!(response.answer.contains("status probe (live)"));
        assert!(response.answer.contains("phase=running"));
        assert!(response.answer.contains("currently applying runtime changes"));
        assert_eq!(
            response.control_feedback.as_ref().map(|value| value.continuity_confidence),
            Some(93)
        );
        assert_eq!(
            fs::read_to_string(session_dir.join("conversation/messages.json")).expect("after"),
            before_messages
        );
        assert_eq!(
            fs::read_to_string(session_dir.join("digests/recent_digests.json")).expect("after"),
            before_digests
        );
    }
}
