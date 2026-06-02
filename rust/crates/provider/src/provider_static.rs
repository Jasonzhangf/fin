use super::*;

#[derive(Debug, Clone)]
pub struct StaticProviderClient {
    descriptor: ProviderDescriptor,
}

impl StaticProviderClient {
    pub fn new(descriptor: ProviderDescriptor) -> Self {
        Self { descriptor }
    }
}

impl InferenceProvider for StaticProviderClient {
    fn descriptor(&self) -> &ProviderDescriptor {
        &self.descriptor
    }

    fn execute_prepared(
        &self,
        request: &PreparedRequest,
    ) -> Result<ProviderResponse, ProviderError> {
        let user_response = format!("simulated response for {}", request.input);
        let escaped_response = serde_json::to_string(&user_response).map_err(|error| {
            ProviderError::ParseResponse {
                message: format!("failed to encode static response: {error}"),
            }
        })?;
        let escaped_current_topic =
            serde_json::to_string(request.input.trim()).map_err(|error| {
                ProviderError::ParseResponse {
                    message: format!("failed to encode static topic summary: {error}"),
                }
            })?;
        Ok(ProviderResponse {
            provider_name: request.provider_name.clone(),
            model: request.model.clone(),
            output_text: format!(
                "<fin_user_response>{user_response}</fin_user_response>\n\
<fin_control_feedback>{{\"origin\":\"model_output_contract_v1\",\"is_continuation\":false,\"is_simple_query\":true,\"task_completed\":false,\"is_simple_chat\":true,\"blocked\":false,\"needs_user_involve\":false,\"candidate_task_id\":null,\"candidate_topic_thread_id\":null,\"continuity_confidence\":72,\"topic_shift_confidence\":28,\"simple_query_confidence\":88,\"previous_topic_summary\":\"static provider\",\"current_topic_summary\":{escaped_current_topic},\"completion_evidence\":[],\"final_conclusions\":[],\"blocked_reason\":null,\"what_needs_to_be_done_by_user\":null,\"note_candidate\":{escaped_response},\"digest_candidate\":{escaped_response},\"reason\":\"structured static provider\"}}</fin_control_feedback>\n\
<fin_tool_calls>[{{\"tool_name\":\"reasoning.stop\",\"arguments\":{{\"summary\":\"static provider complete\"}}}}]</fin_tool_calls>"
            ),
            response_id: Some("simulated-response".into()),
            stop_reason: Some("end_turn".into()),
            status: 200,
            tool_calls: Vec::new(),
        })
    }
}

#[derive(Debug, Clone)]
pub struct StructuredStaticProviderClient {
    descriptor: ProviderDescriptor,
}

impl StructuredStaticProviderClient {
    pub fn new(descriptor: ProviderDescriptor) -> Self {
        Self { descriptor }
    }
}

impl InferenceProvider for StructuredStaticProviderClient {
    fn descriptor(&self) -> &ProviderDescriptor {
        &self.descriptor
    }

    fn execute_prepared(
        &self,
        request: &PreparedRequest,
    ) -> Result<ProviderResponse, ProviderError> {
        let user_response = format!("simulated response for {}", request.input);
        let escaped_response = serde_json::to_string(&user_response).map_err(|error| {
            ProviderError::ParseResponse {
                message: format!("failed to encode structured static response: {error}"),
            }
        })?;
        let trimmed_input = request.input.trim();
        let word_count = trimmed_input.split_whitespace().count();
        let punctuation_count = trimmed_input
            .chars()
            .filter(|ch| matches!(ch, '?' | '？' | '!' | '！'))
            .count();
        let is_simple_query = word_count <= 12 && punctuation_count <= 1;
        let continuity_confidence = if is_simple_query { 72 } else { 58 };
        let topic_shift_confidence = if is_simple_query { 28 } else { 36 };
        let simple_query_confidence = if is_simple_query { 88 } else { 24 };
        let escaped_current_topic =
            serde_json::to_string(trimmed_input).map_err(|error| ProviderError::ParseResponse {
                message: format!("failed to encode current topic summary: {error}"),
            })?;
        Ok(ProviderResponse {
            provider_name: request.provider_name.clone(),
            model: request.model.clone(),
            output_text: format!(
                "<fin_user_response>{user_response}</fin_user_response>\n\
<fin_control_feedback>{{\"origin\":\"model_output_contract_v1\",\"is_continuation\":false,\"is_simple_query\":{is_simple_query},\"task_completed\":false,\"is_simple_chat\":true,\"blocked\":false,\"needs_user_involve\":false,\"candidate_task_id\":null,\"candidate_topic_thread_id\":null,\"continuity_confidence\":{continuity_confidence},\"topic_shift_confidence\":{topic_shift_confidence},\"simple_query_confidence\":{simple_query_confidence},\"previous_topic_summary\":\"static provider\",\"current_topic_summary\":{escaped_current_topic},\"completion_evidence\":[],\"final_conclusions\":[],\"blocked_reason\":null,\"what_needs_to_be_done_by_user\":null,\"note_candidate\":{escaped_response},\"digest_candidate\":{escaped_response},\"reason\":\"structured static provider\"}}</fin_control_feedback>\n\
<fin_tool_calls>[{{\"tool_name\":\"reasoning.stop\",\"arguments\":{{\"summary\":\"structured static complete\"}}}}]</fin_tool_calls>"
            ),
            response_id: Some("structured-static-response".into()),
            stop_reason: Some("end_turn".into()),
            status: 200,
            tool_calls: vec![ProviderToolCall {
                tool_call_id: "tool-use-static-reasoning-stop".into(),
                name: "reasoning.stop".into(),
                arguments: serde_json::json!({ "summary": user_response }),
            }],
        })
    }
}
