# Phase 2: Module Inventory + Rename Map

## Domain Directory Inventory (10 directories, 102 files)

| Domain | Files | Status | Notes |
|--------|-------|--------|-------|
| `agent/` | 3 | ✓ | naming.rs + naming_tests.rs + mod.rs |
| `closure/` | 8 | ✓ | checkpoint/events/finalize/records/retry/rounds/state/mod |
| `context/` | 10 | ✓ | block_render/blocks/view/project_support + 4 test files |
| `control/` | 8 | ✓ | feedback/plane/plane_segments/owner_loop/routing/scheduler + test |
| `model/` | 7 | ✓ | parser/shapes + input_assembler + 2 test files |
| `pipeline/` | 10 | ✓ | input/reason/feedback/error pipeline + 4 static test files |
| `prompt/` | 6 | ✓ | assembly + basics/catalog/role_policy tests + mod |
| `runtime_home/` | 3 | ✓ | skill_loader + source_visibility + mod |
| `session/` | 7 | ✓ | journal/materializer/materializer_events/materializer_support/trace/turn + mod |
| `task/` | 6 | ✓ | assignment_queue/board_snapshot/handoff/managed_board/store + mod |
| `tools/` | 34 | ⚠ | 30 dispatch + 4 catalog files |

### Root-level files remaining (27 total)

**Domain-belonging (18 files) — candidate for domain split:**

| File | Target Domain | Priority |
|------|-------------|----------|
| `activity_cards.rs` | `activity_cards/` | Medium |
| `activity_cards_agents.rs` | `activity_cards/` | Low (helpers) |
| `activity_cards_helpers.rs` | `activity_cards/` | Low (helpers) |
| `activity_cards_store.rs` | `activity_cards/` | Medium |
| `activity_cards_tests.rs` | `activity_cards/` | Low (test) |
| `activity_cards_tests_delivery.rs` | `activity_cards/` | Low (test) |
| `closure_runtime.rs` | `closure/` | High (bridge via #[path]) |
| `closure_runtime_rounds_tools.rs` | `closure/` | High (bridge via #[path]) |
| `model_output_feedback.rs` | `model/` | High (used by reason_pipeline) |
| `model_output_tool_calls.rs` | `model/` | High (used by reason_pipeline) |
| `model_output_runtime_tests_basic.rs` | `model/` | Low (test) |
| `model_output_runtime_tests_rounds.rs` | `model/` | Low (test) |
| `round_context.rs` | `context/` | High |
| `round_loop_runtime_tests.rs` | `closure/` | Low (test) |
| `round_loop_runtime_tests_contract_retry.rs` | `closure/` | Low (test) |
| `round_loop_runtime_tests_contract_retry_basic.rs` | `closure/` | Low (test) |
| `round_loop_runtime_tests_contract_retry_advanced.rs` | `closure/` | Low (test) |
| `round_loop_runtime_tests_full_history.rs` | `closure/` | Low (test) |
| `round_loop_runtime_tests_receipt_feedback.rs` | `closure/` | Low (test) |
| `run_closure_error_center_tests.rs` | `closure/` | Low (test) |
| `tests_context_render.rs` | `context/` | Low (test) |
| `tests_mainline.rs` | `closure/` | Low (test) |
| `tests_role_runtime.rs` | `closure/` | Low (test) |
| `tests.rs` | `closure/` | Low (test) |
| `execution_checkpoint_tests.rs` | `control/` | Low (test) |
| `fault_injection_tests.rs` | `pipeline/` | Low (test) |

**Not domain-belonging (2 files):**
- `lib.rs` — root entry
- `round_context.rs` — context assembly, used by model/input_assembler

**Duplicate/dead code (2 files):**
- `model_output_feedback.rs` — duplicate of model/feedback logic, used only by reason_pipeline.rs
- `model_output_tool_calls.rs` — duplicate, used only by reason_pipeline.rs

## Naming Issues

### Forbidden patterns found in root files

| Pattern | Found In | Severity |
|---------|---------|----------|
| `extended` | 17 files in tools/ | Medium (intentional per Phase 5d backlog) |
| `_v4a` | 1 file (`tool_dispatch_extended_patch_v4a.rs`) | Medium (Phase 5d backlog) |
| `helpers` | `activity_cards_helpers.rs` | Low |
| `support` | `session_materializer_support.rs`, `tool_dispatch_control_support.rs`, `tool_dispatch_peer_support.rs` | Low |
| `round_loop_runtime_tests_*` | 6 files | Low (tests, naming OK) |

## Evidence

- `cargo test -p fin-runtime --lib`: 132 passed, 0 failed
- `cargo test -p fin-provider --lib`: 13 passed, 0 failed
- `cargo test -p fin-runtime --lib domain_mods_do_not_use_legacy_crate_paths`: 1 passed

## Phase 2 Conclusion

Most of the remaining root-level files are test files (~20) that don't need domain split
(they're inline integration/e2e tests). The high-priority non-test files that need
domain split are:
1. `model_output_feedback.rs` → `model/` (duplicate)
2. `model_output_tool_calls.rs` → `model/` (duplicate)
3. `round_context.rs` → `context/` or `model/`
4. `closure_runtime.rs` → `closure/` (already bridged)
5. `closure_runtime_rounds_tools.rs` → `closure/` (already bridged)

The duplicate files (model_output_feedback, model_output_tool_calls) should be
physically deleted after confirming their logic is absorbed into the model/ domain.
