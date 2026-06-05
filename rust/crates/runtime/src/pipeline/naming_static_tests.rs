//! Cross-pipeline naming guards for cleaned modules only.
//!
//! Phase 7a scope: pipeline/ + provider/ + lib.rs mod declarations.
//! Phase 7b scope: extended/v4a forbidden in lib.rs mod declarations.

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

// Phase 7b: extended naming boundary tests
// The `extended` / `v4a` naming family is forbidden in lib.rs mod declarations
// because the per-domain split plan requires consolidating these into single
// `tools::dispatch_X` modules (see docs/architecture/45-runtime-module-inventory.md).
//
// These tests do NOT prevent the underlying files from existing (Phase 5d/5e
// was deferred due to bridge approach failures), but they prevent NEW
// `mod ..._extended_...` declarations from creeping in via lib.rs.

#[test]
fn lib_rs_no_extended_mod_declarations() {
    let lib = read_crate_file("src/lib.rs");
    for bad in [
        "mod tool_dispatch_extended;",
        "mod tool_dispatch_extended_collab;",
        "mod tool_dispatch_extended_collab_coordination;",
        "mod tool_dispatch_extended_collab_mailbox;",
        "mod tool_dispatch_extended_exec;",
        "mod tool_dispatch_extended_exec_receipts;",
        "mod tool_dispatch_extended_patch;",
        "mod tool_dispatch_extended_patch_utils;",
        "mod tool_dispatch_extended_patch_v4a;",
        "mod tool_dispatch_extended_query;",
        "mod tool_dispatch_extended_query_control;",
        "mod tool_dispatch_extended_query_history;",
        "mod tool_dispatch_extended_query_image;",
        "mod tool_dispatch_extended_query_task;",
        "mod tool_dispatch_extended_task_write;",
        "mod tool_dispatch_extended_task_write_claim_guard;",
    ] {
        assert!(
            !lib.contains(bad),
            "forbidden extended mod declaration in lib.rs: {bad}"
        );
    }
}

#[test]
fn lib_rs_no_v4a_mod_declarations() {
    let lib = read_crate_file("src/lib.rs");
    // v4a was a temp version marker; it is forbidden in lib.rs mod declarations.
    for bad in ["_v4a;", "_v4_;"] {
        assert!(
            !lib.contains(bad),
            "forbidden v4a/v4_ naming in lib.rs: {bad}"
        );
    }
}

#[test]
fn lib_rs_no_helpers_or_support_mod_declarations() {
    let lib = read_crate_file("src/lib.rs");
    // `helpers` / `support` are migration signals, not allowed at lib.rs root.
    for bad in [
        "mod activity_cards_helpers;",
        "mod session_materializer_support;",
        "mod tool_dispatch_control_support;",
        "mod tool_dispatch_peer_support;",
    ] {
        assert!(
            !lib.contains(bad),
            "forbidden helpers/support mod declaration in lib.rs: {bad}"
        );
    }
}

#[test]
fn pipeline_pipeline_nodes_never_use_legacy_inline_node_name() {
    // Pipeline node types must follow Domain<Direction><NN><Node> contract.
    // The legacy naming `ErrorReq`, `ReasonReq`, etc. is forbidden.
    for (file, bad) in [
        ("src/pipeline/input_pipeline.rs", "ErrorReq"),
        ("src/pipeline/reason_pipeline.rs", "ErrorReq"),
        ("src/pipeline/feedback_pipeline.rs", "ErrorReq"),
    ] {
        let src = read_crate_file(file);
        assert!(
            !src.contains(bad),
            "{file}: forbidden legacy node name {bad}"
        );
    }
}
