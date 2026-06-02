# fin-5.1 Resumable Pause/Resume Receipt — 2026-04-20

本文档记录 `fin-5.1` 当前实现轮次的收口证据。

它只回答四件事：

1. 本轮到底补了什么
2. 真正冻结的恢复边界是什么
3. 证据是什么
4. 还没有做什么

---

## 1. 本轮补齐内容

本轮把旧的“重新起一个 closure 继续跑”的假恢复，推进成 **runtime-owned execution checkpoint + scheduler resume_checkpoint** 的真实恢复链。

当前新增/收口的关键点：

1. `ExecutionCheckpointRecord` 成为新的 durable truth
2. runtime 在受控 resume anchor 记录 checkpoint
3. scheduler 在 reminder / attached control-plane follow-up 场景下优先消费 checkpoint，而不是只走 generic pending queue
4. checkpoint consume 会写回 durable truth，并记录 `execution.checkpoint_consumed`
5. framework synthetic resume input 不再污染用户会话渲染

---

## 2. 冻结的恢复边界

本轮**明确支持**的恢复边界：

### 2.1 `wait.remind -> waiting_external`

当 closure 因 `wait.remind` 进入 `waiting_external`：

- runtime 记录 open checkpoint
- control plane 标记 `resume_checkpoint_ready=true`
- reminder 到期后 scheduler 优先执行 `resume_checkpoint`
- checkpoint 被消费后写入 consumed state

### 2.2 tool-followup resume boundary

当工具执行完成，但 closure 还未真正 stop，需要继续下一轮 provider：

- runtime 在工具回注后的 framework-owned 边界记录 checkpoint
- 下次续跑时从 checkpoint 生成 follow-up input
- 不再把“再开一个 closure”冒充成精确恢复

### 2.3 conversation truth boundary

framework synthetic resume input：

- **允许进入** turn / step / provider / debug artifacts
- **禁止进入** user-facing `conversation/messages.json`

这样保持：

1. runtime/full truth 完整
2. 用户会话渲染不被 framework prompt 污染

---

## 3. 主要代码落点

### contracts
- `rust/crates/contracts/src/records.rs`
- `rust/crates/contracts/src/lib.rs`

### runtime
- `rust/crates/runtime/src/lib.rs`
- `rust/crates/runtime/src/closure_runtime.rs`
- `rust/crates/runtime/src/closure_runtime_checkpoint.rs`
- `rust/crates/runtime/src/closure_runtime_finalize.rs`
- `rust/crates/runtime/src/closure_runtime_state.rs`
- `rust/crates/runtime/src/control_plane.rs`
- `rust/crates/runtime/src/session_materializer.rs`
- `rust/crates/runtime/src/scheduler.rs`
- `rust/crates/runtime/src/execution_checkpoint_tests.rs`

### cli
- `rust/crates/cli/src/demo.rs`
- `rust/crates/cli/src/execution_checkpoint.rs`
- `rust/crates/cli/src/scheduler_driver.rs`
- `rust/crates/cli/src/scheduler_tick.rs`
- `rust/crates/cli/src/web_debug_tests_runtime_followups.rs`
- `rust/crates/cli/src/project_runtime_resume_tests.rs`

---

## 4. 验证证据

已执行并通过：

```bash
cargo fmt --all --manifest-path rust/Cargo.toml
```

```bash
python3 scripts/check-code-line-limit.py
```

结果：

- `code line-limit ok (limit=500, whitelist_entries=1)`
- `rust/crates/runtime/src/closure_runtime.rs` 当前为 **500 行**

```bash
cargo test -p fin-runtime -p fin-cli --manifest-path rust/Cargo.toml
```

结果：

- `fin-cli`: **109 passed**
- `fin-runtime`: **72 passed**

关键 E2E / orchestration 证据：

```bash
cargo test -p fin-cli --manifest-path rust/Cargo.toml 'web_debug::web_debug_tests::web_debug_tests_runtime::web_debug_tests_runtime_followups::due_reminder_prefers_execution_checkpoint_resume_without_fake_user_message' -- --exact --nocapture
```

验证点：

- waiting_external 生成 checkpoint
- reminder fired 后 scheduler 优先走 `resume_checkpoint`
- synthetic resume input 不进入会话消息
- `execution.checkpoint_consumed` durable event 存在

```bash
cargo test -p fin-cli --manifest-path rust/Cargo.toml 'channel_peer_qqbot_bridge::e2e_tests::qqbot_inbound_message_runs_end_to_end_and_emits_reply_from_session_truth' -- --exact --nocapture
```

验证点：

- inbound channel message 进入 session truth
- runtime inference 闭环
- reply 从 session truth 发出

---

## 5. 本轮不宣称完成的部分

以下内容**不在本轮完成口径内**：

1. provider mid-flight stack restore
2. detached / always-on runtime-owned background recovery
3. 普通用户输入的真正并行推理
4. product-grade session/task/topic 正式状态机

因此本轮结论是：

> `fin-5.1` 已完成当前最小真实恢复切口：checkpoint-based precise recovery at framework-owned boundaries。

但不应被误解为“所有中断点都已支持任意精确恢复”。
