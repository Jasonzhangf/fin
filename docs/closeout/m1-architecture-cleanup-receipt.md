# Architecture Cleanup Receipt (2026-06-05 v3)

## Status

| Phase | Status | Evidence |
| --- | --- | --- |
| 1. Docs truth frozen | DONE | `44-runtime-error-center.md`, `45-runtime-module-inventory.md` |
| 2. Inventory + rename map | DONE | `45-runtime-module-inventory.md` 117 files classified |
| 3. Fallback scan | DONE | no silent fallback in `crate::*PipelineReq*` mainline |
| 4. Error center mainline | DONE | `run_closure` failures route through `ErrorErr*` chain |
| 5a. Pipeline domain split | DONE | `pipeline/` subdirectory 10 files |
| 5b. Closure domain split | DONE | `closure/` subdirectory 8 files (commits `744813f`, `012bcf5`) |
| 5c. Context domain split | DONE | `context/` subdirectory 9 files (commit `4c04c8b`) |
| 5d. Tools domain split | SKIPPED | bridge approach failed (196 errors); heavy cross-refs need physical-move mode. See `note.md` 2026-06-05T17:00. |
| 5e. Session+control split | SKIPPED | bridge approach failed (142 errors). See `note.md` 2026-06-05T17:15. |
| 6. Provider hub decision | DONE | `hub_pipeline.rs` deleted; 19 dead_code warnings cleared |
| 7a. Naming/boundary static tests | DONE | `naming_static_tests.rs` 5 tests for cleaned modules |
| 7b. Expand naming tests | IN PROGRESS | see this PR |
| 8. Validation matrix L1-L5 | PARTIAL | L1 + L2 verified; L3-L5 still pending |

## L1: Unit tests (after Phase 5c, before 5d attempt)

```
fin-config       11 passed
fin-contracts     6 passed
fin-shared        3 passed
fin-provider     13 passed
fin-runtime     112 passed  (Phase 5c added 2 tests)
fin-orchestrator  2 passed
fin-debug-server 32 passed
```

## L2: Contract + static tests

- pipeline node uniqueness: `input_pipeline_uses_unique_node_type_names` ok
- reason pipeline: `reasoning_pipeline_uses_unique_node_type_names_and_no_round_execution_alias` ok
- feedback pipeline: `feedback_pipeline_uses_unique_node_type_names_and_adjacent_parsers` ok
- error pipeline: `error_pipeline_uses_unique_node_type_names_and_no_swallow` ok
- naming: `pipeline_modules_use_crate_path_not_legacy` ok
- naming: `feedback_pipeline_no_from_no_fallback` ok
- naming: `provider_hub_pipeline_fully_deleted` ok
- naming: `error_pipeline_no_swallow_no_fallback` ok
- naming: `error_pipeline_no_forbidden_numbering` ok

## L3: Fault injection

NOT RUN. Blocked on `ProviderFacade: InferenceProvider` trait impl (pre-existing on `stash@{0}` WIP).

## L4: Live provider smoke

NOT RUN. Requires live API key.

## L5: Web/debug manual

NOT RUN. Not exercised in this work.

## Phase 5d/5e lessons (documented in `note.md` 2026-06-05)

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

This pattern was deferred from this work scope. Tracked as M2 backlog.
