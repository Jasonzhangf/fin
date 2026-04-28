use super::*;
use fin_provider::{PreparedRequest, ProviderDescriptor, ProviderRequest, ProviderResponse};
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone)]
struct SystemInspectionProvider {
    descriptor: ProviderDescriptor,
    requests: Arc<Mutex<Vec<PreparedRequest>>>,
}

impl SystemInspectionProvider {
    fn new(system: &SystemConfig) -> Self {
        Self {
            descriptor: ProviderDescriptor::from_resolved(
                system.default_provider_config().expect("default provider"),
            ),
            requests: Arc::new(Mutex::new(Vec::new())),
        }
    }

    fn captured_requests(&self) -> Vec<PreparedRequest> {
        self.requests.lock().expect("requests lock").clone()
    }
}

impl fin_provider::InferenceProvider for SystemInspectionProvider {
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
        self.requests
            .lock()
            .expect("requests lock")
            .push(request.clone());
        if request.prior_tool_calls.is_empty() {
            return Ok(ProviderResponse {
                provider_name: request.provider_name.clone(),
                model: request.model.clone(),
                output_text: "<fin_user_response>已收到，先检查当前 agent 与 supervision 真相。</fin_user_response>"
                    .into(),
                response_id: Some("system-inspection-round-1".into()),
                stop_reason: Some("tool_use".into()),
                status: 200,
                tool_calls: vec![
                    fin_provider::ProviderToolCall {
                        tool_call_id: "toolu_presence".into(),
                        name: "agent.presence.list".into(),
                        arguments: serde_json::json!({"limit": 10}),
                    },
                    fin_provider::ProviderToolCall {
                        tool_call_id: "toolu_supervision".into(),
                        name: "project.supervision.list".into(),
                        arguments: serde_json::json!({"limit": 10}),
                    },
                ],
            });
        }

        Ok(ProviderResponse {
            provider_name: request.provider_name.clone(),
            model: request.model.clone(),
            output_text: "<fin_user_response>已完成 system 检查，当前可继续推进，无需用户额外确认。</fin_user_response>\n<fin_control_feedback>{\"origin\":\"model_output_contract_v1\",\"is_continuation\":true,\"is_simple_query\":false,\"task_completed\":true,\"is_simple_chat\":false,\"blocked\":false,\"needs_user_involve\":false,\"candidate_task_id\":null,\"candidate_topic_thread_id\":null,\"continuity_confidence\":94,\"topic_shift_confidence\":6,\"simple_query_confidence\":4,\"previous_topic_summary\":\"system inspection\",\"current_topic_summary\":\"system inspection\",\"completion_evidence\":[\"agent.presence.list receipt inspected\",\"project.supervision.list receipt inspected\"],\"final_conclusions\":[\"system inspection closure completed\",\"business loop may continue without prompting the user\"],\"blocked_reason\":null,\"what_needs_to_be_done_by_user\":null,\"note_candidate\":\"system inspection completed\",\"digest_candidate\":\"system inspection completed\",\"reason\":\"tool receipts were consumed and the same turn continued without framework prompt leakage\"}</fin_control_feedback>\n<fin_tool_calls>[{\"tool_name\":\"reasoning.stop\",\"arguments\":{\"summary\":\"system inspection complete\"}}]</fin_tool_calls>".into(),
            response_id: Some("system-inspection-round-2".into()),
            stop_reason: Some("end_turn".into()),
            status: 200,
            tool_calls: Vec::new(),
        })
    }
}

#[test]
fn system_tool_round_consumes_presence_and_supervision_truth_without_prompting_user() {
    let home = temp_runtime_home();
    ensure_runtime_home_layout(&home).expect("runtime home should init");
    let system = map_system_config(&sample_user_toml()).expect("system config");
    let handler =
        CliDebugActionHandler::new(sample_user_toml(), system).expect("handler should build");
    let provider = SystemInspectionProvider::new(&handler.system);

    write_file(
        &home.join("runtime/current/current_agent_presence_registry.json"),
        br#"{
  "agents":[
    {"agent_id":"mbp.system","device_name":"mbp","agent_name":"system","status":"busy"},
    {"agent_id":"mbp.fin","device_name":"mbp","agent_name":"fin","status":"idle"},
    {"agent_id":"mbp.infra","device_name":"mbp","agent_name":"infra","status":"waiting"}
  ]
}"#,
    )
    .expect("presence registry");
    write_file(
        &home.join("runtime/current/current_project_supervision.json"),
        br#"{
  "ready_count":1,
  "resume_ready_count":1,
  "busy_count":1,
  "waiting_count":1,
  "recover_needed_count":0,
  "projects":[
    {"project_id":"fin","desired_action":"running_observed"},
    {"project_id":"infra","desired_action":"resume_candidate_present"}
  ]
}"#,
    )
    .expect("project supervision");

    let response = handler
        .send_message_internal_with_provider(
            &home,
            ChatSendRequest {
                message: "检查当前 agent 与 supervision 状态，然后继续处理。".into(),
                input_kind: None,
                attachments: Vec::new(),
            },
            &provider,
        )
        .expect("system inspection turn should run");

    assert_eq!(response.response_kind, "assistant_message");
    assert!(response.answer.contains("已完成 system 检查"));
    assert!(response.answer.contains("无需用户额外确认"));
    assert!(
        response
            .routing_action
            .as_ref()
            .is_none_or(|action| !action.prompt_user)
    );

    let session_id = response.binding.session_id.expect("session id");
    let session_dir = crate::session_binding::find_session_dir(&home, &session_id)
        .map(|(_, _, dir)| dir)
        .expect("session dir should exist");
    let messages =
        fs::read_to_string(session_dir.join("conversation/messages.json")).expect("messages");
    assert!(messages.contains("已完成 system 检查"));
    assert!(!messages.contains("/formalize"));
    assert!(!messages.contains("请选择"));
    assert!(!messages.contains("agent.presence.list"));
    assert!(!messages.contains("project.supervision.list"));

    let tools =
        fs::read_to_string(session_dir.join("tools/recent_tool_records.json")).expect("tools");
    assert!(tools.contains("\"tool_name\": \"agent.presence.list\""));
    assert!(tools.contains("\"tool_name\": \"project.supervision.list\""));

    let events = fs::read_to_string(session_dir.join("events/stream.jsonl")).expect("events");
    assert!(events.contains("agent.presence_list_completed"));
    assert!(events.contains("project.supervision_list_completed"));
    assert!(!events.contains("routing_prompt_user"));

    let requests = provider.captured_requests();
    assert_eq!(requests.len(), 2);
    assert!(
        requests[1]
            .prior_tool_calls
            .iter()
            .any(|call| call.name == "agent.presence.list")
    );
    assert!(
        requests[1]
            .prior_tool_calls
            .iter()
            .any(|call| call.name == "project.supervision.list")
    );
    assert!(
        requests[1]
            .tool_results
            .iter()
            .any(|result| result.name == "agent.presence.list")
    );
    assert!(
        requests[1]
            .tool_results
            .iter()
            .any(|result| result.name == "project.supervision.list")
    );
    assert!(requests[1].input.contains("Continue the same turn."));
}
