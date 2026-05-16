# Android Turn 双通道（Normal / Debug）实现与测试计划

## 1. 目标

把推理过程显示拆分为两个独立显示通道：

- **Normal 通道（用户态）**：仅显示用户需要的对话信息。
- **Debug 通道（调试态）**：显示完整推理细节（tool/error/control/closure/trace）。

两通道必须满足：
1. 同源消费同一份订阅 `turn.rendered`。
2. 仅渲染层分叉，不改状态层与发送链路（不耦合）。
3. 使用真实推理结果，不允许本地占位 turn。

---

## 2. 字段契约（Turn Contract）

## 2.1 Normal 字段（用户可见）
- `turn_id`
- `user_input`
- `assistant_response`
- `created_at`（可选展示）
- `status`（可选展示）

## 2.2 Debug 字段（调试可见）
- `control_feedback_summary`
- `tool_execution_summary`
- `closure_stop_source`
- `tool_execution_records[]`
- `error_records[]`
- `step_ids[]`
- `operation_id`
- `trace_id`

## 2.3 约束
- `turn.rendered` 一条消息携带 normal/debug 字段。
- 不允许 debug 通道单独建立“第二真源”。
- 字段缺失时只显示空，不伪造内容。

---

## 3. 实现计划

## P1. Contract 冻结
- 在文档与代码中冻结 `turn.rendered` schema。
- 明确 normal/debug 字段边界。

## P2. 服务端输出（`rust/crates/debug-server/src/mobile_ws.rs`）
- `turn.rendered` 挂载完整 debug 字段。
- `tool_execution_records/error_records` 必须从 runtime artifacts 真源读取。
- 用户可见 `assistant_response` 禁止注入 framework 额外文本。

## P3. 客户端状态层（`android-client/app/src/main/assets/mobile-shell.html`）
- 单一 `turnStore[]`。
- 单一入口：`onWs(turn.rendered)` 入库。
- 禁止本地“思考中”占位 turn。

## P4. 客户端渲染层
- 实现 `renderNormalTurn(turn)`。
- 实现 `renderDebugTurn(turn)`。
- debug 开关仅控制渲染分支。
- 默认 normal，debug 可随时打开/关闭。

## P5. 去耦防退化
- debug toggle 不能触发：重连、重发、重绑。
- debug toggle 前后 turnStore 不变。

---

## 4. 测试计划

## 4.1 Unit
- **U1**：同一 turn 在 normal/debug 渲染输出差异正确。
- **U2**：debug 开关切换前后 `turnStore` 内容 hash 一致。
- **U3**：debug on/off 下发送 payload 完全一致。

## 4.2 Contract
- **C1**：`turn.rendered` normal 字段完整性与类型校验。
- **C2**：debug 非空场景（tool/error 存在）。
- **C3**：debug 空场景（tool/error 为空）稳定渲染。

## 4.3 真实 E2E（必须真实推理）

### E1 普通对话轮次
- 连续发送 2-3 条普通请求。
- 验证 normal 连续对话渲染。

### E2 工具调用轮次
- 发送会触发工具调用的请求。
- 验证 `tool_execution_records[]` 字段正确透传。
- normal 不显示工具细节，debug 显示。

### E3 错误轮次
- 发送可控失败请求（触发 tool error）。
- 验证 `error_records[]` 和 `error_summary`。
- normal 只显示助手回复，debug 显示错误明细。

### E4 开关稳定性
- 同一会话切换 debug 开关 3 次并继续发送。
- 验证 turn 顺序/数量一致，无重发重连副作用。

---

## 5. 证据落盘

## 5.1 日志
- `reports/android-mvp-logs/turn-channel-contract.log`
- `reports/android-mvp-logs/turn-channel-e2e.log`
- `reports/android-mvp-logs/turn-channel-toggle.log`

## 5.2 截图
- `reports/android-mvp-screenshots/turn-normal.png`
- `reports/android-mvp-screenshots/turn-debug.png`
- `reports/android-mvp-screenshots/turn-error-debug.png`

## 5.3 验收索引
- `reports/android-mvp-validation.md` 增加：
  - Turn Channel Contract
  - Turn Channel E2E
  - Debug Non-Coupling

---

## 6. 门禁与完成定义（DoD）

满足以下全部条件才允许合入：
1. Unit + Contract + 真实 E2E 全 PASS。
2. 普通/工具/错误三类 turn 字段证据齐全。
3. 两通道同源消费证据齐全。
4. debug 开关不耦合业务链路证据齐全。
5. 验收索引与日志截图全部落盘。
