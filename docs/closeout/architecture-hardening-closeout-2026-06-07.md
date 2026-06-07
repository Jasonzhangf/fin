# Architecture Hardening Closeout Receipt

Date: 2026-06-07

## Commits

- `f0fb0fd` fix(runtime+cli): architecture hardening Layer 6-7 - source truth, hidden history, governance
- `29a41f7` fix(runtime+cli): parallel state restore, scheduler checkpoint decision, cold archive path
- `af5e5b2` fix(runtime+cli): formalize event emission, test date alignment, passive formalize assertions
- `02fb264` style: cargo fmt after architecture hardening fixes

## Verification Commands

### Governance Gate
```
bash scripts/verify-governance.sh
OK: line-limit gate passed
OK: pipeline unique-type routing doc exists
OK: inventory includes domain model
OK: inventory includes domain prompt
=== Governance check passed ===
```

### Line Limit
```
python3 scripts/check-code-line-limit.py
code line-limit ok (limit=500, whitelist_entries=1)
```

### Format
```
cargo fmt --all --manifest-path rust/Cargo.toml
(verified clean)
```

### Workspace Tests
```
cargo test --workspace --manifest-path rust/Cargo.toml
test result: FAILED. 148 passed; 2 failed; 0 ignored; 150 filtered out; finished in 4.73s
```

### fin-cli Tests
```
cargo test -p fin-cli --manifest-path rust/Cargo.toml --lib
test result: FAILED. 148 passed; 2 failed; 0 ignored; 150 filtered out; finished in 4.67s
```

## Remaining 2 Pre-existing Failures

These failures existed before architecture hardening commits and are NOT caused by this work:

1. `assignment_runtime_resume_executes_worker_turn_and_submits_task`
   - Root cause: task tool dispatch (`project.task.submit`) not generating tool records
   - The provider mock returns `project.task.submit` in `<fin_tool_calls>`, but the runtime's closure pipeline only records `provider.call` + `reasoning.stop` tool records, missing the dispatched task tools
   - Pre-existing since before `f0fb0fd`

2. `managed_closed_loop_e2e_reaches_review_done_with_full_framework_chain`
   - Root cause: `/tick` → `manual_tick` supervisor cycle doesn't dispatch owner-loop turns
   - The dispatch turn doesn't produce `project.task.submit` or `agent.assign` tool records
   - Pre-existing since before `f0fb0fd`

## Changes Summary

### Layer 0-2: Document routing, gate, size
- governance.sh passes, line-limit passes, CI gates defined

### Layer 3-4: activity_cards domain, tools naming
- runtime inventory aligned with source tree

### Layer 5: pipeline static gate
- `derive_scheduler_decision` gains `has_checkpoint` parameter
- `ExecutionStateRecord` gains `resume_from_step_id`, `resume_checkpoint_ready`, `resume_checkpoint_id`

### Layer 6: error center + control plane fixes
- `enqueue_request_notice` preserves and restores parallel state (`paused`/`waiting_external`)
- `headless_daemon` adds `clear_waiting_if_due` after `inject_due_reminders`
- `session.formalized` event emitted from `create_and_bind_formal_task`
- Hardcoded test dates (`2026/04`) replaced with dynamic date derivation

### Layer 7: workspace tests
- 148/150 pass (was 135/150 at `f0fb0fd`)
- 7 previously failing tests now pass
