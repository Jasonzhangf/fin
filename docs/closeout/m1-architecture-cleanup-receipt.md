# Architecture Cleanup Receipt (2026-06-05)

## Status

| Phase | Status | Evidence |
| --- | --- | --- |
| 1. Docs truth frozen | DONE | commits `5e71f1a`, `f9a19ed`; docs `44-runtime-error-center.md`, `45-runtime-module-inventory.md` |
| 2. Inventory + rename map | DONE | `45-runtime-module-inventory.md` 117 files classified by domain |
| 3. Fallback scan | DONE | no silent fallback in `crate::*PipelineReq*` mainline; `unwrap_or(stripped)` is legitimate display, `default_tool_calls` is legitimate absence |
| 4. Error center mainline | DONE | commit `a3f8746 refactor(runtime): route run_closure failures through error pipeline`; 1 new test in `run_closure_error_center_tests.rs` |
| 5a. Pipeline domain split | DONE | commit `6f078b9 refactor(runtime): move pipeline files into pipeline/ subdirectory`; 9 files in `pipeline/` |
| 5b-5e. Other domain splits | NOT DONE | 16 `tool_dispatch_extended_*` files still in place; 1 attempt rolled back |
| 6. Provider hub decision | DONE | commits `63252df`, `55bf1ad`; `hub_pipeline.rs` + `hub_pipeline_static_tests.rs` deleted; 19 dead_code warnings cleared |
| 7a. Naming/boundary static tests | DONE | commit `a34766a test(runtime): add naming/boundary static tests for cleaned modules`; 5 new tests in `naming_static_tests.rs` |
| 7b. Expand naming tests | NOTDONE | depends on 5d |
| 8. Validation matrix L1-L5 | PARTIAL | L1 + L2 verified; L3-L5 not run |

## L1: Unit tests

```
fin-config      11 passed
fin-contracts    6 passed
fin-shared       3 passed
fin-provider    13 passed
fin-runtime    110 passed  (1 new test in run_closure_error_center_tests)
fin-orchestrator 2 passed
fin-registry     0 passed
fin-harness-core 0 passed
fin-debug-server 32 passed
fin-cli         NOT COMPILED (pre-existing on stash WIP)
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

NOT RUN. Requires real `ProviderFacade` impl `InferenceProvider` trait — pre-existing on `stash@{0}` WIP, not in this work scope.

## L4: Cluster / multi-worker

NOT RUN. Single worktree scope; harness harness `fin-harness-core` 0 tests.

## L5: Web debug manual

NOT RUN. Web debug console reads `~/.fin/runtime/current/*` artifacts; not exercised in this work.

## Known gaps and follow-up

- `fin-cli` compilation error: `ProviderFacade: InferenceProvider` not satisfied. Pre-existing on `stash@{0}` WIP `571c87e test(provider): execute_prepared protocol coverage invariant`. To fix: ensure `pub trait InferenceProvider` is re-exported from `fin_provider::lib` (or re-add impl block to `ProviderFacade`).
- `tool_dispatch_extended_*` 16 files: legacy naming. Phase 5d is to merge business into `tool_dispatch.rs` then delete. Not done in this work; tracked in `docs/goals/architecture-cleanup-plan.md` Phase 5d-5e.
- 5b-5e domain split for closure / context / tools / session / control: not done. Pipeline domain is the only successful split.
- 7b expand naming tests to full `runtime/src/`: blocked on 5d cleanup.

## Commits in scope (this work)

```
a34766a test(runtime): add naming/boundary static tests for cleaned modules
55bf1ad chore(provider): remove hub_pipeline mod declarations
63252df refactor(provider): delete unused hub_pipeline skeleton
a6357e3 docs(note): plan phase 5b-5e small batches + phase 6-8
4c09bb5 docs(note): record phase 5b rollback and lessons learned
6f078b9 refactor(runtime): move pipeline files into pipeline/ subdirectory
a3f8746 refactor(runtime): route run_closure failures through error pipeline
f9a19ed docs(architecture): add runtime inventory and cleanup plan
5e71f1a docs(architecture): define error center
```

(Plus 1 commit that was `docs(goals)` for the plan doc, also pushed earlier.)
