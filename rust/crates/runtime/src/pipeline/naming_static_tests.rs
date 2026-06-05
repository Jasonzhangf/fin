//! Cross-pipeline naming guards for cleaned modules only.
//!
//! Phase 7a scope: pipeline/ + provider/ + lib.rs mod declarations.
//! Phase 7b scope (after Phase 5c-5e): expand to full runtime/src/.

use std::fs;

fn read_crate_file(relative: &str) -> String {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(relative);
    fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {relative}: {e}"))
}

#[test]
fn error_pipeline_no_swallow_no_fallback() {
    let src = read_crate_file("src/pipeline/error_pipeline.rs");
    assert!(
        !src.contains("treat_invalid_as_success") && !src.contains("fallback_ok"),
        "error chain must not silently swallow or fall back to success"
    );
}

#[test]
fn error_pipeline_no_forbidden_numbering() {
    let src = read_crate_file("src/pipeline/error_pipeline.rs");
    for bad in ["ErrorErr03a", "ErrorErr04_", "ErrorErr04.", "ErrorErrV2"] {
        assert!(!src.contains(bad), "forbidden numbering: {bad}");
    }
}

#[test]
fn feedback_pipeline_no_from_no_fallback() {
    let src = read_crate_file("src/pipeline/feedback_pipeline.rs");
    assert!(!src.contains("impl From<"), "feedback chain: no From conversions");
    for bad in ["FeedbackResp03a", "FeedbackResp04_", "FeedbackRespV2"] {
        assert!(!src.contains(bad), "forbidden numbering: {bad}");
    }
    assert!(
        !src.contains("fallback_ok") && !src.contains("treat_invalid_as_success"),
        "feedback chain must not silently salvage"
    );
}

#[test]
fn pipeline_modules_use_crate_path_not_legacy() {
    let lib = read_crate_file("src/lib.rs");
    assert!(
        !lib.contains("mod hub_pipeline"),
        "hub_pipeline must be deleted"
    );
    for bad in [
        "mod error_pipeline;",
        "mod input_pipeline;",
        "mod reason_pipeline;",
        "mod feedback_pipeline;",
    ] {
        assert!(!lib.contains(bad), "must use pipeline/ subdomain: {bad}");
    }
}

#[test]
fn provider_hub_pipeline_fully_deleted() {
    let lib = read_crate_file("../../crates/provider/src/lib.rs");
    assert!(
        !lib.contains("hub_pipeline"),
        "hub_pipeline must be deleted from provider"
    );
}
