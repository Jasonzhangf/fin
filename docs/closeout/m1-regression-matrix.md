# M1 Regression Matrix

本文档列出当前 M1 收口所需的最小回归矩阵，并标明：

- 已有自动化覆盖
- 仍需补的自动化
- 建议手工验证项

## 1. 当前统一自动测试入口

当前主回归命令：

```bash
cargo test -p fin-runtime -p fin-cli -p fin-debug-server --manifest-path rust/Cargo.toml
```

最近已通过的基线结果：

- `fin-runtime`
- `fin-cli`
- `fin-debug-server`

## 2. 回归矩阵总表

| 能力组 | 目标 | 当前自动化覆盖 | 当前状态 | 手工验证建议 |
| --- | --- | --- | --- | --- |
| Provider smoke | provider 基本请求/响应、错误路径、协议装配 | `scripts/probe-anthropic-provider.py`；runtime/provider 相关测试；`model_output_tests` | 部分自动化 + 需手工 smoke | 用隔离 test config 验证 anthropic-wire 能真实返回 |
| 多轮推理 | 同一 session 连续多轮 history/context 生效 | `runtime::tests`、`model_output_tests`、`session_materializer` 相关测试 | 已覆盖核心路径 | 用 transcript-demo / web debug 手工看 recent contexts |
| Auto tool loop | 同一 turn 内多轮 tool->再推理->再 tool | `tool_dispatch_tests`、`runtime/src/tests.rs`、`model_output_tests` | 已覆盖核心路径 | 验证 tool round 上限与错误路径在 Web 可见 |
| Queue / interrupt / resume-run | pending 输入、resume、segment merge、execution state | `scheduler_driver_tests`、`scheduler_tick_tests`、`runtime::control_plane` 相关测试 | 已覆盖 M1 范围 | 手工验证 `/resume-run` 不丢 state |
| Wait / reminder | 超时等待、due reminder 自动恢复 | `web_debug::tests::due_reminder_auto_ticks_scheduler_and_drains_pending_queue`、`scheduler_tick_tests` | 已覆盖核心路径 | 手工验证等待后自动注入提醒 |
| Compact rebuild | `/compact` 走 rebuild，不走模型压缩 | session/history/context rebuild 相关测试，`session_materializer` / `turn_records` | 基础覆盖已具备 | 手工验证 compact 后 context 结构仍完整 |
| Status probe | 非中断状态查询，不产生新 closure | `web_debug::tests::status_probe_returns_latest_framework_state_without_new_closure` | 已覆盖 | 手工验证 `/status` 时消息与 digest hash 不变 |
| Event archive / control plane | archive、tick、supervisor、heartbeat、daemon state 可读 | `supervisor_cycle_tests`、`supervisor_heartbeat_tests`、`daemon_state_tests`、`scheduler_tick_tests` | 已覆盖核心路径 | 在 4040 上检查 inspector/status 显示最新控制面 truth |
| Web debug truth | conversation/debug 同源，读取 session truth | `fin-debug-server` tests、`session_view` / `chat_api` tests | 已覆盖基础 API | 手工验证 UI 刷新来自 session 文件而非自造状态 |
| Installed-binary smoke | 安装态读取配置、写 runtime home、基础命令可跑 | `install-dev` 已真实跑通；已有 summary receipt + `receipt-index.json` | 已通过 | 后续继续把 receipt 生成固化到默认流程 |

## 3. 当前重点已存在测试

### 3.1 control plane / scheduler / supervisor

- `scheduler_driver_tests::*`
- `scheduler_tick_tests::*`
- `supervisor_cycle_tests::*`
- `supervisor_heartbeat_tests::*`
- `daemon_state_tests::*`

### 3.2 Web debug / status / reminder

- `web_debug::tests::tick_command_drives_pending_queue_when_scheduler_allows`
- `web_debug::tests::due_reminder_auto_ticks_scheduler_and_drains_pending_queue`
- `web_debug::tests::status_probe_returns_latest_framework_state_without_new_closure`

### 3.3 runtime / context / tool dispatch

- `runtime/src/model_output_tests.rs`
- `runtime/src/tool_dispatch_tests.rs`
- `runtime/src/tests.rs`
- `runtime/src/session_materializer.rs` 关联测试

## 4. 仍需明确补齐的最小缺口

以下项目若要宣称 M1 fully closed，当前还需要继续补“自动化门禁”而不是功能证据：

1. **installed-binary smoke / install receipts 标准化**
   - 当前 `install-dev` 已真实跑通，但 receipt 组织仍可继续固化

## 5. closeout 阶段建议的最小手工验证脚本

### 5.1 status probe

目标：

- `POST /api/chat/send {"message":"/status"}` 返回 `response_kind=status_probe`
- `events_count=0`
- session `messages.json` 与 `recent_digests.json` 不新增 closure

### 5.2 multi-turn + tool loop

目标：

- 同一 session 至少两轮连续对话
- 第二轮 provider request 中能看到第一轮 recent history/context
- tool call / tool result / assistant summary 都写入 session artifacts

### 5.3 reminder auto resume

目标：

- 发起 wait/reminder
- due 后自动进入 tick pipeline
- `latest_tick.json` / `latest_supervisor_cycle.json` / `latest_heartbeat.json` 能反映这次推进

### 5.4 installed-binary smoke

目标：

- 从 `~/.fin/bin/fin` 启动
- 读取隔离 test config
- 能写入隔离 runtime home / sessions / logs

## 6. closeout 判定

当以下条件成立时，可宣称 M1 回归矩阵基本收口：

1. 当前 cargo test 基线持续为绿
2. provider / compact / 4040 / installed-binary 的 receipts 都存在
3. 当前自动化缺口已不再阻塞正式 build/install gate，剩余工作主要是 receipts 标准化
