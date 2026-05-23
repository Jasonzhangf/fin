use super::*;
use fin_provider::{PreparedRequest, ProviderDescriptor, ProviderRequest, ProviderResponse};

#[derive(Debug, Clone)]
struct PlanningKickoffProvider {
    descriptor: ProviderDescriptor,
    output_text: String,
    response_id: &'static str,
}

impl PlanningKickoffProvider {
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

impl fin_provider::InferenceProvider for PlanningKickoffProvider {
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
            .starts_with("Framework planning kickoff: the task is now formalized.")
        {
            self.output_text.clone()
        } else {
            "<fin_user_response>unexpected planning input</fin_user_response>\n<fin_control_feedback>{\"origin\":\"model_output_contract_v1\",\"is_continuation\":false,\"is_simple_query\":true,\"candidate_task_id\":null,\"candidate_topic_thread_id\":null,\"continuity_confidence\":20,\"topic_shift_confidence\":80,\"simple_query_confidence\":95,\"previous_topic_summary\":\"unknown\",\"current_topic_summary\":\"unknown\",\"note_candidate\":\"unexpected planning input\",\"digest_candidate\":\"unexpected planning input\",\"reason\":\"unexpected planning kickoff input\"}</fin_control_feedback>\n<fin_tool_calls>[{\"tool_name\":\"reasoning.stop\",\"arguments\":{\"summary\":\"unexpected planning kickoff input\"}}]</fin_tool_calls>".into()
        };
        Ok(ProviderResponse {
            provider_name: request.provider_name.clone(),
            model: request.model.clone(),
            output_text,
            response_id: Some(self.response_id.into()),
            stop_reason: Some("end_turn".into()),
            status: 200,
            usage: None,
        })
    }
}

fn kickoff_session_dir(home: &std::path::Path, session_id: &str) -> std::path::PathBuf {
    crate::session_binding::find_session_dir(home, session_id)
        .map(|(_, _, dir)| dir)
        .expect("session dir should exist")
}

fn formalize_candidate_message() -> &'static str {
    "please create a formal project task for building runtime routing state machine and session formalization flow"
}

fn managed_planning_output() -> &'static str {
    "<fin_user_response>已进入 managed path，并创建两个执行任务。</fin_user_response>\n<fin_control_feedback>{\"origin\":\"model_output_contract_v1\",\"is_continuation\":true,\"is_simple_query\":false,\"candidate_task_id\":\"task-plan-managed\",\"candidate_topic_thread_id\":\"topic-plan-managed\",\"continuity_confidence\":97,\"topic_shift_confidence\":3,\"simple_query_confidence\":2,\"previous_topic_summary\":\"formalized task\",\"current_topic_summary\":\"managed planning\",\"note_candidate\":\"managed planning created task board\",\"digest_candidate\":\"managed planning created task board\",\"reason\":\"complex work requires managed task decomposition\"}</fin_control_feedback>\n<fin_tool_calls>[{\"tool_name\":\"project.task.create\",\"arguments\":{\"task_id\":\"task-runtime-design\",\"title\":\"design runtime routing slice\",\"summary\":\"freeze routing/planning kickoff wiring\",\"status\":\"ready\"}},{\"tool_name\":\"project.task.create\",\"arguments\":{\"task_id\":\"task-web-debug-timeline\",\"title\":\"build framework timeline debug view\",\"summary\":\"render framework progress from session event truth\",\"status\":\"ready\"}},{\"tool_name\":\"reasoning.stop\",\"arguments\":{\"summary\":\"managed planning completed\"}}]</fin_tool_calls>"
}

fn direct_planning_output() -> &'static str {
    "<fin_user_response>已走 direct path，并写入最小 plan。</fin_user_response>\n<fin_control_feedback>{\"origin\":\"model_output_contract_v1\",\"is_continuation\":true,\"is_simple_query\":false,\"candidate_task_id\":\"task-plan-direct\",\"candidate_topic_thread_id\":\"topic-plan-direct\",\"continuity_confidence\":96,\"topic_shift_confidence\":4,\"simple_query_confidence\":3,\"previous_topic_summary\":\"formalized task\",\"current_topic_summary\":\"direct planning\",\"note_candidate\":\"direct planning updated plan\",\"digest_candidate\":\"direct planning updated plan\",\"reason\":\"bounded work can close through direct plan\"}</fin_control_feedback>\n<fin_tool_calls>[{\"tool_name\":\"update_plan\",\"arguments\":{\"explanation\":\"direct path selected after framework planning kickoff\",\"steps\":[{\"step\":\"Inspect current runtime wiring and verify the formalized task scope\",\"status\":\"in_progress\"},{\"step\":\"Land the bounded change and validate with focused tests\",\"status\":\"pending\"}]}},{\"tool_name\":\"reasoning.stop\",\"arguments\":{\"summary\":\"direct planning completed\"}}]</fin_tool_calls>"
}

#[test]
fn formalize_auto_kickoff_runs_managed_planning_and_forms_task_board() {
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
    assert!(!tentative.answer.contains("/formalize"));
    assert!(
        tentative
            .routing_action
            .as_ref()
            .and_then(|action| action.prompt_text.as_deref())
            .is_some_and(|text| text.contains("/formalize"))
    );
    let session_id = tentative.binding.session_id.clone().expect("session id");
    let session_dir = kickoff_session_dir(&home, &session_id);

    let formalized = handler
        .send_message_internal_with_provider(
            &home,
            ChatSendRequest {
                message: "/formalize".into(),
                input_kind: None,
                attachments: Vec::new(),
            },
            &PlanningKickoffProvider::new(
                &handler.system,
                managed_planning_output(),
                "planning-managed-response",
            ),
        )
        .expect("formalize should run");

    assert_eq!(formalized.response_kind, "system_notice");
    let messages =
        fs::read_to_string(session_dir.join("conversation/messages.json")).expect("messages");
    assert!(messages.contains("已进入 managed path，并创建两个执行任务"));
    assert!(!messages.contains("Framework planning kickoff:"));

    let runtime_design =
        fs::read_to_string(session_dir.join("tasks/registry/task-runtime-design.json"))
            .expect("runtime task should exist");
    assert!(runtime_design.contains("\"status\": \"ready\""));
    let web_timeline =
        fs::read_to_string(session_dir.join("tasks/registry/task-web-debug-timeline.json"))
            .expect("web task should exist");
    assert!(web_timeline.contains("\"status\": \"ready\""));

    let pending = fs::read_to_string(session_dir.join("queue/pending_inputs.json")).expect("queue");
    assert_eq!(pending.trim(), "[]");

    let latest_tick = fs::read_to_string(session_dir.join("control/scheduler/latest_tick.json"))
        .expect("latest tick");
    assert!(latest_tick.contains("\"source\": \"formalize_kickoff\""));
    assert!(latest_tick.contains("\"drove_count\": 1"));

    let events = fs::read_to_string(session_dir.join("events/stream.jsonl")).expect("events");
    assert!(events.contains("session.formalized"));
    assert!(events.contains("framework.task_kickoff_enqueued"));
    assert!(events.contains("scheduler.tick_started"));
    assert!(events.contains("scheduler.tick_completed"));
}

#[test]
fn formalize_auto_kickoff_can_take_direct_path_and_persist_plan_artifact() {
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
    assert!(!tentative.answer.contains("/formalize"));
    assert!(
        tentative
            .routing_action
            .as_ref()
            .and_then(|action| action.prompt_text.as_deref())
            .is_some_and(|text| text.contains("/formalize"))
    );
    let session_id = tentative.binding.session_id.clone().expect("session id");
    let session_dir = kickoff_session_dir(&home, &session_id);

    let formalized = handler
        .send_message_internal_with_provider(
            &home,
            ChatSendRequest {
                message: "/formalize".into(),
                input_kind: None,
                attachments: Vec::new(),
            },
            &PlanningKickoffProvider::new(
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
    assert!(messages.contains("已走 direct path，并写入最小 plan"));
    assert!(!messages.contains("Framework planning kickoff:"));

    let latest_plan =
        fs::read_to_string(session_dir.join("tasks/plan/latest.json")).expect("plan latest");
    assert!(latest_plan.contains("direct path selected after framework planning kickoff"));
    assert!(latest_plan.contains("Inspect current runtime wiring"));
    assert!(latest_plan.contains("Land the bounded change and validate with focused tests"));

    let current_plan = fs::read_to_string(home.join("runtime/current/current_plan_update.json"))
        .expect("current plan");
    assert!(current_plan.contains(&task_id));

    let latest_tick = fs::read_to_string(session_dir.join("control/scheduler/latest_tick.json"))
        .expect("latest tick");
    assert!(latest_tick.contains("\"source\": \"formalize_kickoff\""));
    assert!(latest_tick.contains("\"drove_count\": 1"));

    let events = fs::read_to_string(session_dir.join("events/stream.jsonl")).expect("events");
    assert!(events.contains("framework.task_kickoff_enqueued"));
    assert!(events.contains("plan.updated"));
}
