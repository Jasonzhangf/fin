use crate::session_run::sanitize_id_fragment;
use fin_debug_server::{ChatSendRequest, ChatSendResponse, DebugBinding};
use std::path::Path;

pub(crate) fn request_source(request: &ChatSendRequest) -> &str {
    if request.input_kind.as_deref() == Some("channel_ingress") {
        "channel.qqbot"
    } else {
        "cli.user"
    }
}

pub(crate) fn last_run_field(last_run: &Option<serde_json::Value>, key: &str) -> Option<String> {
    last_run
        .as_ref()
        .and_then(|value| value.get(key))
        .and_then(serde_json::Value::as_str)
        .map(str::to_string)
}

pub(crate) fn build_debug_binding(
    runtime_home: &Path,
    last_run: &Option<serde_json::Value>,
) -> DebugBinding {
    let project_label = runtime_home
        .file_name()
        .and_then(|value| value.to_str())
        .filter(|value| !value.trim().is_empty())
        .unwrap_or("fin")
        .to_string();
    let project_id = sanitize_id_fragment(&project_label);
    DebugBinding {
        project_id: if project_id.is_empty() {
            "fin".into()
        } else {
            project_id
        },
        project_label,
        runtime_home: runtime_home.display().to_string(),
        session_id: last_run_field(last_run, "session_id"),
        task_id: last_run_field(last_run, "task_id"),
        session_messages_path: last_run_field(last_run, "session_messages_path"),
        recent_contexts_path: last_run_field(last_run, "session_recent_contexts_path"),
        recent_digests_path: last_run_field(last_run, "session_recent_digests_path"),
    }
}

pub(crate) fn queued_notice_response(
    binding: DebugBinding,
    status: &str,
    pending_count: usize,
    pending_input_id: Option<&str>,
) -> ChatSendResponse {
    ChatSendResponse {
        binding,
        answer: format!(
            "input queued: status={status} pending_inputs={pending_count}{}",
            pending_input_id
                .map(|value| format!(" id={value}"))
                .unwrap_or_default()
        ),
        digest_id: format!("digest-queued-{}", pending_input_id.unwrap_or("pending")),
        events_count: 0,
        response_kind: "system_notice".into(),
        freshness: Some("instant".into()),
        control_feedback: None,
        progress: None,
        note: None,
        routing_action: None,
    }
}
