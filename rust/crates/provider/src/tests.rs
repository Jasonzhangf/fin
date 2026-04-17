use super::*;

use fin_config::{ProviderCredential, ResolvedProviderConfig};

fn openai_config() -> ResolvedProviderConfig {
    ResolvedProviderConfig {
        name: "openai".into(),
        protocol: ProviderProtocol::OpenAiCompatible,
        base_url: "https://api.example.com/v1".into(),
        model: "gpt-5".into(),
        credential: ProviderCredential::ApiKeyEnv {
            env_var: "OPENAI_API_KEY".into(),
        },
        user_agent: None,
        headers: BTreeMap::new(),
    }
}

#[test]
fn registry_registers_resolved_provider() {
    let mut registry = ProviderRegistry::default();
    registry
        .register_resolved(&openai_config())
        .expect("register should succeed");

    let descriptor = registry.get("openai").expect("provider should exist");
    assert_eq!(descriptor.default_model, "gpt-5");
    assert!(descriptor.capabilities.supports_tool_calls);
}

#[test]
fn duplicate_provider_is_rejected() {
    let mut registry = ProviderRegistry::default();
    let config = openai_config();
    registry
        .register_resolved(&config)
        .expect("first register should succeed");
    let err = registry
        .register_resolved(&config)
        .expect_err("duplicate must fail");
    assert_eq!(
        err,
        ProviderError::DuplicateProvider {
            name: "openai".into()
        }
    );
}

#[test]
fn client_prepares_request_from_descriptor() {
    let descriptor = ProviderDescriptor::from_resolved(&openai_config());
    let client = StaticProviderClient::new(descriptor);
    let prepared = client.prepare_request(&ProviderRequest {
        input: "hello".into(),
        rendered_input: None,
        override_model: None,
    });

    assert_eq!(client.protocol(), ProviderProtocol::OpenAiCompatible);
    assert_eq!(prepared.model, "gpt-5");
    assert_eq!(prepared.provider_name, "openai");
    assert_eq!(
        prepared.endpoint,
        "https://api.example.com/v1/chat/completions"
    );
    assert_eq!(prepared.user_agent, None);
    assert_eq!(prepared.rendered_input, "hello");
    assert!(prepared.sanitized_headers.is_empty());
}

#[test]
fn anthropic_descriptor_prepares_messages_endpoint() {
    let config = ResolvedProviderConfig {
        name: "ali-coding-plan".into(),
        protocol: ProviderProtocol::AnthropicWire,
        base_url: "https://coding.dashscope.aliyuncs.com/apps/anthropic".into(),
        model: "qwen3.6-plus".into(),
        credential: ProviderCredential::ApiKeyEnv {
            env_var: "ALI_CODINGPLAN_KEY".into(),
        },
        user_agent: Some("opencode/1.2.27".into()),
        headers: BTreeMap::from([("X-Trace-Source".into(), "fin-test".into())]),
    };
    let facade = ProviderFacade::from_resolved(&config);
    let prepared = facade.prepare_request(&ProviderRequest {
        input: "hello".into(),
        rendered_input: None,
        override_model: None,
    });
    assert_eq!(
        prepared.endpoint,
        "https://coding.dashscope.aliyuncs.com/apps/anthropic/v1/messages"
    );
    assert_eq!(prepared.user_agent.as_deref(), Some("opencode/1.2.27"));
    assert_eq!(
        prepared
            .sanitized_headers
            .get("X-Trace-Source")
            .map(String::as_str),
        Some("fin-test")
    );
    assert_eq!(
        prepared
            .sanitized_headers
            .get("x-api-key")
            .map(String::as_str),
        Some("<redacted>")
    );
}

#[test]
fn anthropic_headers_preserve_custom_headers_and_override_reserved_ones() {
    let facade = ProviderFacade::from_resolved(&ResolvedProviderConfig {
        name: "ali-coding-plan".into(),
        protocol: ProviderProtocol::AnthropicWire,
        base_url: "https://coding.dashscope.aliyuncs.com/apps/anthropic".into(),
        model: "qwen3.6-plus".into(),
        credential: ProviderCredential::ApiKeyEnv {
            env_var: "ALI_CODINGPLAN_KEY".into(),
        },
        user_agent: Some("opencode/1.2.27".into()),
        headers: BTreeMap::from([
            ("X-Trace-Source".into(), "fin-test".into()),
            ("user-agent".into(), "bad-agent".into()),
            ("x-api-key".into(), "bad-key".into()),
        ]),
    });

    let headers = facade
        .build_anthropic_headers("real-key")
        .expect("headers should build");
    assert_eq!(
        headers
            .get("x-trace-source")
            .expect("custom header")
            .to_str()
            .unwrap(),
        "fin-test"
    );
    assert_eq!(
        headers
            .get("user-agent")
            .expect("user agent")
            .to_str()
            .unwrap(),
        "opencode/1.2.27"
    );
    assert_eq!(
        headers.get("x-api-key").expect("api key").to_str().unwrap(),
        "real-key"
    );
    assert_eq!(
        headers
            .get("anthropic-version")
            .expect("anthropic version")
            .to_str()
            .unwrap(),
        "2023-06-01"
    );
}
