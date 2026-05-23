# Android Agent Reasoning Chain Codex Alignment Plan

## 目标

把 `fin` 的 agent 推理链路改造成与 `~/code/codex` 核心做法一致的可验证事件链：本地 runtime 完成推理，turn 过程消息被完整消费，Android 只作为控制界面与投影视图，不承担业务真相补偿。

## 验收标准

1. 一次用户输入必须产生完整 turn lifecycle：`turn.started -> item.started/delta/completed|failed -> turn.completed`。
2. 工具、reasoning、exec/error 过程必须有结构化 item，不允许前端显示 `tool` / `unknown` 这种无信息占位。
3. Android 端只消费后端事件/projection，不猜字段、不补业务语义。
4. 本地回归矩阵必须验证：provider 可用、turn 推理完成、tool/error/reasoning 过程被消费、前端投影字段完整。
5. provider/runtime 不可用时必须在 runtime 层显式失败，不允许被 WS `healthy` 掩盖。

## 范围

### In Scope

- `rust/crates/debug-server/src/mobile_ws.rs`
- `rust/crates/contracts/src/records.rs`
- `rust/crates/runtime/src/turn_records.rs`
- `rust/crates/runtime/src/session_materializer.rs`
- `android-client/app/src/main/assets/mobile-shell.html`
- `scripts/android-mvp/run_turn_channel_e2e.py`
- `scripts/regression/run_android_client_matrix.sh`
- `.github/workflows/ci.yml`

### Out of Scope

- 不重写 provider 协议层。
- 不把 Android 改成原生 Compose UI。
- 不引入 fallback provider 自动降级。
- 不修改无关 web-debug / qqbot / task-board 逻辑。

## Codex 对齐基线

参考 `~/code/codex` 的核心做法：

1. `ResponseItem` 是模型输出真源，区分 `Message`、`Reasoning`、`FunctionCall`、`FunctionCallOutput`、`LocalShellCall` 等类型。
2. turn 不是单个最终对象，而是一组强类型事件：`TurnStarted`、`ItemStarted`、`ReasoningContentDelta`、`ItemCompleted`、`ExecCommandBegin/OutputDelta/End`、`TurnComplete`。
3. 工具执行必须有可配对 ID：`call_id` / `turn_id` / started / completed / status / duration / output / error。
4. UI 只消费事件和 projection，不从泛化字段猜测工具名称或状态。
5. completion 由明确 `TurnComplete` 收口，而不是从 provider 结束或某个 final text 推断。

## 当前差异

1. `fin` mobile WS 发送 `turn.tool_event` 时把真实记录放在 `tool_record` 嵌套对象里，但 Android 曾读取顶层 `m.tool_name/m.status`，导致 `tool · running` / `tool · failed · unknown`。
2. `fin` 没有统一 `TurnItem` lifecycle，只有 `turn.progress/tool_event/error_event/trace_event/rendered`。
3. `fin` 的 incremental mobile event 来自轮询 `runtime/current` 快照，顺序与 started/completed/failed 事实不够强。
4. `fin` 的 reasoning 主要是 `ReasoningViewRecord.summary`，缺少可增量消费的 reasoning item/delta。
5. 当前回归已覆盖 turn event 存在，但未严格验证 Android 投影不出现 `tool/unknown`。

## 技术方案

### 1. 定义统一 Mobile Turn Event Contract

新增或规范化以下事件：

```text
turn.started
turn.item.started
turn.item.delta
turn.item.completed
turn.item.failed
turn.completed
runtime.health
provider.health
```

`turn.item.*` 必须包含：

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
  "started_at": "..."
}
```

完成/失败事件必须带：

```json
{
  "item_id": "...",
  "status": "completed|failed",
  "duration_ms": 123,
  "output_summary": "...",
  "error_summary": null
}
```

### 2. 后端 projection 单一化

`mobile_ws.rs` 不再发送字段不一致的自由 JSON。它必须从 `ToolExecutionRecord` / `ReasoningViewRecord` / `StepRecord` 映射成统一 `turn.item.*`。

旧事件兼容策略：

- `turn.progress` 可保留，但只表示阶段状态。
- `turn.tool_event` / `turn.error_event` 可暂时继续发，但必须由统一 item projection 生成。
- `turn.rendered` 只作为最终投影，不再承载过程真相。

### 3. Android 只渲染统一 item

`mobile-shell.html` 必须：

- 按 `item_id` 聚合 item。
- 用 `label/title/purpose/status/error_summary/duration_ms` 渲染。
- 禁止默认显示 `tool` / `unknown`。
- 对缺字段直接显示 `schema_error:<field>`，使问题暴露。
- 显示三层状态：`ws`、`runtime`、`provider`。

### 4. Runtime Health 明确化

新增/投影：

```text
runtime.health: available|unavailable|degraded
provider.health: available|unavailable|auth_failed|route_unavailable
```

`503 没有可用的内网节点` 必须变成 `provider.health=route_unavailable`，不能只渲染成普通 tool error。

### 5. 回归矩阵升级

`run_android_client_matrix.sh` 必跑：

1. Android unit tests。
2. Android build。
3. WS event contract smoke。
4. turn channel E2E。
5. projection contract check：读取 E2E 产物并断言：
   - `turn.item.started` 数量 > 0。
   - 每个 started 都有 completed/failed。
   - 没有 label 为 `tool` / `unknown` / 空。
   - error item 保留完整错误摘要。
   - rendered turn 包含 assistant response 与过程 item refs。

CI 中 live provider 测试按 secrets gate 控制；本地开发矩阵默认启用 live E2E。

## 实施步骤

### Phase 1：契约修复

1. 在 contracts 中定义 `MobileTurnEvent` / `MobileTurnItem` 数据结构。
2. 在 `mobile_ws.rs` 中统一用 mapper 生成 item lifecycle。
3. 修复 `tool_record` 嵌套/顶层字段不一致。
4. 保留旧事件兼容，但来源必须是统一 mapper。

### Phase 2：Android 消费端修复

1. 移除前端字段猜测逻辑。
2. 用 `item_id` 聚合 `turn.item.*`。
3. 渲染 `title/purpose/status/error_summary/duration_ms`。
4. 缺字段显示 schema error，不做 fallback。

### Phase 3：Runtime / Provider Health

1. 后端推送 `runtime.health` 和 `provider.health`。
2. 503 / auth / route error 明确分类。
3. Android 分开显示 WS、runtime、provider。

### Phase 4：回归矩阵

1. 扩展 `run_turn_channel_e2e.py`，验证 item lifecycle 和字段质量。
2. 增加 projection contract check。
3. 接入 `run_android_client_matrix.sh`。
4. CI job 使用同一矩阵脚本。

### Phase 5：在线验收

1. 本地跑完整矩阵。
2. Android 设备安装 APK。
3. 发一条正常推理请求和一条错误工具请求。
4. 截图确认 UI 不再出现 `tool/unknown` 占位。
5. 保存日志到 `reports/android-e2e-*`。

## 风险与规避

1. **协议双轨风险**：旧事件和新事件并行可能造成重复渲染。规避：旧事件只由 mapper 派生，前端只消费新事件。
2. **历史数据兼容风险**：旧 session 没有 item lifecycle。规避：历史渲染可显示 legacy 标记，但新回归只验新 turn。
3. **provider 不稳定风险**：live provider 可能因外部故障失败。规避：live gate 单独标记，contract/replay 测试必须稳定通过。
4. **Android 缓存旧 APK 风险**：验收前必须记录 build hash / versionName，并通过 screenshot/logcat 证明当前版本。

## 验证矩阵

```text
L0 unit:
- cargo test targeted mobile_ws mapper tests
- gradlew test

L1 contract:
- node android-client/scripts/smoke/ws-event-contract-smoke.mjs
- projection contract check: no tool/unknown labels

L2 local E2E:
- python3 scripts/android-mvp/run_turn_channel_e2e.py
- assert item started/completed/failed pairing

L3 device E2E:
- adb install APK
- send normal request
- send failing tool request
- capture screenshot + logcat + app files logs
```

## 完成定义

1. `run_android_client_matrix.sh` 全绿。
2. `run_turn_channel_e2e.py` 输出 `ok=true`，并包含 item lifecycle 检查。
3. Android UI 正常 turn 中展示具体步骤名，不出现 `tool` / `unknown`。
4. 错误 turn 展示完整错误摘要和失败工具名。
5. provider/runtime 不可用时状态清晰，不影响 WS 连接状态。
6. 审计报告补充最终证据路径。
