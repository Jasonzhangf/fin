# Architecture Cleanup Receipt (2026-06-05 v4 — final)

## Status

| Phase | Status | Evidence |
| --- | --- | --- |
| 1. Docs truth frozen | DONE | `44-runtime-error-center.md`, `45-runtime-module-inventory.md` |
| 2. Inventory + rename map | DONE | `45-runtime-module-inventory.md` 117 files classified |
| 3. Fallback scan | DONE | no silent fallback in `crate::*PipelineReq*` mainline |
| 4. Error center mainline | DONE | `run_closure` failures route through `ErrorErr*` chain |
| 5a. Pipeline domain split | DONE | `pipeline/` subdirectory 10 files |
| 5b. Closure domain split | DONE | `closure/` subdirectory 8 files |
| 5c. Context domain split | DONE | `context/` subdirectory 9 files |
| 5d. Tools domain split | DEFERRED | bridge approach failed; tracked in M2 backlog (see `note.md` 2026-06-05T17:00) |
| 5e. Session+control split | DEFERRED | bridge approach failed; tracked in M2 backlog (see `note.md` 2026-06-05T17:15) |
| 6. Provider hub decision | DONE | `hub_pipeline.rs` deleted; 19 dead_code warnings cleared |
| 7a. Naming/boundary static tests | DONE | `naming_static_tests.rs` 5 tests for cleaned modules |
| 7b. Extended naming tests | DONE | 9 additional static tests covering extended/v4a/helpers/support/legacy naming |
| 8. Validation matrix L1-L5 | PARTIAL | L1 + L2 verified; L3-L5 not run (see below) |

## Phase 5d/5e lessons (see `note.md` 2026-06-05)

The 5-step bridge domain split pattern works for **cross-domain refs < 15**:
1. `pipeline/` (10 files, ~5 cross-refs) — OK
2. `closure/` (8 files, ~8 cross-refs) — OK
3. `context/` (9 files, ~6 cross-refs) — OK
4. `tools/` (35 files, ~20+ cross-refs) — FAIL (196 errors)
5. `session+control/` (22 files, ~25+ cross-refs) — FAIL (142 errors)

For domains with > 15 cross-references, use a **physical-move pattern** instead:
1. `git mv` all files into subdirectory
2. Single sweep rewrite all `crate::X::` → `crate::domain::X::`
3. Add `use crate::*;` to each sub-file (for `use super::*` resolution)
4. `cargo build` one-shot fix
5. `cargo test` verify

This pattern is deferred from this work scope. Tracked as M2 backlog.

## Phase 8 Validation Matrix Evidence

### L1: Unit tests (all green)

```
fin-config       11 passed
fin-contracts     6 passed
fin-shared        3 passed
fin-provider     13 passed
fin-runtime     116 passed  (Phase 5b/5c/7b added 6 tests vs baseline 110)
```

### L2: Contract + static tests (all green)

Naming/boundary tests (14 tests in `naming_static_tests.rs`):
- `error_pipeline_no_swallow_no_fallback` ok
- `error_pipeline_no_forbidden_numbering` ok
- `feedback_pipeline_no_from_no_fallback` ok
- `pipeline_modules_use_crate_path_not_legacy` ok
- `provider_hub_pipeline_fully_deleted` ok
- `lib_rs_extended_mod_declarations_count_is_documented` ok (asserts 13 = known Phase 5d backlog)
- `lib_rs_v4a_mod_declarations_count_is_documented` ok (asserts 1 = tool_dispatch_extended_patch_v4a)
- `lib_rs_no_helpers_or_support_mod_declarations` ok
- `pipeline_pipeline_nodes_never_use_legacy_inline_node_name` ok
- + 5 more pipeline-specific static tests in error/reason/feedback/input_pipeline_static_tests.rs

Pipeline node uniqueness:
- `input_pipeline_uses_unique_node_type_names` ok
- `reasoning_pipeline_uses_unique_node_type_names_and_no_round_execution_alias` ok
- `feedback_pipeline_uses_unique_node_type_names_and_adjacent_parsers` ok
- `error_pipeline_uses_unique_node_type_names_and_no_swallow` ok

### L3: Fault injection

NOT RUN. Blocked on `ProviderFacade: InferenceProvider` trait impl (pre-existing on `stash@{0}` WIP, not in this work scope).

### L4: Live provider smoke

NOT RUN. Requires live API key (none configured in this work scope).

### L5: Web/debug manual

NOT RUN. Not exercised in this work scope; web debug console reads `~/.fin/runtime/current/*` artifacts but was not manually validated.

## Commits in scope (this work)

```
bcc131e test(runtime): fix expected count for lib_rs_extended_mod_declarations_count_is_documented
7b64533 test(runtime): phase 7b — extend naming/boundary static tests
0587248 docs(closeout): update architecture cleanup receipt to v3
a75b5ab docs(note): record phase 5d+5e domain split bridge approach boundary
08dc937 docs(note): record phase 5d tools bridge approach failure and lessons
4c04c8b refactor(runtime): finish context domain split (9 files into context/)
012bcf5 refactor(runtime): finish closure domain split (batch 2: 7 sub-files into closure/)
744813f refactor(runtime): start closure domain split (batch 1: mod.rs scaffold + rounds_tools visibility)
74b6df9 docs(note): record phase 5b small-batch strategy before starting
4215287 docs(closeout): architecture cleanup receipt (Phase 1-4, 5a, 6, 7a; 5b-5e and 8 backlogs)
a34766a test(runtime): add naming/boundary static tests for cleaned modules
55bf1ad chore(provider): remove hub_pipeline mod declarations
63252df refactor(provider): delete unused hub_pipeline skeleton
5e71f1a docs(architecture): define error center
```

## M2 backlog items

1. **tools/ domain split (Phase 5d)**: 35 files with ~20+ cross-refs need physical-move pattern
2. **session+control domain split (Phase 5e)**: 22 files with ~25+ cross-refs need physical-move pattern
3. **Phase 8 L3 fault injection**: requires ProviderFacade: InferenceProvider impl
4. **Phase 8 L4 live provider smoke**: requires live API key configuration
5. **Phase 8 L5 web/debug manual**: requires manual session with browser
