# MEMORY — Long-Term Memory

## Project Overview
- fin 的执行真源在 Rust runtime；Web 只做观察与调试。
- 基础事实模型固定为 `operation -> event -> projection/debug view`。
- 当前 M1 优先级：最小可闭合推理 + 最小可观测 Web debug，而不是复杂自治。

## Key Decisions
- [2026-04-17] Provider debug 通过结构化 event 暴露 sanitized `user_agent` 与 `request_headers`；Web/Projection 只能消费，不复制 provider 语义。
- [2026-04-17] Context snapshot 采用 bounded write：`runtime/current/current_context.json` 只保留最新，session 内 `context/recent_contexts.json` 只保留最近窗口（当前 8 条）。禁止每轮散写无界快照。
- [2026-04-17] 生命周期控制优先：M1 `web-debug` 保持前台阻塞命令，不引入 detached daemon / orphan process。
- [2026-04-17] 真实 provider 回归必须使用隔离 test runtime home 与 test session namespace，不能污染正常 `~/.fin`。
- [2026-04-17] 正式 build/安装/提升必须走统一 build flow；自动回归与自动 build version bump 应挂在统一入口，而不是裸 `cargo build`。
- [2026-04-17] 产品 build version 与 Cargo crate semver 分层；未来预留 core version + module version map 的模块化升级路径。

- [2026-04-17] install-dev 已实现自动 build version：隔离验证中已确认 `0.1.0001 -> 0.1.0002` 自动递增，并维护 `current/previous`。
- [2026-04-17] 网络型 provider 回归允许显式 bounded retry（当前 3 次），但 retry 过程必须写日志，不能静默 fallback。

- [2026-04-17] CLI 已补齐 `build-dev / promote / rollback` 入口；`install-dev` 暂保留为兼容别名。
- [2026-04-17] `transcript-demo` 已形成真实多轮闭环：recent digest/context 不仅落盘，还会编入 provider 实际请求；Web 可通过 `recent_contexts.json` 观察每轮 context 结构。

## Patterns & Learnings
- 大的 CLI/核心入口在行为稳定后要尽快拆回模块边界；门禁白名单只能临时存在，验证通过后应回收。
- Provider 请求调试信息进入 event 时必须脱敏；允许暴露 header 名称和安全值，禁止泄漏 API key/token/cookie。
- 需要直观 debug 时，优先把字段做成结构化 projection + current artifact，而不是靠 grep 巨量日志。
- 多轮闭环调试优先看 `current_context.json + recent_contexts.json + last_run.json` 的组合，而不是猜模型是否记住历史。
- snapshot 设计先考虑 CPU / 内存 / IO 边界，再决定写入粒度；latest overwrite + bounded recent window 是当前默认模式。

## Technical Context
- Projection 当前已包含：`latest_provider_activity` / `latest_provider_user_agent` / `latest_provider_header_names`。
- Runtime 当前已落盘：`current_context.json`、`recent_contexts.json`、`last_run.json`、`current_projection.json`、`latest_events.jsonl`。
- Web debug 当前展示：Current Projection / Provider Debug / Current Context Snapshot / Last Run / Event Stream。

## Verified Evidence
- [2026-04-17] `cargo test --workspace` 通过。
- [2026-04-17] 使用真实 anthropic-compatible provider 的隔离 `runtime-demo` 返回 `OK`，并验证了新的 context/projection/provider debug artifacts。
