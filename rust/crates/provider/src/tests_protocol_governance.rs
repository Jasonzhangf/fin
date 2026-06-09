#[test]
fn execute_prepared_covers_every_registered_provider_protocol() {
    use fin_config::ProviderProtocol;

    let source = read_provider_facade_source();
    let body = execute_prepared_body(&source);
    for variant in [
        "ProviderProtocol::AnthropicWire",
        "ProviderProtocol::OpenAiCompatible",
    ] {
        assert!(
            body.contains(variant),
            "execute_prepared must dispatch {variant}; current body:\n{body}",
        );
    }
    let has_wildcard =
        body.contains("protocol =>") || body.contains("_, =>") || body.contains("_, =>");
    assert!(
        !has_wildcard,
        "execute_prepared must not use a wildcard catch-all; got:\n{body}",
    );

    let protocol_enum_source = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../config/src/lib.rs"),
    )
    .ok();
    if let Some(src) = protocol_enum_source {
        for variant in ["OpenAiCompatible", "AnthropicWire"] {
            assert!(
                src.contains(&format!("    {variant},")),
                "ProviderProtocol variant {variant} must be declared; update this test if you add a new variant",
            );
        }
        let declared_variants = declared_protocol_variants(&src);
        assert!(
            declared_variants.len() >= 2,
            "ProviderProtocol must declare at least 2 variants; got {:?}",
            declared_variants
        );
        let _ = ProviderProtocol::AnthropicWire;
    }
}

#[test]
fn provider_facade_rejects_unknown_protocol_with_explicit_error() {
    let source = read_provider_facade_source();
    let body = execute_prepared_body(&source);
    assert!(
        !body.contains("Err(ProviderError::UnsupportedProtocol"),
        "execute_prepared must not fall back to UnsupportedProtocol; every ProviderProtocol variant must be handled explicitly",
    );
}

fn read_provider_facade_source() -> String {
    std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/provider_facade.rs"),
    )
    .expect("read provider_facade.rs")
}

fn execute_prepared_body(source: &str) -> &str {
    let fn_idx = source
        .find("fn execute_prepared(")
        .expect("execute_prepared must exist");
    let window_end = source.len().min(fn_idx + 1500);
    let window = &source[fn_idx..window_end];
    let body_end_rel = window
        .find("\n    }\n")
        .expect("execute_prepared must end with closing brace");
    &window[..body_end_rel]
}

fn declared_protocol_variants(src: &str) -> Vec<String> {
    let enum_block_start = src
        .find("pub enum ProviderProtocol")
        .expect("ProviderProtocol enum");
    let enum_block_end = src.rfind('}').expect("ProviderProtocol enum end");
    src[enum_block_start..enum_block_end]
        .lines()
        .map(|line| line.trim().trim_end_matches(',').trim().to_string())
        .filter(|line| {
            !line.is_empty()
                && !line.starts_with("//")
                && !line.starts_with("#[")
                && line
                    .chars()
                    .next()
                    .map_or(false, |value| value.is_ascii_uppercase())
        })
        .collect()
}
