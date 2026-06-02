# Live Provider Write E2E
Purpose: Validate end-to-end live provider write flows.
Status: Completed.

## A. Design Differences: Codex CLI vs Hermes Agent
1. **Interaction & Gateway Model**: Codex is primarily a local TUI/CLI-first agent with a rich, locally cached history search in its `tui-chat-composer`. Hermes operates as a multi-channel gateway orchestrator (Slack, Telegram, Discord, Signal, Matrix) with platform-native approval buttons and rich formatting.
2. **Execution Lifecycle & Timeouts**: Codex relies on explicit thread persistence in `~/.codex/sessions`. Hermes introduces activity-based timeouts instead of wall-clock limits and supports `notify_on_complete` for background processes, allowing the agent to remain idle without killing active work.
3. **Extensibility & Routing**: Hermes features a dynamic `/model` command for live provider switching and a formalized plugin system with session lifecycle hooks (`finalize`/`reset`). Codex emphasizes strict agent security boundaries and SDK embedding for workflow integration.

## B. Key Learnings for Fin Framework
1. **Activity-Aware Timeouts**: Implement tool-activity tracking instead of wall-clock timeouts to prevent premature termination of long-running, healthy tasks.
2. **Async Background Decoupling**: Adopt `notify_on_complete` patterns for external commands, freeing the main reasoning loop to handle other tasks while waiting for I/O or CI.
3. **Lifecycle-Driven Plugins**: Add session-level lifecycle hooks (e.g., `finalize`, `reset`) to the plugin architecture, enabling extensions to manage state and cleanup deterministically across reasoning cycles.