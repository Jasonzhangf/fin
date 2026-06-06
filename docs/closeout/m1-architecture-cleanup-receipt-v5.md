# Architecture Cleanup Receipt (2026-06-06 v5 — final)

## Status

| Phase | Status | Evidence |
|-------|--------|----------|
| 1. Docs truth frozen | DONE | `44-runtime-error-center.md`, `45-runtime-module-inventory.md` |
| 2. Inventory + rename map | DONE | `architecture-cleanup-inventory.md` 10 domain dirs classified |
| 3. Fallback scan | DONE | static gate `domain_dirs_have_no_fallback_or_salvage` pass; `rg` 0 hits in production code |
| 4. Error center mainline | DONE | `map_runtime_error_through_error_pipeline` called in closure_runtime.rs; ErrorErr01-05 chain verified |
| 5a-c. Pipeline/closure/context split | DONE | 10/8/10 files in domain dirs |
| 5d. Tools split | DONE | 34 files in tools/ |
| 5e. Session+control split | DONE | 7/8 files in domain dirs |
| 5f. Task+agent split | DONE | 6/3 files in domain dirs |
| 5g. Runtime_home split | DONE | 3 files (skill_loader, source_visibility) |
| 5h. Model+prompt split | DONE | 7/6 files in domain dirs |
| 6. Provider hub decision | DONE | `hub_pipeline` deleted; `provider_facade` moved to production (removed `#[cfg(test)]` gate) |
| 7a. Naming/boundary static tests | DONE | 13/13 pass in `naming_static_tests.rs` |
| 7b. Extended naming tests | DONE | tools_mod_v4a documented (1 occurrence) |
| 8. Validation matrix L1-L5 | L1+L2 DONE, L3-L5 DEFERRED | L1: 132+13=145 tests; L2: 13/13 gate; L3-L5: external deps |

## L1 Evidence (2026-06-06)

```
fin-runtime: 132 passed, 0 failed
fin-provider: 13 passed, 0 failed
fin-cli: builds successfully (1 unused import warning)
```

## L2 Evidence (2026-06-06)

13/13 naming_static_tests pass:
- error_pipeline_no_swallow_no_fallback ✓
- error_pipeline_no_forbidden_numbering ✓
- feedback_pipeline_no_from_no_fallback ✓
- pipeline_modules_use_crate_path_not_legacy ✓
- provider_hub_pipeline_fully_deleted ✓
- lib_rs_domain_dirs_have_mod_entries ✓ (10 domain dirs)
- domain_mods_do_not_use_legacy_crate_paths ✓
- domain_dirs_have_no_fallback_or_salvage ✓
- cross_domain_no_direct_crate_file_imports ✓
- lib_rs_extended_mod_declarations_count_is_documented ✓
- tools_mod_v4a_mod_declarations_count_is_documented ✓
- lib_rs_no_helpers_or_support_mod_declarations ✓
- pipeline_pipeline_nodes_never_use_legacy_inline_node_name ✓

## Phase 6 Fix (2026-06-06)

- Removed `#[cfg(test)]` from `provider_facade` in `provider/lib.rs` — production builds now have full provider execution
- Removed `#[cfg(test)]` from `mod task` in `runtime/lib.rs` — task module was incorrectly gated behind test-only
- Both fixes verified: 145 tests pass + CLI builds

## Domain Directory Inventory (final)

```
agent/          3 files    naming + tests
closure/       10 files    rounds/retry/state/events/finalize/checkpoint/records + bridges
context/       10 files    view/blocks/block_render/project_support + 4 test files
control/        8 files    feedback/plane/routing/scheduler/owner_loop + test
model/          7 files    parser/shapes/input_assembler + tests
pipeline/      10 files    input/reason/feedback/error + 4 static tests + naming gate
prompt/         6 files    assembly + basics/catalog/role_policy tests
runtime_home/   3 files    skill_loader + source_visibility
session/        7 files    materializer/journal/turn/trace + support
task/           6 files    assignment_queue/board_snapshot/handoff/managed_board/store
tools/         34 files    dispatch/catalog/semantics/history_render + extended family + tests
```

## L3-L5 Deferred (external deps)

- L3 fault injection: needs live provider execution paths
- L4 live provider smoke: needs API key configuration
- L5 web/debug manual: needs running server

## Commits (this session)

```
61e9b80 fix: remove #[cfg(test)] gate from task module and provider_facade
4870a07 docs: Phase 2 architecture cleanup inventory + domain dir status
d623fa7 refactor(runtime): split model/ + prompt/ domain dirs
267d7ff refactor(runtime): split prompt assembly + tests
```
