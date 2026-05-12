use super::*;
use fin_provider::{PreparedRequest, ProviderDescriptor, ProviderRequest, ProviderResponse};

#[derive(Debug, Clone)]
struct AssignmentWorkerProvider {
    descriptor: ProviderDescriptor,
}

impl AssignmentWorkerProvider {
    fn new(system: &SystemConfig) -> Self {
        Self {
            descriptor: ProviderDescriptor::from_resolved(
                system.default_provider_config().expect("default provider"),
            ),
        }
    }
}

impl fin_provider::InferenceProvider for AssignmentWorkerProvider {
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
            .starts_with("Framework assignment: execute task now.")
        {
            "<fin_user_response>worker 已完成 assignment 执行。</fin_user_response>\n<fin_control_feedback>{\"origin\":\"model_output_contract_v1\",\"is_continuation\":true,\"is_simple_query\":false,\"candidate_task_id\":\"task-owner-dispatch\",\"candidate_topic_thread_id\":\"topic-owner-dispatch\",\"continuity_confidence\":94,\"topic_shift_confidence\":6,\"simple_query_confidence\":4,\"previous_topic_summary\":\"worker assignment\",\"current_topic_summary\":\"worker assignment\",\"note_candidate\":\"worker submitted result\",\"digest_candidate\":\"worker submitted result\",\"reason\":\"assignment execution completed\"}</fin_control_feedback>\n<fin_tool_calls>[{\"tool_name\":\"project.task.submit\",\"arguments\":{\"task_id\":\"task-owner-dispatch\",\"result_summary\":\"worker implemented ready task and attached receipts\"}},{\"tool_name\":\"reasoning.stop\",\"arguments\":{\"summary\":\"worker assignment completed\"}}]</fin_tool_calls>"
        } else {
            "<fin_user_response>unexpected assignment input</fin_user_response>\n<fin_control_feedback>{\"origin\":\"model_output_contract_v1\",\"is_continuation\":false,\"is_simple_query\":true,\"candidate_task_id\":null,\"candidate_topic_thread_id\":null,\"continuity_confidence\":20,\"topic_shift_confidence\":80,\"simple_query_confidence\":90,\"previous_topic_summary\":\"unknown\",\"current_topic_summary\":\"unknown\",\"reason\":\"unexpected assignment input\"}</fin_control_feedback>\n<fin_tool_calls>[{\"tool_name\":\"reasoning.stop\",\"arguments\":{\"summary\":\"unexpected assignment input\"}}]</fin_tool_calls>"
        };
        Ok(ProviderResponse {
            provider_name: request.provider_name.clone(),
            model: request.model.clone(),
            output_text: output_text.into(),
            response_id: Some("assignment-worker-response".into()),
            stop_reason: Some("end_turn".into()),
            status: 200,
        })
    }
}

#[test]
fn assignment_runtime_resume_executes_worker_turn_and_submits_task() {
    let home = temp_runtime_home();
    ensure_runtime_home_layout(&home).expect("runtime home should init");
    let mut system = map_system_config(&sample_user_toml()).expect("system config");
    system.runtime.device_name = Some("mbp".into());
    system
        .runtime
        .startup
        .project_agents
        .push(fin_config::ProjectAgentStartupConfig {
            project_id: "fin".into(),
            mode: fin_config::ProjectAgentMode::Local,
            project_root: Some("/tmp/fin".into()),
            endpoint: None,
            agent_name: Some("builder".into()),
            worker_budget: 2,
            always_on: true,
            auto_resume: true,
            auto_connect: true,
        });
    let handler =
        CliDebugActionHandler::new(sample_user_toml(), system).expect("handler should build");
    let session_dir = home.join("sessions/2026/05/session-owner-loop-dispatch");
    fs::create_dir_all(session_dir.join("conversation")).expect("conversation dir");
    fs::create_dir_all(session_dir.join("control")).expect("control dir");
    fs::create_dir_all(session_dir.join("queue")).expect("queue dir");
    fs::create_dir_all(session_dir.join("tasks/registry")).expect("registry dir");
    fs::create_dir_all(session_dir.join("digests")).expect("digests dir");
    fs::create_dir_all(session_dir.join("context")).expect("context dir");
    fs::create_dir_all(session_dir.join("reasoning")).expect("reasoning dir");
    fs::create_dir_all(session_dir.join("tools")).expect("tools dir");
    fs::create_dir_all(session_dir.join("events")).expect("events dir");
    write_file(&session_dir.join("conversation/messages.json"), b"[]").expect("messages");
    write_file(&session_dir.join("digests/recent_digests.json"), b"[]").expect("digests");
    write_file(&session_dir.join("context/recent_contexts.json"), b"[]").expect("contexts");
    write_file(
        &session_dir.join("reasoning/recent_reasoning_views.json"),
        b"[]",
    )
    .expect("reasoning");
    write_file(&session_dir.join("tools/recent_tool_records.json"), b"[]").expect("tools");
    write_file(&session_dir.join("events/stream.jsonl"), b"").expect("events");
    write_file(
        &session_dir.join("context/current_context.json"),
        br#"{"project":{"primary_project":{"project_id":"fin","label":"fin"}}}"#,
    )
    .expect("current context");
    write_file(&session_dir.join("queue/pending_inputs.json"), b"[]").expect("pending");
    write_file(
        &session_dir.join("tasks/registry/task-owner-dispatch.json"),
        br#"{
  "task_id":"task-owner-dispatch",
  "session_id":"session-owner-loop-dispatch",
  "title":"owner dispatch task",
  "summary":"ready task assigned to worker",
  "status":"claimed",
  "review_owner_worker_id":"worker-system",
  "claimed_by_worker_id":"worker-builder",
  "created_at":"2026-04-21T12:15:00+08:00",
  "updated_at":"2026-04-21T12:15:00+08:00"
}"#,
    )
    .expect("task registry");
    write_file(
        &home.join("runtime/assignments/pending.json"),
        br#"[
  {
    "assignment_id":"assign-task-owner-dispatch-01",
    "peer_id":"local-worker-builder",
    "project_id":"fin",
    "session_id":"session-owner-loop-dispatch",
    "task_id":"task-owner-dispatch",
    "target_worker_id":"worker-builder",
    "target_agent_name":"builder",
    "requested_role_id":"project",
    "owner_worker_id":"worker-system",
    "task_summary":"implement ready task and report receipts",
    "created_at":"2026-04-21T12:16:00+08:00",
    "status":"pending"
  }
]"#,
    )
    .expect("assignment queue");

    let report = crate::assignment_runtime_resume::drive_ready_assignment_resumes(
        &home,
        &handler.system,
        "assignment_runtime_resume",
        "2026-04-21T12:17:00+08:00",
        |binding, agent_name, message, source, attachments, merge_segment| {
            handler.run_project_turn_with_provider(
                &home,
                binding,
                message,
                &source,
                attachments,
                &AssignmentWorkerProvider::new(&handler.system),
                merge_segment,
                agent_name.as_deref(),
            )
        },
    )
    .expect("assignment runtime resume");

    assert_eq!(report.attempted_count, 1);
    assert_eq!(report.drove_count, 1);
    assert_eq!(report.completed_count, 1);

    let task_json = fs::read_to_string(session_dir.join("tasks/registry/task-owner-dispatch.json"))
        .expect("task json");
    assert!(task_json.contains("\"status\": \"submitted\""));
    assert!(task_json.contains("\"submitted_by_worker_id\": \"worker-builder\""));
    assert!(task_json.contains("worker implemented ready task and attached receipts"));

    let pending =
        fs::read_to_string(session_dir.join("queue/pending_inputs.json")).expect("pending queue");
    assert_eq!(pending.trim(), "[]");

    let assignment_queue =
        fs::read_to_string(home.join("runtime/assignments/pending.json")).expect("assignments");
    assert!(assignment_queue.contains("\"status\": \"completed\""));
    assert!(assignment_queue.contains("\"target_agent_name\": \"builder\""));

    let current_assignment =
        fs::read_to_string(home.join("runtime/current/current_assignment_summary.json"))
            .expect("assignment summary");
    assert!(current_assignment.contains("\"worker_id\": \"worker-builder\""));
    let builder_presence = fs::read_to_string(home.join("runtime/agents/state/mbp.builder.json"))
        .expect("builder presence");
    assert!(builder_presence.contains("\"role_id\": \"project\""));
    assert!(builder_presence.contains("\"status\": \"idle\""));
    assert!(builder_presence.contains("\"current_session_id\": \"session-owner-loop-dispatch\""));
    assert!(builder_presence.contains("\"current_task_id\": \"task-owner-dispatch\""));

    let latest_tools =
        fs::read_to_string(session_dir.join("tools/recent_tool_records.json")).expect("tools");
    assert!(latest_tools.contains("\"tool_name\": \"project.task.submit\""));
}
