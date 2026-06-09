use super::{
    ProviderCapabilities, ProviderDescriptor, ProviderRequest, ProviderToolCall,
    ProviderToolResult, provider_wire,
};
use fin_config::ProviderProtocol;

#[test]
fn anthropic_messages_keep_unique_tool_use_ids_and_matching_results() {
    let descriptor = ProviderDescriptor {
        name: "minimax".into(),
        protocol: ProviderProtocol::AnthropicWire,
        base_url: "https://api.minimaxi.com/anthropic".into(),
        default_model: "MiniMax-M3".into(),
        capabilities: ProviderCapabilities::for_protocol(ProviderProtocol::AnthropicWire),
    };
    let request = descriptor.prepare_request(&ProviderRequest {
        input: "follow up".into(),
        rendered_input: Some("follow up".into()),
        override_model: None,
        tools: Vec::new(),
        prior_tool_calls: vec![
            ProviderToolCall {
                tool_call_id: "tool-call-00-exec-command".into(),
                name: "exec_command".into(),
                arguments: serde_json::json!({"cmd":"pwd"}),
            },
            ProviderToolCall {
                tool_call_id: "tool-call-01-exec-command".into(),
                name: "exec_command".into(),
                arguments: serde_json::json!({"cmd":"ls"}),
            },
        ],
        tool_results: vec![
            ProviderToolResult {
                tool_call_id: "tool-call-00-exec-command".into(),
                name: "exec_command".into(),
                content: "/tmp/project".into(),
                is_error: false,
            },
            ProviderToolResult {
                tool_call_id: "tool-call-01-exec-command".into(),
                name: "exec_command".into(),
                content: "Cargo.toml".into(),
                is_error: false,
            },
        ],
    });

    let messages = provider_wire::build_anthropic_messages(&request);
    assert_eq!(messages.len(), 3);
    let tool_uses = messages[1]["content"].as_array().expect("tool uses");
    let tool_results = messages[2]["content"].as_array().expect("results");
    assert_eq!(tool_uses[0]["id"], "tool-call-00-exec-command");
    assert_eq!(tool_uses[1]["id"], "tool-call-01-exec-command");
    assert_eq!(tool_results[0]["tool_use_id"], tool_uses[0]["id"]);
    assert_eq!(tool_results[1]["tool_use_id"], tool_uses[1]["id"]);
    assert_ne!(tool_uses[0]["id"], tool_uses[1]["id"]);
}
