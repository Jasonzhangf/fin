# Multi-Agent E2E Architecture: Codex vs Hermes-Agent & Fin Learnings

## 1. Multi-Agent Design Differences

| Dimension | Codex (Centralized/Hierarchical) | Hermes-Agent (Decentralized/Event-Driven) |
|-----------|----------------------------------|-------------------------------------------|
| **Orchestration** | Top-down planner-executor pattern. Single orchestrator decomposes tasks and assigns strict workflows to sub-agents. | Peer-to-peer actor model. Agents self-organize via decentralized task boards and negotiate execution roles dynamically. |
| **Communication** | Synchronous, tool-call bound. Inter-agent data flows via direct function calls or shared scratchpad with strict I/O contracts. | Asynchronous, mailbox/event-bus driven. Messages are routed via topic-based pub/sub, enabling loose coupling and non-blocking handoffs. |
| **State Isolation** | Strict sandboxing per agent. State is scoped to execution context and cleared post-run; global state is read-only. | Shared context graph with transactional isolation. Agents write to local slots, with explicit commit/rollback mechanisms for global sync. |
| **Role Routing** | Static or rule-based dispatch. Router matches task type to pre-registered agent capabilities with deterministic alternate chains. | Dynamic capability negotiation. Agents broadcast readiness, bid on tasks, or auto-escalate to supervisor nodes based on runtime metrics. |
| **Fault Tolerance** | Retry-with-state-reset or full workflow restart. Relies on idempotent tool executions and validator gates. | Circuit-breaker patterns, graceful degradation, and checkpoint recovery. Failed agents shed load; peers assume abandoned work automatically. |

## 2. Actionable Learnings for `fin`

1. **Hybrid Dispatch Architecture**: Combine Codex-style deterministic task decomposition with Hermes-style dynamic peer routing. Use a lightweight frontstage dispatcher to parse intents, then route to specialized backend workers based on real-time capacity & capability matching.
2. **Structured Async Mailbox**: Decouple agent coordination from synchronous tool calls. Implement a framework-managed mailbox for task claims, progress updates, and escalation signals. This enables resilient retries and multi-turn workflows without blocking the event loop.
3. **Explicit State Checkpointing & Isolation**: Enforce strict boundaries between agent local memory and shared session truth. Adopt transactional snapshots for critical state transitions, enabling instant recovery from crashes or context evictions.
4. **Dynamic Role Escalation & Handoff**: Instead of hard-coded failure paths, allow agents to negotiate handoffs. If an agent's confidence drops below threshold or a tool fails repeatedly, route the payload to a peer or supervisor agent automatically, preserving continuity.
5. **Unified Observation & Replay Telemetry**: Standardize all agent actions (reasoning, tool I/O, state diffs, messages) as durable, timestamped artifacts. This creates a built-in debug timeline and enables deterministic replay for auditing and continuous improvement.

> *Next Step: Implement the mailbox routing and checkpoint primitives in `fin/rust/runtime` to validate E2E resilience.*
