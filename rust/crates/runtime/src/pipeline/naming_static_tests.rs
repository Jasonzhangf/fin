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
fn lib_rs_extended_mod_declarations_count_is_documented() {
    // Phase 5d completed: all `mod tool_dispatch_extended_*` declarations moved
    // out of lib.rs into tools/mod.rs.
    let lib = read_crate_file("src/lib.rs");
    let lib_lines: std::collections::HashSet<&str> = lib
        .lines()
        .filter(|l| l.trim().starts_with("mod tool_dispatch_extended"))
        .collect();
    assert_eq!(
        lib_lines.len(),
        0,
        "lib.rs must not declare tool_dispatch_extended_* modules; move to tools/mod.rs"
    );
    let tools_mod = read_crate_file("src/tools/mod.rs");
    let tools_lines: std::collections::HashSet<&str> = tools_mod
        .lines()
        .filter(|l| l.trim().starts_with("pub(crate) mod tool_dispatch_extended")
                || l.trim().starts_with("mod tool_dispatch_extended"))
        .collect();
    assert!(
        !tools_lines.is_empty(),
        "expected at least one tool_dispatch_extended_* submodule in tools/mod.rs"
    );
}

#[test]
fn tools_mod_v4a_mod_declarations_count_is_documented() {
    // v4a is a temp version marker. Target state: zero occurrences.
    // Current state: 1 (tool_dispatch_extended_patch_v4a) in tools/mod.rs.
    let tools_mod = read_crate_file("src/tools/mod.rs");
    let count = tools_mod.matches("_v4a").count();
    assert_eq!(
        count, 1,
        "expected 1 _v4a declaration (Phase 5d backlog), found {}",
        count
    );
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

// === Phase 5d+ gate: subdomain presence + cross-domain boundary lock ===

#[test]
fn lib_rs_domain_dirs_have_mod_entries() {
    // After Phase 5 splits, these subdirectories must exist and have mod.rs.
    let lib = read_crate_file("src/lib.rs");
    for domain in [
        "pipeline", "closure", "context", "tools",
        "session", "control", "task", "agent", "runtime_home",
        "model", "prompt",
    ] {
        assert!(
            lib.contains(&format!("mod {domain};")),
            "lib.rs must declare `mod {domain};` for domain split"
        );
    }
}

#[test]
fn domain_mods_do_not_use_legacy_crate_paths() {
    // After domain split, subdomain modules should use crate::<domain>::<module>
    // not the old crate::<file_name>::<item> paths.
    // Check that lib.rs no longer declares old flat names.
    let lib = read_crate_file("src/lib.rs");
    for bad_mod in [
        "mod prompt_assembly;",
        "mod model_output;",
        "mod model_input_assembler;",
        "mod skill_loader;",
        "mod source_visibility;",
        "mod agent_naming;",
        "mod assignment_queue;",
        "mod managed_task_board;",
        "mod task_store;",
        "mod task_handoff;",
        "mod task_board_snapshot;",
    ] {
        assert!(
            !lib.contains(bad_mod),
            "lib.rs must not declare {bad_mod} — move into domain subdir"
        );
    }
}

#[test]
fn domain_dirs_have_no_fallback_or_salvage() {
    // Verify each domain mod.rs does not import or contain fallback patterns.
    use std::path::PathBuf;
    for domain in [
        "pipeline", "closure", "context", "tools",
        "session", "control", "task", "agent", "runtime_home",
        "model", "prompt",
    ] {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join(format!("src/{domain}/mod.rs"));
        if path.exists() {
            let content = std::fs::read_to_string(&path)
                .unwrap_or_else(|e| panic!("read {domain}/mod.rs: {e}"));
            assert!(
                !content.contains("fallback"),
                "{domain}/mod.rs must not reference fallback"
            );
        }
    }
}

#[test]
fn cross_domain_no_direct_crate_file_imports() {
    // After split, no subdomain module should do `use crate::<module_name>::` where
    // <module_name> is a root-level file that has been moved into a domain dir.
    // Check known cross-domain files that are now subdomain-owned.
    use std::path::PathBuf;
    let migrated = [
        ("src/agent/naming.rs", "agent_naming"),
        ("src/session/journal.rs", "session_record_journal"),
        ("src/task/store.rs", "task_store"),
        ("src/task/handoff.rs", "task_handoff"),
        ("src/control/feedback.rs", "control_feedback"),
    ];
    for (file, old_name) in &migrated {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(file);
        if !path.exists() { continue; }
        let content = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("read {file}: {e}"));
        assert!(
            !content.contains(&format!("use crate::{old_name}")),
            "{file}: must not use legacy crate::{old_name} — use crate::<domain>::<module>"
        );
    }
}
