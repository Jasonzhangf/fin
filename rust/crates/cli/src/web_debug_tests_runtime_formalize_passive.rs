use super::*;
use fin_provider::{PreparedRequest, ProviderDescriptor, ProviderRequest, ProviderResponse};

#[derive(Debug, Clone)]
struct FormalizePassiveProvider {
    descriptor: ProviderDescriptor,
    output_text: String,
    response_id: &'static str,
}

impl FormalizePassiveProvider {
    fn new(system: &SystemConfig, output_text: &str, response_id: &'static str) -> Self {
        Self {
            descriptor: ProviderDescriptor::from_resolved(
                system.default_provider_config().expect("default provider"),
            ),
            output_text: output_text.into(),
            response_id,
        }
    }
}

impl fin_provider::InferenceProvider for FormalizePassiveProvider {
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
        let output_text = if request.input.contains("/formalize") {
            self.output_text.clone()
        } else {
            "<fin_user_response>unexpected formalize input</fin_user_response>\n<fin_control_feedback>{\"origin\":\"model_output_contract_v1\",\"is_continuation\":false,\"is_simple_query\":true,\"task_completed\":false,\"is_simple_chat\":true,\"blocked\":false,\"needs_user_involve\":false,\"candidate_task_id\":null,\"candidate_topic_thread_id\":null,\"continuity_confidence\":20,\"topic_shift_confidence\":80,\"simple_query_confidence\":95,\"previous_topic_summary\":\"unknown\",\"current_topic_summary\":\"unknown\",\"completion_evidence\":[],\"final_conclusions\":[],\"blocked_reason\":null,\"what_needs_to_be_done_by_user\":null,\"note_candidate\":\"unexpected formalize input\",\"digest_candidate\":\"unexpected formalize input\",\"reason\":\"unexpected passive formalize test input\"}</fin_control_feedback>\n<fin_tool_calls>[{\"tool_name\":\"reasoning.stop\",\"arguments\":{\"summary\":\"unexpected passive formalize input\"}}]</fin_tool_calls>".into()
        };
        Ok(ProviderResponse {
            provider_name: request.provider_name.clone(),
            model: request.model.clone(),
            output_text,
            response_id: Some(self.response_id.into()),
            stop_reason: Some("end_turn".into()),
            status: 200,
            tool_calls: Vec::new(),
        })
    }
}

fn formalize_session_dir(home: &std::path::Path, session_id: &str) -> std::path::PathBuf {
    crate::session_binding::find_session_dir(home, session_id)
        .map(|(_, _, dir)| dir)
        .expect("session dir should exist")
}

fn formalize_candidate_message() -> &'static str {
    "please create a formal project task for building runtime routing state machine and session formalization flow"
}

fn managed_planning_output() -> &'static str {
    "<fin_user_response>已进入 managed path，并创建两个执行任务。</fin_user_response>\n<fin_control_feedback>{\"origin\":\"model_output_contract_v1\",\"is_continuation\":true,\"is_simple_query\":false,\"task_completed\":true,\"is_simple_chat\":false,\"blocked\":false,\"needs_user_involve\":false,\"candidate_task_id\":\"task-plan-managed\",\"candidate_topic_thread_id\":\"topic-plan-managed\",\"continuity_confidence\":97,\"topic_shift_confidence\":3,\"simple_query_confidence\":2,\"previous_topic_summary\":\"formalized task\",\"current_topic_summary\":\"managed planning\",\"completion_evidence\":[\"task-runtime-design created\",\"task-web-debug-timeline created\"],\"final_conclusions\":[\"managed planning closure completed\",\"managed task decomposition is ready\"],\"blocked_reason\":null,\"what_needs_to_be_done_by_user\":null,\"note_candidate\":\"managed planning created task board\",\"digest_candidate\":\"managed planning created task board\",\"reason\":\"complex work requires managed task decomposition\"}</fin_control_feedback>\n<fin_tool_calls>[{\"tool_name\":\"project.task.create\",\"arguments\":{\"task_id\":\"task-runtime-design\",\"title\":\"design runtime routing slice\",\"summary\":\"freeze passive formalize follow-up wiring\",\"status\":\"ready\"}},{\"tool_name\":\"project.task.create\",\"arguments\":{\"task_id\":\"task-web-debug-timeline\",\"title\":\"build framework timeline debug view\",\"summary\":\"render framework progress from session event truth\",\"status\":\"ready\"}},{\"tool_name\":\"reasoning.stop\",\"arguments\":{\"summary\":\"managed planning completed\"}}]</fin_tool_calls>"
}

fn direct_planning_output() -> &'static str {
    "<fin_user_response>已走 direct path，并写入最小 plan。</fin_user_response>\n<fin_control_feedback>{\"origin\":\"model_output_contract_v1\",\"is_continuation\":true,\"is_simple_query\":false,\"task_completed\":true,\"is_simple_chat\":false,\"blocked\":false,\"needs_user_involve\":false,\"candidate_task_id\":\"task-plan-direct\",\"candidate_topic_thread_id\":\"topic-plan-direct\",\"continuity_confidence\":96,\"topic_shift_confidence\":4,\"simple_query_confidence\":3,\"previous_topic_summary\":\"formalized task\",\"current_topic_summary\":\"direct planning\",\"completion_evidence\":[\"update_plan executed\",\"minimum direct plan artifact written\"],\"final_conclusions\":[\"direct planning closure completed\",\"bounded work stays on direct path\"],\"blocked_reason\":null,\"what_needs_to_be_done_by_user\":null,\"note_candidate\":\"direct planning updated plan\",\"digest_candidate\":\"direct planning updated plan\",\"reason\":\"bounded work can close through direct plan\"}</fin_control_feedback>\n<fin_tool_calls>[{\"tool_name\":\"update_plan\",\"arguments\":{\"explanation\":\"direct path selected after explicit post-formalize turn\",\"steps\":[{\"step\":\"Inspect current runtime wiring and verify the formalized task scope\",\"status\":\"in_progress\"},{\"step\":\"Land the bounded change and validate with focused tests\",\"status\":\"pending\"}]}},{\"tool_name\":\"reasoning.stop\",\"arguments\":{\"summary\":\"direct planning completed\"}}]</fin_tool_calls>"
}

fn assert_no_removed_hidden_formalize_event(events: &str) {
    assert!(!events.contains(REMOVED_LEGACY_HIDDEN_FOLLOWUP_EVENT));
}

#[test]
fn formalize_stops_after_bind_without_hidden_followup_turn() {
    let home = temp_runtime_home();
    ensure_runtime_home_layout(&home).expect("runtime home should init");
    let system = map_system_config(&sample_user_toml()).expect("system config");
    let handler =
        CliDebugActionHandler::new(sample_user_toml(), system).expect("handler should build");

    let tentative = handler
        .send_message_internal_with_provider(
            &home,
            ChatSendRequest {
                message: formalize_candidate_message().into(),
                input_kind: None,
                attachments: Vec::new(),
            },
            &static_provider(&handler.system),
        )
        .expect("tentative turn should run");
    let session_id = tentative.binding.session_id.clone().expect("session id");
    let session_dir = formalize_session_dir(&home, &session_id);

    let formalized = handler
        .send_message_internal_with_provider(
            &home,
            ChatSendRequest {
                message: "/formalize".into(),
                input_kind: None,
                attachments: Vec::new(),
            },
            &FormalizePassiveProvider::new(
                &handler.system,
                managed_planning_output(),
                "planning-managed-response",
            ),
        )
        .expect("formalize should run");

    assert_eq!(formalized.response_kind, "system_notice");
    let task_id = formalized.binding.task_id.clone().expect("task id");
    let messages =
        fs::read_to_string(session_dir.join("conversation/messages.json")).expect("messages");
    assert!(!messages.contains("已进入 managed path，并创建两个执行任务"));
    assert!(messages.contains("formalized current session into task"));

    let formalized_task =
        fs::read_to_string(session_dir.join(format!("tasks/registry/{task_id}.json")))
            .expect("formalized task should exist");
    assert!(formalized_task.contains("\"status\": \"ready\""));

    assert!(
        !session_dir
            .join("control/scheduler/latest_tick.json")
            .exists()
    );
    assert!(
        !session_dir
            .join("tasks/registry/task-runtime-design.json")
            .exists()
    );
    assert!(
        !session_dir
            .join("tasks/registry/task-web-debug-timeline.json")
            .exists()
    );
    let topic_latest = fs::read_to_string(session_dir.join("topics/latest.json"))
        .expect("formalize should bind topic truth");
    assert!(topic_latest.contains("\"task_id\":"));
    let last_run =
        fs::read_to_string(home.join("runtime/current/last_run.json")).expect("last run");
    assert!(last_run.contains(&format!("\"task_id\": \"{task_id}\"")));
    assert!(last_run.contains("\"topic_thread_id\":"));
    let current_task_board =
        fs::read_to_string(home.join("runtime/current/current_task_board.json"))
            .expect("current task board");
    assert!(current_task_board.contains(&format!("\"task_id\": \"{task_id}\"")));

    let events = fs::read_to_string(session_dir.join("events/stream.jsonl")).expect("events");
    assert!(events.contains("session.formalized"));
    assert_no_removed_hidden_formalize_event(&events);
    assert!(!events.contains("scheduler.tick_started"));
    assert!(!events.contains("scheduler.tick_completed"));
}

#[test]
fn formalize_does_not_write_plan_or_managed_tasks_without_explicit_next_turn() {
    let home = temp_runtime_home();
    ensure_runtime_home_layout(&home).expect("runtime home should init");
    let system = map_system_config(&sample_user_toml()).expect("system config");
    let handler =
        CliDebugActionHandler::new(sample_user_toml(), system).expect("handler should build");

    let tentative = handler
        .send_message_internal_with_provider(
            &home,
            ChatSendRequest {
                message: formalize_candidate_message().into(),
                input_kind: None,
                attachments: Vec::new(),
            },
            &static_provider(&handler.system),
        )
        .expect("tentative turn should run");
    let session_id = tentative.binding.session_id.clone().expect("session id");
    let session_dir = formalize_session_dir(&home, &session_id);

    let formalized = handler
        .send_message_internal_with_provider(
            &home,
            ChatSendRequest {
                message: "/formalize".into(),
                input_kind: None,
                attachments: Vec::new(),
            },
            &FormalizePassiveProvider::new(
                &handler.system,
                direct_planning_output(),
                "planning-direct-response",
            ),
        )
        .expect("formalize should run");

    assert_eq!(formalized.response_kind, "system_notice");
    let task_id = formalized.binding.task_id.clone().expect("task id");
    let messages =
        fs::read_to_string(session_dir.join("conversation/messages.json")).expect("messages");
    assert!(!messages.contains("已走 direct path，并写入最小 plan"));
    assert!(messages.contains("formalized current session into task"));

    let formalized_task =
        fs::read_to_string(session_dir.join(format!("tasks/registry/{task_id}.json")))
            .expect("formalized task");
    assert!(formalized_task.contains("\"status\": \"ready\""));
    assert!(!session_dir.join("tasks/plan/latest.json").exists());
    assert!(
        !home
            .join("runtime/current/current_plan_update.json")
            .exists()
    );
    assert!(
        !session_dir
            .join("control/scheduler/latest_tick.json")
            .exists()
    );
    let topic_latest = fs::read_to_string(session_dir.join("topics/latest.json"))
        .expect("formalize should bind topic truth");
    assert!(topic_latest.contains("\"task_id\":"));
    let last_run =
        fs::read_to_string(home.join("runtime/current/last_run.json")).expect("last run");
    assert!(last_run.contains(&format!("\"task_id\": \"{task_id}\"")));
    assert!(last_run.contains("\"topic_thread_id\":"));
    let current_task_board =
        fs::read_to_string(home.join("runtime/current/current_task_board.json"))
            .expect("current task board");
    assert!(current_task_board.contains(&format!("\"task_id\": \"{task_id}\"")));

    let events = fs::read_to_string(session_dir.join("events/stream.jsonl")).expect("events");
    assert!(events.contains("session.formalized"));
    assert_no_removed_hidden_formalize_event(&events);
    assert!(!events.contains("plan.updated"));
}
