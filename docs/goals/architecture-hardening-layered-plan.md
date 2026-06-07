# Architecture Hardening Layered Implementation Plan

## 1. Goal and Acceptance Criteria

Goal: turn the current architecture rules from docs-only / partial gates into enforceable repo structure, CI gates, owner registry, and runtime truth paths.

This plan is the execution entry for the gaps found in the 2026-06-06 architecture review. It builds on:

- `docs/goals/architecture-cleanup-plan.md`
- `docs/goals/pipeline-unique-type-refactor-plan.md`
- `docs/architecture/02-layer-boundaries.md`
- `docs/architecture/08-testing-and-ci-strategy.md`
- `docs/architecture/09-workspace-and-crate-map.md`
- `docs/architecture/44-runtime-error-center.md`
- `docs/architecture/45-runtime-module-inventory.md`

Acceptance criteria:

1. CI enforces the same gates required by `docs/architecture/08-testing-and-ci-strategy.md`.
2. Runtime domain inventory matches the actual `rust/crates/runtime/src/` tree and is queryable through `feature_id -> owner -> tests`.
3. Pipeline unique-type routing points to an existing design document and gate coverage.
4. `tools` no longer exposes long-term `extended` / `v4a` / generic `support` names on the main path.
5. `activity_cards` is a real domain directory, not root-level files with `#[path]` bridges.
6. Runtime failure path persists `RuntimeError -> ErrorErr01..05 -> events + ledger + user-visible decision` as the unique runtime error truth.
7. Line-limit gate passes with only explicitly justified whitelist entries.
8. Verification evidence is recorded in closeout receipts; no completion claim without commands and results.

## 2. Scope and Boundaries

In scope:

1. Governance scripts and CI gate wiring.
2. Runtime module inventory, function map, and verification map updates.
3. Pipeline unique-type design routing and static gate expansion.
4. Runtime domain cleanup for `tools` and `activity_cards`.
5. Error center closeout from skeleton to persisted unique truth.
6. Targeted tests, static gates, and closeout receipt.

Out of scope:

1. No provider protocol redesign.
2. No UI/Web redesign except consuming existing runtime truth if a test requires it.
3. No new user-facing feature.
4. No prompt compression as a test-passing tactic.
5. No broad delete. Physical removal is allowed only after owner, references, and tests prove the code is obsolete or wrongly located.

## 3. Layered Design

### Layer 0: Truth Routing and Plan Alignment

Purpose: fix broken routing before touching runtime code.

Current gaps:

- `skills/fin-general-dev/SKILL.md` references `docs/architecture/44-pipeline-unique-type-and-error-chain.md`, but the file does not exist.
- `docs/architecture/45-runtime-module-inventory.md` still describes 9 runtime domains while code already has `model/` and `prompt/`.
- Existing cleanup docs contain stale Phase 5 debt after later partial cleanup.

Implementation:

1. Create or route to a real pipeline unique-type design doc:
   - Preferred: create `docs/architecture/46-pipeline-unique-type-and-error-chain.md`.
   - Update `skills/fin-general-dev/SKILL.md` to reference that path.
   - Update `AGENTS.md` route-map if the new doc is added.
2. Update `docs/architecture/45-runtime-module-inventory.md`:
   - Include actual domains: `pipeline`, `closure`, `context`, `tools`, `session`, `control`, `task`, `agent`, `runtime_home`, `model`, `prompt`, and planned `activity_cards`.
   - Reconcile root-level debt list with current code.
   - Update function map and verification map for `model_output_parse`, `prompt_assembly`, `activity_cards`, and `dispatch_capabilities`.
3. Add a short closeout note in `docs/closeout/` when this layer is complete.

Files:

- `skills/fin-general-dev/SKILL.md`
- `AGENTS.md`
- `docs/architecture/45-runtime-module-inventory.md`
- `docs/architecture/46-pipeline-unique-type-and-error-chain.md`
- `docs/closeout/<receipt>.md`

Verification:

- `test -f docs/architecture/46-pipeline-unique-type-and-error-chain.md`
- `rg -n "44-pipeline-unique-type-and-error-chain|46-pipeline-unique-type-and-error-chain" AGENTS.md skills docs`
- `rg -n "model|prompt|activity_cards" docs/architecture/45-runtime-module-inventory.md`

### Layer 1: Governance and CI Gates

Purpose: make architecture rules executable.

Current gaps:

- `.github/workflows/ci.yml` only runs `verify-governance.sh` and `cargo test --workspace`.
- `verify-governance.sh` only checks file existence.
- `python3 scripts/check-code-line-limit.py` currently fails with 12 non-whitelisted files over 500 lines.

Implementation:

1. Extend `scripts/verify-governance.sh` from skeleton check to rule check:
   - Required docs exist.
   - Line-limit passes.
   - Pipeline unique-type doc route exists.
   - Runtime inventory mentions all actual domain dirs.
   - Main-path forbidden naming scan has expected zero or explicit backlog count.
2. Update `.github/workflows/ci.yml`:
   - Run governance.
   - Run line-limit directly, even if governance also runs it.
   - Run `cargo fmt --check`.
   - Run `cargo test --workspace`.
3. Add optional `scripts/verify-runtime-architecture.py` if shell grows too complex:
   - Compare `rust/crates/runtime/src/*/mod.rs` directories to inventory domain table.
   - Check no root-level `activity_cards_*.rs` after Layer 3.
   - Check `_v4a` count after Layer 4 target is zero.
4. Do not whitelist oversized files silently. Each whitelist entry needs owner and reason in `scripts/line-limit-whitelist.txt`.

Files:

- `.github/workflows/ci.yml`
- `scripts/verify-governance.sh`
- `scripts/verify-runtime-architecture.py` if needed
- `scripts/line-limit-whitelist.txt`

Verification:

- `./scripts/verify-governance.sh`
- `python3 scripts/check-code-line-limit.py`
- `cargo fmt --check --manifest-path rust/Cargo.toml`
- `cargo test --workspace --manifest-path rust/Cargo.toml`

### Layer 2: File Size and Test Structure Cleanup

Purpose: make line-limit gate pass without hiding real module debt.

Current failing files from review:

- `rust/crates/cli/src/channel_peer_activity_delivery_tests.rs`
- `rust/crates/cli/src/channel_peer_activity_delivery_tests_dev.rs`
- `rust/crates/cli/src/channel_peer_activity_delivery_tests_periodic.rs`
- `rust/crates/cli/src/channel_peer_conversations.rs`
- `rust/crates/cli/src/channel_peer_qqbot_bridge_support.rs`
- `rust/crates/cli/src/tests.rs`
- `rust/crates/provider/src/provider_facade.rs`
- `rust/crates/provider/src/tests.rs`
- `rust/crates/runtime/src/activity_cards.rs`
- `rust/crates/runtime/src/closure_runtime.rs`
- `rust/crates/runtime/src/model_output_runtime_tests_basic.rs`
- `rust/crates/runtime/src/session/materializer.rs`

Implementation order:

1. Split test files first because they are lower behavior risk:
   - group by behavior family, not arbitrary line ranges.
   - parent test module should only declare submodules and shared helpers.
2. Split `provider_facade.rs` by provider protocol blocks:
   - facade entry remains thin.
   - OpenAI/Anthropic execution branches move to owned protocol files if not already present.
   - Add test ensuring registered provider protocols equal executable facade branches.
3. Split CLI channel support:
   - bridge handler stays thin.
   - delivery render, conversation registry, QQBot adapter, and receipt helpers get separate owners.
4. Split runtime files only after domain inventory is updated:
   - `activity_cards.rs` handled in Layer 3.
   - `closure_runtime.rs` handled after error center and closure module boundaries are clear.
   - `session/materializer.rs` split into materialization orchestration, artifact writers, checkpoint writer, and tests.

Rules:

- No broad rewrite.
- No behavior change without tests.
- Do not add new whitelist entries unless the file is a documented top-level orchestrator and has a future split plan.

Verification:

- `python3 scripts/check-code-line-limit.py`
- Targeted crate test after each split:
  - `cargo test -p fin-cli --manifest-path rust/Cargo.toml`
  - `cargo test -p fin-provider --manifest-path rust/Cargo.toml`
  - `cargo test -p fin-runtime --manifest-path rust/Cargo.toml`

### Layer 3: Runtime Domain Inventory and `activity_cards` Domain

Purpose: align physical layout with owning domains.

Current gaps:

- `activity_cards.rs` is root-level.
- It imports `activity_cards_agents.rs`, `activity_cards_helpers.rs`, and `activity_cards_store.rs` through `#[path]`.
- Inventory does not list `activity_cards` as a domain.

Target layout:

```text
rust/crates/runtime/src/activity_cards/
  mod.rs
  agents.rs
  store.rs
  helpers.rs
  tests.rs
  delivery_tests.rs
```

Implementation:

1. Add `activity_cards` to inventory and function map:
   - `feature_id`: `activity_cards`
   - owner module: `activity_cards`
   - canonical types: `ActivityCardsSnapshot`, `SourceActivityCardView`, `UserActivityCardView`
   - canonical builder: `activity_cards::build_activity_cards_for_session`
   - forbidden paths: CLI/Web/channel adapters must not rebuild card semantics.
2. Physically move files into `activity_cards/`.
3. Replace `#[path]` bridges with normal `mod` declarations.
4. Keep `lib.rs` public export stable:
   - `pub use activity_cards::{build_activity_cards, build_activity_cards_for_session};`
5. Add static gate:
   - no root-level `activity_cards_*.rs`.
   - no `#[path = "activity_cards_*.rs"]`.

Verification:

- `cargo test -p fin-runtime activity_cards --manifest-path rust/Cargo.toml`
- `cargo test -p fin-runtime --manifest-path rust/Cargo.toml`
- `python3 scripts/check-code-line-limit.py`

### Layer 4: Tools Domain Naming Closeout

Purpose: remove migration names from the main runtime path.

Current gaps:

- `tools/mod.rs` still declares `dispatch_router_*`.
- `dispatch_patch_apply.rs` remains.
- `dispatch_control_args.rs` and `dispatch_peer_records.rs` use generic support naming.

Target layout:

```text
rust/crates/runtime/src/tools/
  catalog.rs
  catalog_dynamic.rs
  catalog_task_tools.rs
  dispatch.rs
  dispatch_assignment.rs
  dispatch_control.rs
  dispatch_control_args.rs
  dispatch_query.rs
  dispatch_query_control.rs
  dispatch_query_history.rs
  dispatch_query_image.rs
  dispatch_query_task.rs
  dispatch_exec.rs
  dispatch_exec_receipts.rs
  dispatch_patch.rs
  dispatch_patch_apply.rs
  dispatch_task_write.rs
  dispatch_collab.rs
  dispatch_collab_mailbox.rs
  dispatch_collab_coordination.rs
  dispatch_peer.rs
  dispatch_peer_records.rs
  dispatch_result_receipts.rs
  history_render.rs
  semantics.rs
```

Implementation:

1. Build rename map from old files to new files.
2. Move files physically with `git mv` or normal filesystem move through shell only when safe; update module declarations.
3. Update all imports.
4. Change naming static gate:
   - `_v4a` count must be zero.
   - `dispatch_router` declarations must be zero in `tools/mod.rs`, not only in `lib.rs`.
   - generic `support` names are forbidden unless listed as temporary with a closeout date.
5. Remove old file names physically.

Verification:

- `cargo test -p fin-runtime tools --manifest-path rust/Cargo.toml`
- `cargo test -p fin-runtime pipeline::naming_static_tests --manifest-path rust/Cargo.toml`
- `rg -n "dispatch_router|_v4a|dispatch_.*_support" rust/crates/runtime/src/tools`

### Layer 5: Pipeline Unique-Type Gate Closeout

Purpose: make pipeline boundary rules enforceable.

Current status:

- Input, reason, feedback, and error pipeline files exist.
- Static tests cover some naming, no fallback strings, and some `impl From` cases.
- Design routing is broken because the referenced pipeline doc is missing.

Implementation:

1. Add the missing design doc from Layer 0.
2. Expand static tests:
   - no non-adjacent builder names in pipeline files.
   - no `impl From<...>` across pipeline domains.
   - no duplicate `*Request` / `*Response` synonyms for pipeline nodes.
   - no forbidden numbering: `03a`, `03_1`, `03.5`, `V2`.
3. Add owner registry check:
   - every `ReasonReq*`, `ReasonResp*`, `FeedbackResp*`, `InputIn*`, `ErrorErr*` must have a row in inventory or pipeline doc.
4. Lock current builder/parser ownership.
5. Do not rename published node numbers unless creating a new chain version with deletion plan.

Files:

- `rust/crates/runtime/src/pipeline/naming_static_tests.rs`
- `rust/crates/runtime/src/pipeline/*_static_tests.rs`
- `docs/architecture/46-pipeline-unique-type-and-error-chain.md`
- `docs/architecture/45-runtime-module-inventory.md`

Verification:

- `cargo test -p fin-runtime pipeline --manifest-path rust/Cargo.toml`
- `cargo test -p fin-runtime pipeline::naming_static_tests --manifest-path rust/Cargo.toml`

### Layer 6: Runtime Error Center Closeout

Purpose: move from "mapped skeleton" to durable unique error truth.

Current status:

- `M1Runtime::run_closure` catches `RuntimeError`.
- It maps through `ErrorErr01..05`.
- It emits `error.detected` and `error.user_visible_prepared` into `last_error_events`.
- It still returns `Err(RuntimeError)` without a full durable failed closure receipt and full error ledger chain.

Target behavior:

```text
run_closure
  -> run_closure_inner
  -> Err(RuntimeError)
  -> ErrorErr01Detected
  -> ErrorErr02SourceClassified
  -> ErrorErr03RuntimeClassified
  -> ErrorErr04SessionRecorded
  -> ErrorErr05UserVisible
  -> persist error ledger
  -> emit error.* events
  -> append operation.failed / inference.failed as appropriate
  -> return explicit failed receipt or RuntimeError carrying recorded receipt
```

Implementation:

1. Define the durable error ledger record shape in runtime or contracts if it crosses crate boundaries.
2. Persist `ErrorErr04SessionRecorded.ledger_path` to session truth, not just a string.
3. Emit all required events:
   - `error.detected`
   - `error.source_classified`
   - `error.runtime_classified`
   - `error.session_recorded`
   - `error.user_visible_prepared`
   - business failure event such as `operation.failed`.
4. Add tests for:
   - provider HTTP failure.
   - missing config / invalid operation.
   - invalid model output after retry budget.
   - tool execution failure.
5. Ensure CLI/debug/Web only consume error events and ledger; they must not reclassify runtime failures.

Verification:

- `cargo test -p fin-runtime run_closure_error_center_tests --manifest-path rust/Cargo.toml`
- `cargo test -p fin-runtime fault_injection_tests --manifest-path rust/Cargo.toml`
- `rg -n "map_runtime_error_through_error_pipeline" rust/crates/runtime/src`
- inspect generated test ledger path in isolated runtime home if the test creates one.

### Layer 7: Harness, Smoke, and Receipt Closeout

Purpose: prove architecture hardening works beyond unit tests.

Implementation:

1. Update closeout receipt template:
   - governance result.
   - line-limit result.
   - static gate result.
   - workspace test result.
   - targeted feature tests.
   - live/provider/manual items explicitly marked run or deferred with reason.
2. Run standard local verification:
   - `./scripts/verify-governance.sh`
   - `python3 scripts/check-code-line-limit.py`
   - `cargo fmt --check --manifest-path rust/Cargo.toml`
   - `cargo test --workspace --manifest-path rust/Cargo.toml`
3. If provider keys/config are available:
   - `scripts/run-real-provider-smoke.sh test-architecture-hardening-<timestamp>`
4. If Web debug is affected:
   - run local debug server in foreground.
   - verify raw event/projection consistency.
5. Create closeout receipt:
   - `docs/closeout/architecture-hardening-<date>.md`

Completion standard:

- All locally runnable gates pass.
- External/live gates either pass or are explicitly marked deferred with exact missing dependency.
- No stale docs contradict current source layout.

## 4. Risk and Mitigation

1. Risk: CI starts failing immediately after adding line-limit.
   - Mitigation: wire line-limit only after Layer 2 passes locally, or add a temporary explicit "known failing" job during the same branch only; final state must fail hard.

2. Risk: moving files breaks module visibility.
   - Mitigation: move one domain at a time; run targeted test after each move; avoid `#[cfg(test)]` on production modules.

3. Risk: error center changes alter public runtime API.
   - Mitigation: keep `run_closure` signature stable until durable receipt shape is proven; if API change is required, update owner registry and callers in the same phase.

4. Risk: static scans overmatch words in docs/tests.
   - Mitigation: scans should target production source or explicitly filter docs/tests where appropriate; tests can mention forbidden words as assertions.

5. Risk: duplicate implementation survives after rename.
   - Mitigation: every rename phase ends with `rg` for old names and a physical-delete check.

## 5. Verification Matrix

| Layer | Required verification |
| --- | --- |
| 0 | route/doc existence checks; `rg` proves no broken pipeline doc route |
| 1 | `./scripts/verify-governance.sh`; CI includes governance, line-limit, fmt, tests |
| 2 | `python3 scripts/check-code-line-limit.py`; targeted crate tests |
| 3 | `cargo test -p fin-runtime activity_cards`; no root-level `activity_cards_*.rs` |
| 4 | `cargo test -p fin-runtime tools`; no `extended/v4a/support` main-path names |
| 5 | `cargo test -p fin-runtime pipeline`; expanded naming static tests pass |
| 6 | error center tests + fault injection tests; persisted ledger evidence |
| 7 | full workspace tests + closeout receipt |

## 6. Implementation Order

1. Layer 0: repair docs/routing first.
2. Layer 1: prepare governance scripts, but do not merge final hard CI until line-limit can pass.
3. Layer 2: split oversized files until line-limit passes.
4. Layer 3: move `activity_cards` into a real domain.
5. Layer 4: rename tools domain away from migration names.
6. Layer 5: expand pipeline static gates and owner registry checks.
7. Layer 6: close runtime error center durability.
8. Layer 7: run verification and write receipt.

## 7. Definition of Done

1. `docs/architecture/45-runtime-module-inventory.md` matches actual runtime domains.
2. Pipeline unique-type routing points to an existing doc.
3. `python3 scripts/check-code-line-limit.py` passes.
4. `./scripts/verify-governance.sh` validates real architecture rules, not only file existence.
5. `.github/workflows/ci.yml` runs governance, line-limit, fmt, and workspace tests.
6. `activity_cards` is a domain directory with no root-level bridge files.
7. `tools` main path has no `extended`, `_v4a`, or generic `support` module names.
8. Error center persists full error ledger and emits full required event chain.
9. `cargo test --workspace --manifest-path rust/Cargo.toml` passes.
10. A closeout receipt records all verification commands and results.
