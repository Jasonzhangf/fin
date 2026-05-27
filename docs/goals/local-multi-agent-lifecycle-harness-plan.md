# Local Multi-Agent Lifecycle Harness Plan

## Objective
Build and verify a real local multi-agent lifecycle harness for fin: a default system agent starts from the standard system-agent path, discovers/configures a project agent, sends a task, receives progress/tool-turn updates, receives final result refs, and records the whole lifecycle in runtime truth and ledger/projection artifacts.

## Completion Rule
This feature is not complete until tests are designed first, implemented, then code is implemented, and finally a real local run proves the main lifecycle. Unit tests alone are insufficient.

## Scope

### In Scope
- Local two-instance harness using isolated runtime homes and disposable namespaces.
- System agent default startup path.
- Project agent dynamic config with distinct cwd and persisted auto-selected port.
- Headless project agent parity with headed/UI agent configuration: cwd, session binding, slashcommand/control commands, and non-AI harness controls must be available through a standard command set.
- Authenticated system-to-project connection.
- Online project-agent discovery by `machine.agentname` or equivalent canonical identity.
- Task dispatch from system agent to project agent.
- Project execution turn with progress/tool events.
- Result refs returned to system agent.
- Ledger/projection evidence for both agents.
- Disconnect, lost connection, reconnect, auth failure, unavailable agent, execution error, and recovery cases.
- Strict cleanup using explicit PID/service-scoped shutdown only.

### Out Of Scope
- UI-specific inference of agent state.
- Making subagent-local state globally visible.
- Broad process kill commands.
- Treating single-runtime unit tests as lifecycle completion evidence.

## Test-First Scenario Design

### 1. Registration And Startup
- Start isolated system agent with default system path.
- Add project agent config dynamically.
- Start project agent with separate cwd and persisted auto-selected port.
- Assert both agents have durable identity, listener endpoint, auth config, and ledger roots.
- Assert headless project agent exposes the same baseline controls as headed agent: configure cwd, bind/resume/new session, run slashcommand/control command, status/query, stop/restart, and strict ledger check.

### 2. Discovery And Auth
- System agent enumerates online project agents.
- Discovery includes `machine.agentname`, project id, endpoint, lease/heartbeat, and capability descriptor.
- Auth success allows mailbox/control-plane communication.
- Auth failure returns structured error and records runtime/ledger event.

### 3. Dispatch And Execution
- System agent dispatches a task to project agent.
- Project agent accepts task into durable mailbox/queue.
- Project agent runs one execution turn.
- Progress reports include turn started, tool/provider step started, tool/provider step completed/failed, turn completed.
- Project agent returns result refs to system agent.
- System agent records received result and final task status.

### 4. Ledger And Projection Assertions
- System ledger contains dispatch, mailbox, progress, result, and remote-agent lifecycle records.
- Project ledger contains task accepted, turn, steps, tools, provider/control, session.detail, session.snapshot, and knowledge if produced.
- `fin ledger check --strict` passes for both ledgers.
- Old projection readers still work: status probe, Web debug session messages, QQBot-compatible conversation path.

### 5. Disconnect / Reconnect
- Stop one explicit project-agent PID/service only.
- System agent records connection lost/unavailable state.
- Dispatch while disconnected returns structured unavailable error; no silent fallback.
- Restart project agent from durable config.
- System agent observes reconnect.
- Mailbox seq and ledger seq remain monotonic.
- Pending or retried task reaches terminal success/failure with evidence.

### 6. Execution Error
- Force a project-agent tool/provider error.
- Error is recorded in progress, steps/tools/provider/control, and result refs.
- System receives structured failure, not a swallowed or fabricated success.
- `fin ledger check --strict` still passes.

### 7. Subagent Boundary
- Project agent may spawn a local subagent to execute a bounded slice.
- Subagent is not globally discoverable by system agent.
- System only observes parent-visible progress/result refs from project agent.

## Implementation Tasks

1. Add test harness fixtures for isolated runtime homes, disposable sessions, explicit PID/service tracking, and cleanup.
2. Add scenario definitions for startup, discovery/auth, dispatch, progress/result, disconnect/reconnect, execution error, and subagent boundary.
3. Add failing tests first, with assertions tied to runtime events, ledgers, and compatibility projections.
4. Implement missing system/project agent startup config if needed: default system path, dynamic project config, cwd/port persistence.
5. Implement or wire authenticated agent-to-agent discovery and mailbox/control-plane communication.
6. Implement task dispatch and progress/result propagation through runtime truth.
7. Ensure lifecycle events are written into ledger tracks and projections without creating a second fact source.
8. Add CLI or harness command to run the local two-agent lifecycle test end-to-end.
9. Add cleanup that only stops explicit PIDs/services created by the harness.
10. Run full verification gates and record receipts.

## Verification Gates

### Required Automated Gates
- Focused local lifecycle harness tests.
- Auth failure and unavailable-agent fault tests.
- Disconnect/reconnect recovery tests.
- Execution error propagation tests.
- Ledger strict checks for system and project ledgers.
- Compatibility reader checks for status probe/Web debug/QQBot paths.
- `cargo test --manifest-path rust/Cargo.toml -p fin-contracts -p fin-runtime -p fin-debug-server -p fin-cli`.
- `git diff --check`.

### Required Real Local Run
Run two local instances in isolated runtime homes:
1. Start system agent.
2. Start project agent from dynamic config.
3. Discover project agent.
4. Dispatch task.
5. Observe progress/tool turn.
6. Return result refs.
7. Force disconnect and reconnect.
8. Force execution error.
9. Run strict ledger checks.
10. Stop only harness-owned explicit PIDs/services.

### Android / Java
Run Android build/test only if Java is available. If Java is missing, record exact environment blocker and do not claim Android verification.

## Definition Of Done
- Test scenarios are documented before implementation.
- Harness tests exist for normal lifecycle and failure lifecycle.
- Real local two-instance run completes and produces receipts.
- Both system and project ledgers pass strict checks.
- Compatibility readers still pass.
- No fallback, hidden second truth source, broad kill, or UI-side state inference is introduced.
- Summary explains exactly which lifecycle scenarios passed and which evidence proves them.

## Unique Implementation Rationale
The correct implementation point is the runtime agent control plane plus ledger/session materialization boundary. Runtime owns durable identity, listener state, mailbox, task dispatch, progress events, and ledger truth. UI/QQBot/Web/Android must only consume projections/snapshots. Implementing lifecycle semantics in UI adapters or test-only shims would create a second fact source and would not prove real cross-agent behavior.

## Verified Evidence 2026-05-23
- Focused lifecycle tests passed: `cargo test --manifest-path rust/Cargo.toml -p fin-cli local_multi_agent -- --nocapture`.
- Focused headless parity tests passed: `cargo test --manifest-path rust/Cargo.toml -p fin-cli project_agent_harness -- --nocapture`.
- Full Rust gate passed: `cargo test --manifest-path rust/Cargo.toml -p fin-contracts -p fin-runtime -p fin-debug-server -p fin-cli`.
- Whitespace gate passed: `git diff --check`.
- Real local two-instance harness passed with isolated runtime `/tmp/fin-local-agent-e2e.p3iXG4/runtime`.
- Receipt evidence: `system_pid=19913`, `project_pid=19938`, `headless_parity_ok=true`, auth success/failure recorded, dispatch/progress/result recorded, disconnect/reconnect recorded, forced execution error recorded, project-local subagent hidden from system, scoped cleanup true.
- Result refs returned to system: `ledger://project-fin-agent/tracks/session.snapshot#session-snapshot-7`, `child-node://project-fin-agent/result/task-local-multi-agent`.
- Strict ledger checks passed: `system-agent` ok with 9 timeline records; `project-fin-agent` ok with 9 timeline records.

## Verified Evidence 2026-05-23 Second Audit
- Strengthened local harness evidence after completion audit: project endpoint now persists a real auto-selected local port instead of `127.0.0.1:0`; harness nodes are spawned through the actual `fin-cli local-multi-agent-node` command path rather than a shell-only shim.
- Focused tests passed after strengthening: `cargo build --manifest-path rust/Cargo.toml -p fin-cli`; `cargo test --manifest-path rust/Cargo.toml -p fin-cli local_multi_agent -- --nocapture`; `cargo test --manifest-path rust/Cargo.toml -p fin-cli project_agent_harness -- --nocapture`.
- Real local two-instance run passed at `/tmp/fin-local-agent-e2e.s8zQP3` with `system_pid=54944`, `project_pid=54945`, `project_endpoint=127.0.0.1:59972`, and `project_port_persisted=true`.
- Receipt proves normal/fault/boundary paths: `auth_success=true`, `auth_failure_recorded=true`, `dispatch_recorded=true`, `progress_event_count=3`, result refs returned, `disconnect_recorded=true`, `reconnect_recorded=true`, `execution_error_recorded=true`, `subagent_hidden_from_system=true`, `cleanup_scoped=true`.
- Receipt proves ordering/compatibility: `mailbox_seq_monotonic=true`, `ledger_seq_monotonic=true`, `compatibility_projection_ok=true`, compatibility readers `status_probe`, `web_debug_session_messages`, and `qqbot_conversations` all verified.
- Strict ledger checks passed again: `system-agent` ok with 9 timeline records; `project-fin-agent` ok with 9 timeline records.
