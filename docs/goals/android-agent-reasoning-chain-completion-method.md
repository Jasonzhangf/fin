# Android Agent 推理链路 Codex 对齐完成方式

日期：2026-05-22
仓库：`/Users/fanzhang/code/fin`
基线参考：`/Users/fanzhang/code/codex`
关联计划：`docs/goals/android-agent-reasoning-chain-codex-alignment-plan.md`

## 1. 目标

把 fin 的 Android Agent 推理链路做成可持续升级、可回归、可在线验证的稳定实现：

1. 本地 runtime/provider/tool 推理链路真实跑通。
2. 每个 turn 的过程消息被结构化消费，而不是只展示最终文案。
3. Android 只是控制界面和事件投影壳，不拥有业务推理语义。
4. 后端事件是唯一真源，UI 不猜字段、不用 `tool` / `unknown` 这类无信息占位。
5. 每次 build 自动运行回归矩阵，验证推理链路而不只是验证 APK 能编译。

## 2. 当前已知问题与根因

### 2.1 表面现象

- Android 已能连接 WS，但 UI 仍出现 `tool · running`、`tool · failed · unknown`。
- 这说明连接链路已不是主问题；真正问题是 turn 过程事件 schema 与前端消费不一致。

### 2.2 根因

后端 `rust/crates/debug-server/src/mobile_ws.rs` 当前发送的 `turn.tool_event` 结构类似：

```json
{
  "type": "turn.tool_event",
  "tool_record": {
    "tool_name": "...",
    "status": "...",
    "error_summary": "..."
  }
}
```

但 Android `android-client/app/src/main/assets/mobile-shell.html` 读取的是顶层字段：

```js
m.tool_name
m.status
m.error_summary
```

因此字段读不到，前端用无意义占位显示成 `tool` / `unknown`。

### 2.3 不能接受的修复

以下都不是完成：

- 只把 UI 文案从 `tool` 改成别的默认词。
- 在前端继续猜 `m.tool_record?.tool_name || m.tool_name || 'tool'`。
- 只让 APK build 成功。
- 只让 WebSocket 显示 connected。
- 只让最终 assistant 文本显示出来，过程消息仍未消费。

真正修复必须建立唯一事件契约，然后后端、Android、E2E、CI 全部消费同一契约。

## 3. Codex 基线差异

参考文件：

- `~/code/codex/codex-rs/protocol/src/models.rs`
- `~/code/codex/codex-rs/protocol/src/protocol.rs`

Codex 的核心做法：

1. **强类型 item lifecycle**：turn 内每个 message/reasoning/tool/exec 都有明确 item。
2. **开始、增量、完成、失败分离**：不是自由 JSON 拼字段。
3. **稳定关联键**：`turn_id`、`item_id`、`call_id` 用于把 delta/end/error 归并到同一个 item。
4. **事件即事实**：UI/日志只是投影，不能补业务真相。
5. **工具调用不叫抽象 tool**：必须有具体名称、参数摘要、输出摘要、错误摘要、耗时、状态。

fin 当前差异：

| 维度 | Codex 基线 | fin 当前问题 | 完成要求 |
|---|---|---|---|
| turn 生命周期 | `turn.started` / `turn.completed` | 部分事件自由散落 | 建立统一 turn lifecycle |
| item 生命周期 | `item.started` / delta / completed / failed | `turn.tool_event` schema 不统一 | 所有过程事件映射为 `turn.item.*` |
| 字段位置 | 类型稳定 | nested/top-level 混用 | 后端只发一个权威 schema |
| UI 责任 | 只投影 event | 前端猜字段和占位 | 前端只消费统一 event，缺字段暴露 schema_error |
| 回归 | 协议/运行链路验证 | build 偏多 | build 时跑事件契约 + E2E 矩阵 |

## 4. 唯一事件契约

### 4.1 必须支持的事件类型

```text
runtime.health
provider.health
turn.started
turn.item.started
turn.item.delta
turn.item.completed
turn.item.failed
turn.completed
```

允许短期兼容旧事件：

```text
turn.progress
turn.tool_event
turn.error_event
turn.trace_event
turn.rendered
```

但旧事件只能由统一 item mapper 派生，不得成为第二真源。

### 4.2 `turn.item.started`

```json
{
  "type": "turn.item.started",
  "session_id": "...",
  "client_message_id": "...",
  "turn_id": "...",
  "item_id": "...",
  "item_kind": "reasoning|tool|exec|provider|message|error",
  "label": "provider.call",
  "title": "调用模型",
  "purpose": "dispatch compiled prompt to provider",
  "status": "running",
  "started_at": "2026-05-22T00:00:00Z"
}
```

硬要求：

- `item_id` 必须稳定，完成/失败事件必须能按它归并。
- `label` 禁止为空、`tool`、`unknown`。
- `title` 必须是可读的具体动作标题。
- `purpose` 必须说明该步骤为什么存在。

### 4.3 `turn.item.completed`

```json
{
  "type": "turn.item.completed",
  "session_id": "...",
  "client_message_id": "...",
  "turn_id": "...",
  "item_id": "...",
  "item_kind": "tool",
  "label": "shell.exec",
  "title": "执行本地命令",
  "purpose": "run requested local check",
  "status": "completed",
  "duration_ms": 123,
  "output_summary": "exit=0, stdout=...",
  "error_summary": null
}
```

### 4.4 `turn.item.failed`

```json
{
  "type": "turn.item.failed",
  "session_id": "...",
  "client_message_id": "...",
  "turn_id": "...",
  "item_id": "...",
  "item_kind": "provider",
  "label": "provider.mimo.route",
  "title": "模型路由失败",
  "purpose": "dispatch request to configured provider",
  "status": "failed",
  "duration_ms": 456,
  "output_summary": null,
  "error_summary": "503: no available intranet node"
}
```

硬要求：

- 失败必须保留完整 `error_summary`，不能吞成 `unknown`。
- provider/routing/auth 类错误还必须同步投影 `provider.health`。

### 4.5 health 事件

```json
{
  "type": "runtime.health",
  "status": "available|unavailable|degraded",
  "reason": null
}
```

```json
{
  "type": "provider.health",
  "provider": "mimo",
  "status": "available|unavailable|auth_failed|route_unavailable",
  "reason": "..."
}
```

WebSocket 连接状态、runtime 状态、provider 状态必须分开显示。

## 5. 文件级完成方式

### 5.1 后端：统一 mapper

目标文件：

- `rust/crates/debug-server/src/mobile_ws.rs`
- 如需下沉类型：`rust/crates/contracts/src/records.rs`

实施：

1. 定义一个后端唯一 mapper：从 `ToolExecutionRecord` / `ReasoningViewRecord` / `StepRecord` 映射为 `turn.item.*`。
2. `mobile_ws.rs` 发送过程事件时只能调用这个 mapper。
3. 修复 `tool_record` nested 与前端顶层读取不一致问题：新前端只读 `turn.item.*`，旧 `turn.tool_event` 如保留也必须由同一 item 生成。
4. 为 provider route/auth/503 错误映射 `provider.health`。
5. 禁止吞异常；字段缺失在事件中明确暴露，不在 UI 私自补语义。

唯一性理由：

- 真源错误发生在后端事件契约不统一，不在 Android 视觉层。
- 只改 UI 会继续保留双 schema；后续新增工具仍会复发。
- 只改 E2E 不修复 runtime 事件无法让真实手机端消费过程消息。
- 因此唯一正确修改点是：后端统一 item lifecycle mapper + 前端只消费该契约。

### 5.2 Android：只消费 item lifecycle

目标文件：

- `android-client/app/src/main/assets/mobile-shell.html`
- 必要时：`android-client/app/src/main/java/com/fin/client/model/Models.kt`

实施：

1. `onWs` 增加 `turn.item.started|delta|completed|failed` 分支。
2. 用 `item_id` 聚合 item，不按 label/title 猜测。
3. 渲染字段：`title`、`purpose`、`status`、`duration_ms`、`output_summary`、`error_summary`。
4. 若必需字段缺失，显示 `schema_error:<field>`。
5. 禁止默认显示 `tool`、`unknown`、空标题。
6. WS/runtime/provider 三层状态独立渲染。

唯一性理由：

- Android 是投影壳，不能继续拥有 schema 猜测逻辑。
- 缺字段显示 schema_error 能把后端契约问题暴露到测试和现场，而不是隐藏。

### 5.3 回归：build 自动验证推理链路

目标文件：

- `scripts/regression/run_android_client_matrix.sh`
- `android-client/scripts/smoke/ws-event-contract-smoke.mjs`
- `scripts/android-mvp/run_turn_channel_e2e.py`
- 可新增：`android-client/scripts/smoke/projection-contract-check.mjs`
- `.github/workflows/ci.yml`

实施：

1. build 矩阵保留 Android unit test 与 assembleDebug。
2. 增加 WS event contract smoke。
3. 增加 turn channel E2E。
4. 增加 projection contract check，断言：
   - `turn.item.started` 数量 > 0。
   - 每个 started 都有 completed 或 failed。
   - `item_id` 无缺失且可归并。
   - `label/title/purpose` 不为空，不等于 `tool` / `unknown`。
   - failed item 有完整 `error_summary`。
   - rendered turn 包含最终 assistant response 与过程 item refs。
5. 矩阵失败时 build 失败，不允许只警告。

唯一性理由：

- 用户关注的是“正确推理链路完成、turn 推理过程消息被消费”，因此 build 本身只是必要非充分条件。
- 回归矩阵必须把过程事件契约作为 build gate，才能防止 UI 文案假通过。

### 5.4 真实 provider / mimo 验证

配置：

- `~/.fin/config/user.toml` 中 `default_provider = "mimo"`。

必须跑：

```bash
/Users/fanzhang/.fin/bin/fin provider-live-smoke /Users/fanzhang/.fin/config/user.toml
python3 /Users/fanzhang/code/fin/scripts/android-mvp/run_turn_channel_e2e.py
./scripts/regression/run_android_client_matrix.sh
```

要求：

- provider live smoke 成功。
- E2E 收到 `turn.item.*`，并验证 started/completed/failed 成对。
- matrix 全绿。

### 5.5 手机端在线验收

设备：

```bash
adb connect 100.127.23.27:1234
adb devices
```

构建安装：

```bash
cd /Users/fanzhang/code/fin/android-client
./gradlew assembleDebug --no-daemon
adb install -r app/build/outputs/apk/debug/app-debug.apk
```

证据采集：

```bash
adb shell run-as com.fin.client tail -120 files/logs/connection-events.log
adb logcat -d | grep -iE 'fin|websocket|turn.item|provider.health|runtime.health' | tail -200
adb exec-out screencap -p > reports/android-client-device-e2e-$(date +%Y%m%d-%H%M%S).png
```

验收动作：

1. 正常推理请求：确认 UI 展示具体过程 item，不出现 `tool` / `unknown`。
2. 错误工具/错误 provider 请求：确认失败 item 展示具体 label/title/error_summary。
3. 断网或 provider route 错误：确认 WS 状态不被误判为 provider 状态，`provider.health` 显示明确失败原因。

## 6. 测试矩阵

### L0：单元与 mapper

```bash
cargo test -p fin-debug-server --manifest-path rust/Cargo.toml mobile
cargo test -p fin-contracts --manifest-path rust/Cargo.toml
```

若 crate 名称不匹配，以 `cargo metadata` 后的真实 package 名为准。

### L1：Android 基础

```bash
cd android-client
./gradlew test --no-daemon
./gradlew assembleDebug --no-daemon
```

### L2：协议与投影契约

```bash
node android-client/scripts/smoke/ws-event-contract-smoke.mjs
python3 scripts/android-mvp/run_turn_channel_e2e.py
node android-client/scripts/smoke/projection-contract-check.mjs <e2e-output-json>
```

### L3：统一矩阵

```bash
./scripts/regression/run_android_client_matrix.sh
```

### L4：设备 E2E

```bash
adb connect 100.127.23.27:1234
adb install -r android-client/app/build/outputs/apk/debug/app-debug.apk
# 发正常 turn + 错误 turn，保存截图与日志
```

## 7. 执行顺序

1. 读取当前 git status，确认已有改动，禁止覆盖无关文件。
2. 对比 Codex baseline，确认 fin 缺失的是 item lifecycle，不是 UI 文案。
3. 在后端新增/收口 `turn.item.*` mapper。
4. 改 Android 只消费 `turn.item.*`。
5. 升级 E2E 与 projection contract check。
6. 接入 `run_android_client_matrix.sh` 与 CI。
7. 跑本地 provider live smoke。
8. 跑完整 matrix。
9. 安装到手机并在线截图/日志验收。
10. 更新报告：写明改了什么、验证命令、关键输出、日志/截图路径、剩余风险。

## 8. 禁止事项

1. 禁止 fallback / 降级 / 吞异常。
2. 禁止前端通过默认词掩盖 schema 缺失。
3. 禁止把 `tool`、`unknown` 当作有效渲染内容。
4. 禁止只汇报 build 成功。
5. 禁止没有真实 provider / turn E2E 证据就宣称推理链路可用。
6. 禁止 broad kill：`pkill`、`killall`、`kill $(...)`、`xargs kill`。
7. 禁止批量 checkout 或覆盖无关文件。

## 9. 完成定义 DoD

必须同时满足：

1. 后端发送统一 `turn.item.*` lifecycle。
2. Android UI 不再显示无意义 `tool` / `unknown` 占位。
3. 正常 turn 有 started → completed 链路。
4. 失败 turn 有 started → failed 链路，并显示具体 `error_summary`。
5. `runtime.health`、`provider.health`、WebSocket 连接状态分开可见。
6. `run_turn_channel_e2e.py` 验证 item lifecycle、label/title/purpose、error_summary。
7. `run_android_client_matrix.sh` 全绿，且 matrix 包含推理链路检查。
8. mimo provider live smoke 通过，或若外部 provider 临时不可用，必须有明确 provider.health 失败证据，不能把它说成链路成功。
9. Android 真机安装当前 APK 后完成正常 turn 与失败 turn 的截图/日志验收。
10. 交付报告写入 `reports/`，包含命令、关键输出、日志路径、截图路径、剩余风险。

## 10. 最终交付物

1. 代码改动：后端 mapper、Android item 消费、E2E、matrix、CI。
2. 文档：本文件 + closeout 报告。
3. 证据：
   - provider live smoke 输出。
   - turn channel E2E 输出。
   - android client matrix 输出。
   - 设备截图。
   - app connection logs / logcat。
4. `/goal` 提示词：见下方。

## 11. /goal 提示词

```text
/goal
目标：把 fin Android Agent 推理链路对齐 Codex item lifecycle，让本地 runtime/provider/tool 推理真实完成，turn 过程消息被消费并在 Android 只作为后端事件投影渲染。

实现文档：
- docs/goals/android-agent-reasoning-chain-completion-method.md
- docs/goals/android-agent-reasoning-chain-codex-alignment-plan.md

执行规范：
- 先对比 ~/code/codex 的 protocol/model 基线，再改 fin；后端事件是唯一真源，Android 只是控制与投影壳。
- 禁止 fallback、吞异常、UI 猜字段、默认显示 tool/unknown；缺 schema 必须暴露 schema_error。
- 统一使用 turn.item.started/delta/completed/failed；旧 turn.tool_event/error_event 只能由统一 mapper 派生，不能成为第二真源。
- 不覆盖无关改动，不使用 broad kill，不用 build 成功冒充推理链路成功。

验证：
- targeted cargo tests / Android unit test / assembleDebug。
- WS event contract smoke、turn channel E2E、projection contract check。
- ./scripts/regression/run_android_client_matrix.sh 必须全绿且包含 item lifecycle 检查。
- mimo provider live smoke 与 Android 真机正常 turn + 失败 turn 截图/日志验收。

完成标准：
- Android UI 不再出现无信息量的 tool/unknown，占位缺失会显示 schema_error。
- 每个 turn 的过程 item 都有 started 并最终 completed/failed，失败保留完整 error_summary。
- WS/runtime/provider 三层状态分离可见。
- reports/ 中有最终证据：命令输出、E2E/matrix 结果、设备截图、日志路径、剩余风险。
```
