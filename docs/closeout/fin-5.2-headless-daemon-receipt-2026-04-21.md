# fin-5.2 Headless Daemon Receipt — 2026-04-21

本文档记录 `fin-5.2` 当前轮次的收口证据。

目标只回答四件事：

1. 本轮新增了什么
2. headless/detached 的最小边界是什么
3. 证据是什么
4. 哪些事情仍未宣称完成

---

## 1. 本轮新增内容

本轮把原来只能依赖 attached/frontstage 请求链推进的 continuation，补成了 **single-agent headless daemon** 的最小闭环。

新增入口：

- `fin start <user.toml>`
- `fin stop <user.toml>`
- `fin daemon-run <user.toml>`（内部 headless loop entry）

新增实现：

1. detached/headless daemon 生命周期入口
2. pid/lease/stop-request/state/recovery 五类 framework-owned truth
3. session truth 驱动的 sessions-with-work 发现
4. headless 模式直接驱动 `supervisor cycle`，不再依赖前台 UI 请求来顺手 tick
5. due reminder + execution checkpoint 可在无前台请求时继续推进

---

## 2. 冻结的最小边界

### 2.1 生命周期真源

本轮冻结以下文件为最小生命周期真源：

- `runtime/pids/headless-daemon.pid`
- `runtime/leases/headless-daemon.json`
- `runtime/current/current_daemon_state.json`
- `runtime/current/current_daemon_recovery_action.json`
- `runtime/locks/headless-daemon.stop`

含义：

- pid = 当前 daemon 进程标识
- lease = 最近心跳与活动 session 摘要
- daemon_state = 当前生命周期/监督状态
- recovery_action = 当前恢复意图摘要
- stop_request = graceful stop 请求

### 2.2 执行边界

当前 headless daemon：

- 从 session truth 发现 sessions-with-work
- 对每个有工作的 session 直接运行 framework-owned `supervisor cycle`
- 继续复用同一 runtime 主链
- 根据 session context truth 推断当前 role：
  - entry/system session -> `system`
  - project session -> `project`

### 2.3 本轮保证的自治能力

本轮保证：

1. waiting session 可以在 reminder 到期后继续推进
2. execution checkpoint 可以在 headless 模式下被恢复并消费
3. 无前台 Web / qqbot 请求时，单 agent 仍能继续执行 reminder/checkpoint follow-up

---

## 3. 主要代码落点

- `rust/crates/cli/src/cli.rs`
- `rust/crates/cli/src/command.rs`
- `rust/crates/cli/src/error.rs`
- `rust/crates/cli/src/lib.rs`
- `rust/crates/cli/src/headless_daemon.rs`
- `rust/crates/cli/src/headless_daemon_support.rs`
- `rust/crates/cli/src/headless_daemon_tests.rs`
- `rust/crates/cli/src/tests.rs`
- `rust/crates/cli/src/web_debug_turns.rs`

---

## 4. 验证证据

已执行并通过：

```bash
cargo fmt --all --manifest-path rust/Cargo.toml
```

```bash
python3 scripts/check-code-line-limit.py
```

```bash
cargo test -p fin-cli -p fin-runtime --manifest-path rust/Cargo.toml
```

关键 E2E：

```bash
cargo test -p fin-cli --manifest-path rust/Cargo.toml 'headless_daemon_tests::headless_daemon_cycle_resumes_checkpoint_without_frontstage' -- --exact --nocapture
```

验证点：

- headless daemon 发现 session work
- due reminder 自动注入
- supervisor cycle 在无前台请求时直接推进
- execution checkpoint 被消费
- synthetic resume prompt 不污染用户会话消息
- daemon lease/state truth 成功写入

附加回归：

```bash
cargo test -p fin-cli --manifest-path rust/Cargo.toml 'channel_peer_qqbot_bridge::e2e_tests::qqbot_inbound_message_runs_end_to_end_and_emits_reply_from_session_truth' -- --exact --nocapture
```

用于确保本轮没有破坏现有真实 channel 最小闭环。

---

## 5. 本轮不宣称完成的部分

以下内容不在本轮完成口径内：

1. 外部 service manager / launchd 集成
2. 多 daemon / 多 supervisor 选主
3. detached daemon 的跨机 supervision
4. 普通用户输入真正并行推理
5. product-grade session/task/topic 状态机

因此本轮结论是：

> `fin-5.2` 已完成 single-agent always-on/headless 的最小闭环：detached daemon + framework-owned lifecycle truth + headless supervisor continuation。
