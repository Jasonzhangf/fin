use crate::{
    ControlFeedbackBuilder, InferenceOperationBuilder, InferenceRequest, M1Runtime,
    ModelOutputParser, WorkerRuntime,
};
use fin_config::{
    ConfigMapper, ProviderCredential, ProviderProtocol, ResolvedProviderConfig, UserConfig,
    UserProviderConfig,
};
use fin_contracts::{
    EntityRefs, InferenceOperationPayload, MinimalContextView, ProviderPath, ProviderStrategy,
    ProviderTarget, RoleProfileRef,
};
use fin_provider::{
    InferenceProvider, PreparedRequest, ProviderDescriptor, ProviderRequest, ProviderResponse,
    ProviderToolCall,
};
use std::collections::BTreeMap;

fn worker_runtime() -> WorkerRuntime {
    let user = UserConfig {
        default_provider: "openai".into(),
        providers: BTreeMap::from([(
            "openai".into(),
            UserProviderConfig {
                protocol: ProviderProtocol::OpenAiCompatible,
                base_url: "https://api.example.com/v1".into(),
                model: "gpt-5".into(),
                api_key: None,
                api_key_env: Some("OPENAI_API_KEY".into()),
                user_agent: None,
                headers: BTreeMap::new(),
            },
        )]),
        runtime: fin_config::UserRuntimeConfig::default(),
    };
    let system = ConfigMapper::map_user_to_system(&user).expect("mapping should succeed");
    WorkerRuntime::from_system(&system, "agent-1", "worker-1", "runtime", None)
        .expect("worker runtime")
}

fn payload_with_task() -> InferenceOperationPayload {
    InferenceOperationPayload {
        input: "please continue the current work".into(),
        role: RoleProfileRef::new("default").expect("role"),
        provider_path: ProviderPath::new(vec![
            ProviderTarget::new("openai", "gpt-5").expect("target"),
        ])
        .expect("path"),
        provider_strategy: ProviderStrategy::Priority,
        protocol_version: "fin.m1".into(),
        stream: false,
        context: MinimalContextView {
            control: Some(fin_contracts::ContextControlBlock {
                task_id: Some("task-1".into()),
                topic_thread_id: Some("topic-1".into()),
                ..Default::default()
            }),
            summary: Some("previous task context".into()),
            ..Default::default()
        },
    }
}

fn prepared_request(payload: &InferenceOperationPayload) -> PreparedRequest {
    PreparedRequest {
        provider_name: "openai".into(),
        protocol: ProviderProtocol::OpenAiCompatible,
        endpoint: "https://api.example.com/v1/chat/completions".into(),
        model: "gpt-5".into(),
        input: payload.input.clone(),
        rendered_input: "compiled".into(),
        user_agent: None,
        sanitized_headers: BTreeMap::new(),
        tools: Vec::new(),
        prior_tool_calls: Vec::new(),
        tool_results: Vec::new(),
    }
}

fn provider_response(output_text: &str, response_id: &str, stop_reason: &str) -> ProviderResponse {
    ProviderResponse {
        provider_name: "openai".into(),
        model: "gpt-5".into(),
        output_text: output_text.into(),
        response_id: Some(response_id.into()),
        stop_reason: Some(stop_reason.into()),
        status: 200,
        tool_calls: Vec::new(),
    }
}

#[test]
fn model_output_parser_extracts_response_and_control_feedback() {
    let payload = payload_with_task();
    let request = prepared_request(&payload);
    let response = provider_response(
        "<fin_user_response>好的，我继续当前任务。</fin_user_response>\n<fin_control_feedback>{\"origin\":\"\",\"is_continuation\":true,\"is_simple_query\":false,\"candidate_task_id\":\"task-1\",\"candidate_topic_thread_id\":\"topic-1\",\"continuity_confidence\":97,\"topic_shift_confidence\":6,\"simple_query_confidence\":4,\"previous_topic_summary\":\"\",\"current_topic_summary\":\"继续当前任务\",\"note_candidate\":\"继续推进当前任务\",\"digest_candidate\":\"任务连续\",\"reason\":\"same task\"}</fin_control_feedback>",
        "resp-1",
        "end_turn",
    );

    let parsed = ModelOutputParser::default().parse(&payload, &request, &response);
    assert_eq!(parsed.user_response, "好的，我继续当前任务。");
    assert!(parsed.contract_detected);
    assert!(!parsed.control_feedback_salvaged);
    assert!(!parsed.tool_calls_block_present);
    assert_eq!(parsed.tool_calls_parse_status, "absent");
    assert!(parsed.tool_calls_invalid_reason.is_none());
    assert!(parsed.tool_calls.is_empty());
    let feedback = parsed.control_feedback.expect("feedback should parse");
    assert_eq!(feedback.origin, "model_output_contract_v1");
    assert_eq!(feedback.candidate_task_id.as_deref(), Some("task-1"));
    assert_eq!(feedback.continuity_confidence, 97);
}

#[test]
fn control_feedback_builder_uses_runtime_observation_when_no_structured_output_exists() {
    let payload = payload_with_task();
    let request = prepared_request(&payload);
    let response = provider_response("plain answer", "resp-2", "end_turn");

    let parsed = ModelOutputParser::default().parse(&payload, &request, &response);
    let runtime_observation = ControlFeedbackBuilder::default().build(&payload, &request, &response);
    let merged = ControlFeedbackBuilder::default().merge_with_runtime_observation(
        parsed.control_feedback.clone(),
        runtime_observation.clone(),
    );
    assert_eq!(parsed.user_response, "plain answer");
    assert!(!parsed.control_feedback_salvaged);
    assert!(!parsed.tool_calls_block_present);
    assert_eq!(parsed.tool_calls_parse_status, "absent");
    assert!(parsed.tool_calls.is_empty());
    assert_eq!(merged, runtime_observation);
}

#[test]
fn model_output_parser_rejects_unrecognized_control_feedback_shape() {
    let payload = payload_with_task();
    let request = prepared_request(&payload);
    let response = provider_response(
        "<fin_user_response>OK</fin_user_response>\n<fin_control_feedback>{\"project_scope\":\"/tmp/fin\",\"next_verify_step\":\"none\"}</fin_control_feedback>",
        "resp-3",
        "end_turn",
    );

    let parsed = ModelOutputParser::default().parse(&payload, &request, &response);
    assert_eq!(parsed.user_response, "OK");
    assert!(parsed.contract_detected);
    assert!(!parsed.control_feedback_salvaged);
    assert!(!parsed.tool_calls_block_present);
    assert_eq!(parsed.tool_calls_parse_status, "absent");
    assert!(parsed.control_feedback.is_none());
}

#[test]
fn model_output_parser_salvages_whitelisted_feedback_fields_with_mask() {
    let payload = payload_with_task();
    let request = prepared_request(&payload);
    let response = provider_response(
        "<fin_user_response>OK</fin_user_response>\n<fin_control_feedback>{\"is_continuation\":true,\"is_simple_query\":\"false\",\"continuity_confidence\":1.0,\"topic_shift_confidence\":\"0.18\",\"simple_query_confidence\":\"24\",\"current_topic_summary\":\"masked summary\",\"note_candidate\":\"masked note\",\"digest_candidate\":\"masked digest\",\"reason\":\"masked reason\",\"project_scope\":\"/tmp/ignored\"}</fin_control_feedback>",
        "resp-4",
        "end_turn",
    );

    let parsed = ModelOutputParser::default().parse(&payload, &request, &response);
    assert_eq!(parsed.user_response, "OK");
    assert!(parsed.contract_detected);
    assert!(parsed.control_feedback_salvaged);
    assert!(!parsed.tool_calls_block_present);
    assert_eq!(parsed.tool_calls_parse_status, "absent");
    assert!(parsed.tool_calls.is_empty());
    let feedback = parsed
        .control_feedback
        .expect("feedback should be salvaged");
    assert_eq!(feedback.origin, "model_output_contract_masked");
    assert!(feedback.is_continuation);
    assert!(!feedback.is_simple_query);
    assert_eq!(feedback.continuity_confidence, 100);
    assert_eq!(feedback.topic_shift_confidence, 18);
    assert_eq!(feedback.simple_query_confidence, 24);
    assert_eq!(
        feedback.current_topic_summary.as_deref(),
        Some("masked summary")
    );
    assert_eq!(feedback.reason, "masked reason");
}

#[test]
fn model_output_parser_extracts_tool_calls_block() {
    let payload = payload_with_task();
    let request = prepared_request(&payload);
    let response = provider_response(
        "<fin_user_response>waiting for logs</fin_user_response>\n<fin_control_feedback>{\"origin\":\"model_output_contract_v1\",\"is_continuation\":true,\"is_simple_query\":false,\"candidate_task_id\":null,\"candidate_topic_thread_id\":null,\"continuity_confidence\":84,\"topic_shift_confidence\":16,\"simple_query_confidence\":9,\"previous_topic_summary\":\"logs\",\"current_topic_summary\":\"logs\",\"note_candidate\":\"need wait\",\"digest_candidate\":\"wait scheduled\",\"reason\":\"need async wait\"}</fin_control_feedback>\n<fin_tool_calls>[{\"tool_name\":\"wait.remind\",\"arguments\":{\"wait_minutes\":5,\"reminder\":\"check ci logs\"}}]</fin_tool_calls>",
        "resp-tool-1",
        "end_turn",
    );

    let parsed = ModelOutputParser::default().parse(&payload, &request, &response);
    assert_eq!(parsed.user_response, "waiting for logs");
    assert!(parsed.contract_detected);
    assert!(parsed.tool_calls_block_present);
    assert_eq!(parsed.tool_calls_parse_status, "exact");
    assert!(parsed.tool_calls_invalid_reason.is_none());
    assert_eq!(parsed.tool_calls.len(), 1);
    assert_eq!(parsed.tool_calls[0].tool_name, "wait.remind");
    assert_eq!(
        parsed.tool_calls[0]
            .arguments
            .get("wait_minutes")
            .and_then(serde_json::Value::as_u64),
        Some(5)
    );
}

#[test]
fn model_output_parser_repairs_missing_user_response_closing_tag() {
    let payload = payload_with_task();
    let request = prepared_request(&payload);
    let response = provider_response(
        "<fin_user_response>先继续当前任务。\n<fin_control_feedback>{\"origin\":\"model_output_contract_v1\",\"is_continuation\":true,\"is_simple_query\":false,\"candidate_task_id\":\"task-1\",\"candidate_topic_thread_id\":\"topic-1\",\"continuity_confidence\":90,\"topic_shift_confidence\":10,\"simple_query_confidence\":4,\"previous_topic_summary\":\"task\",\"current_topic_summary\":\"task\",\"note_candidate\":\"continue\",\"digest_candidate\":\"continue\",\"reason\":\"same task\"}</fin_control_feedback>",
        "resp-repair-user-response",
        "end_turn",
    );

    let parsed = ModelOutputParser::default().parse(&payload, &request, &response);
    assert_eq!(parsed.user_response, "先继续当前任务。");
    assert!(parsed.control_feedback.is_some());
}

#[test]
fn model_output_parser_repairs_deterministic_tool_call_shape() {
    let payload = payload_with_task();
    let request = prepared_request(&payload);
    let response = provider_response(
        "<fin_user_response>done</fin_user_response>\n<fin_control_feedback>{\"origin\":\"model_output_contract_v1\",\"is_continuation\":false,\"is_simple_query\":true,\"candidate_task_id\":null,\"candidate_topic_thread_id\":null,\"continuity_confidence\":20,\"topic_shift_confidence\":80,\"simple_query_confidence\":95,\"previous_topic_summary\":\"short previous topic summary\",\"current_topic_summary\":\"short current topic summary\",\"note_candidate\":\"done\",\"digest_candidate\":\"done\",\"reason\":\"done\"}</fin_control_feedback>\n<fin_tool_calls>\n```json\n{\"name\":\"reasoning.stop\",\"args\":{\"summary\":\"done\"}}\n```",
        "resp-repair-tool",
        "end_turn",
    );

    let parsed = ModelOutputParser::default().parse(&payload, &request, &response);
    assert!(parsed.tool_calls_block_present);
    assert_eq!(parsed.tool_calls_parse_status, "repaired_deterministic");
    assert!(parsed.tool_calls_invalid_reason.is_none());
    assert_eq!(parsed.tool_calls.len(), 1);
    assert_eq!(parsed.tool_calls[0].tool_name, "reasoning.stop");
    assert_eq!(
        parsed.tool_calls[0]
            .arguments
            .get("summary")
            .and_then(serde_json::Value::as_str),
        Some("done")
    );
}

#[test]
fn model_output_parser_does_not_salvage_truncated_tool_call_value() {
    let payload = payload_with_task();
    let request = prepared_request(&payload);
    let response = provider_response(
        "<fin_user_response>done</fin_user_response>\n<fin_control_feedback>{\"origin\":\"model_output_contract_v1\",\"is_continuation\":true,\"is_simple_query\":true,\"candidate_task_id\":null,\"candidate_topic_thread_id\":null,\"continuity_confidence\":95,\"topic_shift_confidence\":85,\"simple_query_confidence\":95,\"previous_topic_summary\":\"prev\",\"current_topic_summary\":\"current\",\"note_candidate\":\"done\",\"digest_candidate\":\"done\",\"reason\":\"done\"}</fin_control_feedback>\n<fin_tool_calls>\n[{\"tool_name\":\"reasoning.stop\",\"arguments\":{\"summary\":\"Timeline validation complete. Max_tokens cutoff confirmed as cause for",
        "resp-invalid-tool",
        "max_tokens",
    );

    let parsed = ModelOutputParser::default().parse(&payload, &request, &response);
    assert!(parsed.tool_calls_block_present);
    assert_eq!(parsed.tool_calls_parse_status, "masked_partial");
    assert_eq!(
        parsed.tool_calls_invalid_reason.as_deref(),
        Some("unterminated_string_value")
    );
    assert!(parsed.tool_calls.is_empty());
}

#[test]
fn model_output_parser_reads_native_tool_calls_from_provider_response() {
    let payload = payload_with_task();
    let request = prepared_request(&payload);
    let response = ProviderResponse {
        tool_calls: vec![ProviderToolCall {
            tool_call_id: "toolu_1".into(),
            name: "exec_command".into(),
            arguments: serde_json::json!({ "cmd": "pwd" }),
        }],
        ..provider_response(
            "继续检查。\n<fin_control_feedback>{\"origin\":\"model_output_contract_v1\",\"is_continuation\":true,\"is_simple_query\":false,\"candidate_task_id\":\"task-1\",\"candidate_topic_thread_id\":\"topic-1\",\"continuity_confidence\":88,\"topic_shift_confidence\":12,\"simple_query_confidence\":6,\"previous_topic_summary\":\"pwd\",\"current_topic_summary\":\"pwd\",\"note_candidate\":\"need pwd\",\"digest_candidate\":\"need pwd\",\"reason\":\"use tool\"}</fin_control_feedback>",
            "resp-native-tool",
            "tool_use",
        )
    };

    let parsed = ModelOutputParser::default().parse(&payload, &request, &response);
    assert_eq!(parsed.user_response, "继续检查。");
    assert_eq!(parsed.tool_calls.len(), 1);
    assert_eq!(
        parsed.tool_calls[0].tool_call_id.as_deref(),
        Some("toolu_1")
    );
    assert_eq!(parsed.tool_calls[0].tool_name, "exec_command");
    assert_eq!(
        parsed.tool_calls[0]
            .arguments
            .get("cmd")
            .and_then(serde_json::Value::as_str),
        Some("pwd")
    );
}

#[path = "model_output_runtime_tests.rs"]
mod model_output_runtime_tests;
