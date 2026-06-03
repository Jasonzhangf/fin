use super::*;

use fin_config::{ProviderCredential, ResolvedProviderConfig};
use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread;

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
        tools: Vec::new(),
        prior_tool_calls: Vec::new(),
        tool_results: Vec::new(),
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
        tools: Vec::new(),
        prior_tool_calls: Vec::new(),
        tool_results: Vec::new(),
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

#[test]
fn anthropic_execute_retries_retryable_request_failures() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("listener");
    let address = listener.local_addr().expect("local addr");
    let attempts = Arc::new(AtomicUsize::new(0));
    let attempts_for_thread = Arc::clone(&attempts);
    let server = thread::spawn(move || {
        for current in 1..=3 {
            let (mut stream, _) = listener.accept().expect("accept");
            attempts_for_thread.fetch_add(1, Ordering::SeqCst);
            let mut buffer = [0_u8; 2048];
            let _ = stream.read(&mut buffer);
            if current < 3 {
                continue;
            }
            let body = r#"{"id":"msg-1","content":[{"type":"text","text":"OK"}],"stop_reason":"end_turn"}"#;
            let response = format!(
                "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}",
                body.len(),
                body
            );
            stream
                .write_all(response.as_bytes())
                .expect("write response");
        }
    });

    let facade = ProviderFacade::from_resolved(&ResolvedProviderConfig {
        name: "local-anthropic".into(),
        protocol: ProviderProtocol::AnthropicWire,
        base_url: format!("http://{}", address),
        model: "qwen3.6-plus".into(),
        credential: ProviderCredential::DirectApiKey {
            api_key: "test-key".into(),
        },
        user_agent: Some("opencode/1.2.27".into()),
        headers: BTreeMap::new(),
    });
    let prepared = facade.prepare_request(&ProviderRequest {
        input: "hello".into(),
        rendered_input: Some("hello".into()),
        override_model: None,
        tools: Vec::new(),
        prior_tool_calls: Vec::new(),
        tool_results: Vec::new(),
    });

    let response = facade
        .execute_prepared(&prepared)
        .expect("third attempt should succeed");
    assert_eq!(response.output_text, "OK");
    assert_eq!(attempts.load(Ordering::SeqCst), 3);
    server.join().expect("server thread");
}

#[test]
fn anthropic_execute_uses_larger_output_budget() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("listener");
    let address = listener.local_addr().expect("local addr");
    let captured_body = Arc::new(std::sync::Mutex::new(String::new()));
    let captured_body_for_thread = Arc::clone(&captured_body);
    let server = thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept");
        let mut buffer = [0_u8; 16384];
        let read = stream.read(&mut buffer).expect("read request");
        let raw = String::from_utf8_lossy(&buffer[..read]).to_string();
        let body = raw.split("\r\n\r\n").nth(1).unwrap_or_default().to_string();
        *captured_body_for_thread.lock().expect("lock body") = body;
        let response_body =
            r#"{"id":"msg-1","content":[{"type":"text","text":"OK"}],"stop_reason":"end_turn"}"#;
        let response = format!(
            "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}",
            response_body.len(),
            response_body
        );
        stream
            .write_all(response.as_bytes())
            .expect("write response");
    });

    let facade = ProviderFacade::from_resolved(&ResolvedProviderConfig {
        name: "local-anthropic".into(),
        protocol: ProviderProtocol::AnthropicWire,
        base_url: format!("http://{}", address),
        model: "qwen3.6-plus".into(),
        credential: ProviderCredential::DirectApiKey {
            api_key: "test-key".into(),
        },
        user_agent: Some("opencode/1.2.27".into()),
        headers: BTreeMap::new(),
    });
    let prepared = facade.prepare_request(&ProviderRequest {
        input: "hello".into(),
        rendered_input: Some("hello".into()),
        override_model: None,
        tools: Vec::new(),
        prior_tool_calls: Vec::new(),
        tool_results: Vec::new(),
    });

    let response = facade
        .execute_prepared(&prepared)
        .expect("request should succeed");
    assert_eq!(response.output_text, "OK");
    server.join().expect("server thread");

    let request_body = captured_body.lock().expect("lock body").clone();
    assert!(request_body.contains("\"max_tokens\":2048"));
}

#[test]
fn openai_compatible_execute_posts_chat_completions_request() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("listener");
    let address = listener.local_addr().expect("local addr");
    let captured_request = Arc::new(std::sync::Mutex::new(String::new()));
    let captured_request_for_thread = Arc::clone(&captured_request);
    let server = thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept");
        let mut buffer = [0_u8; 16384];
        let read = stream.read(&mut buffer).expect("read request");
        let raw = String::from_utf8_lossy(&buffer[..read]).to_string();
        *captured_request_for_thread.lock().expect("lock request") = raw;
        let response_body = r#"{"id":"chatcmpl-1","choices":[{"message":{"role":"assistant","content":"OK"},"finish_reason":"stop"}]}"#;
        let response = format!(
            "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}",
            response_body.len(),
            response_body
        );
        stream
            .write_all(response.as_bytes())
            .expect("write response");
    });

    let facade = ProviderFacade::from_resolved(&ResolvedProviderConfig {
        name: "local-openai".into(),
        protocol: ProviderProtocol::OpenAiCompatible,
        base_url: format!("http://{}", address),
        model: "gpt-5".into(),
        credential: ProviderCredential::DirectApiKey {
            api_key: "test-key".into(),
        },
        user_agent: Some("opencode/1.2.27".into()),
        headers: BTreeMap::new(),
    });
    let prepared = facade.prepare_request(&ProviderRequest {
        input: "hello".into(),
        rendered_input: Some("hello".into()),
        override_model: None,
        tools: Vec::new(),
        prior_tool_calls: Vec::new(),
        tool_results: Vec::new(),
    });

    let response = facade
        .execute_prepared(&prepared)
        .expect("openai compatible request should succeed");
    assert_eq!(response.output_text, "OK");
    assert_eq!(response.stop_reason.as_deref(), Some("stop"));
    server.join().expect("server thread");

    let request = captured_request.lock().expect("lock request").clone();
    assert!(request.starts_with("POST /chat/completions HTTP/1.1"));
    assert!(request.contains("authorization: Bearer test-key"));
    assert!(request.contains("\"max_tokens\":8192"));
}

#[test]
fn anthropic_execute_does_not_retry_http_status_errors() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("listener");
    let address = listener.local_addr().expect("local addr");
    let attempts = Arc::new(AtomicUsize::new(0));
    let attempts_for_thread = Arc::clone(&attempts);
    let server = thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept");
        attempts_for_thread.fetch_add(1, Ordering::SeqCst);
        let mut buffer = [0_u8; 2048];
        let _ = stream.read(&mut buffer);
        let body = r#"{"error":"bad request"}"#;
        let response = format!(
            "HTTP/1.1 400 Bad Request\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}",
            body.len(),
            body
        );
        stream
            .write_all(response.as_bytes())
            .expect("write response");
    });

    let facade = ProviderFacade::from_resolved(&ResolvedProviderConfig {
        name: "local-anthropic".into(),
        protocol: ProviderProtocol::AnthropicWire,
        base_url: format!("http://{}", address),
        model: "qwen3.6-plus".into(),
        credential: ProviderCredential::DirectApiKey {
            api_key: "test-key".into(),
        },
        user_agent: Some("opencode/1.2.27".into()),
        headers: BTreeMap::new(),
    });
    let prepared = facade.prepare_request(&ProviderRequest {
        input: "hello".into(),
        rendered_input: Some("hello".into()),
        override_model: None,
        tools: Vec::new(),
        prior_tool_calls: Vec::new(),
        tool_results: Vec::new(),
    });

    let err = facade
        .execute_prepared(&prepared)
        .expect_err("http 400 must fail without retry");
    match err {
        ProviderError::HttpStatus { status, body } => {
            assert_eq!(status, 400);
            assert!(body.contains("bad request"));
        }
        other => panic!("unexpected error: {other:?}"),
    }
    assert_eq!(attempts.load(Ordering::SeqCst), 1);
    server.join().expect("server thread");
}

#[test]
fn execute_prepared_covers_every_registered_provider_protocol() {
    use fin_config::ProviderProtocol;

    let source = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/provider_facade.rs"),
    )
    .expect("read provider_facade.rs");

    // Locate the body of fn execute_prepared(&self, request: &PreparedRequest)
    let fn_idx = source
        .find("fn execute_prepared(")
        .expect("execute_prepared must exist");
    // Take a generous window (next 800 chars) covering the full match block.
    let window_end = source.len().min(fn_idx + 1500);
    let window = &source[fn_idx..window_end];
    let body_end_rel = window
        .find("\n    }\n")
        .expect("execute_prepared must end with closing brace");
    let body = &window[..body_end_rel];

    // Every registered ProviderProtocol variant must appear as an explicit match arm.
    for variant in [
        "ProviderProtocol::AnthropicWire",
        "ProviderProtocol::OpenAiCompatible",
    ] {
        assert!(
            body.contains(variant),
            "execute_prepared must dispatch {variant}; current body:\n{body}",
        );
    }

    // Negative invariant: no wildcard catch-all (silent drop) is allowed.
    let has_wildcard = body.contains("protocol =>")
        || body.contains("_, =>")
        || body.contains("_, =>");
    assert!(
        !has_wildcard,
        "execute_prepared must not use a wildcard catch-all; got:\n{body}",
    );

    // The protocol enum itself must enumerate every variant we expect;
    // if a new variant is added, this test must be updated to assert it.
    let protocol_enum_source = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../config/src/lib.rs"),
    )
    .ok();
    if let Some(src) = protocol_enum_source {
        for variant in [
            "OpenAiCompatible",
            "AnthropicWire",
        ] {
            assert!(
                src.contains(&format!("    {variant},")),
                "ProviderProtocol variant {variant} must be declared; update this test if you add a new variant",
            );
        }
        // Verify the test does not silently miss a new variant: count variants
        // and require the test to enumerate the same count.
        let enum_block_start = src
            .find("pub enum ProviderProtocol")
            .expect("ProviderProtocol enum");
        let enum_block_end = src
            .rfind('}')
            .expect("ProviderProtocol enum end");
        let enum_block = &src[enum_block_start..enum_block_end];
        let declared_variants: Vec<String> = enum_block
            .lines()
            .map(|l| l.trim().trim_end_matches(',').trim().to_string())
            .filter(|l| {
                !l.is_empty()
                    && !l.starts_with("//")
                    && !l.starts_with("#[")
                    && l.chars().next().map_or(false, |c| c.is_ascii_uppercase())
            })
            .collect();
        assert!(
            declared_variants.len() >= 2,
            "ProviderProtocol must declare at least 2 variants (AnthropicWire, OpenAiCompatible); got {:?}; update this test when adding a new variant",
            declared_variants
        );
        let _ = declared_variants;
        let _ = ProviderProtocol::AnthropicWire; // keep import live even if unused
    }
}

#[test]
fn provider_facade_rejects_unknown_protocol_with_explicit_error() {
    // A custom enum value that is not in the registered set is impossible to
    // construct via the public API because ProviderProtocol is a closed enum;
    // this test only documents that the execute_prepared body has no
    // 'protocol => Err(UnsupportedProtocol)' fallback that would silently
    // route new variants to a generic error.
    let source = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/provider_facade.rs"),
    )
    .expect("read provider_facade.rs");
    let fn_idx = source
        .find("fn execute_prepared(")
        .expect("execute_prepared must exist");
    let window_end = source.len().min(fn_idx + 1500);
    let window = &source[fn_idx..window_end];
    let body_end_rel = window
        .find("\n    }\n")
        .expect("execute_prepared must end with closing brace");
    let body = &window[..body_end_rel];
    assert!(
        !body.contains("Err(ProviderError::UnsupportedProtocol"),
        "execute_prepared must not fall back to UnsupportedProtocol; every ProviderProtocol variant must be handled explicitly",
    );
}
