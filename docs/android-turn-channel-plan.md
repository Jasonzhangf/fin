# Android 推理显示双通道整理（Normal / Debug）

## 1. 目标与验收

### 1.1 目标
- 将推理过程显示拆分为 **Normal** 与 **Debug** 两个通道。
- 两个通道都消费同一份订阅 `turn.rendered` 事件。
- 两个通道仅在渲染层分叉，不在状态层/发送链路分叉（不耦合）。

### 1.2 验收标准
1. `turn.rendered` 到达后，单一 turn store 入库一次。
2. Normal 只显示用户对话必要信息。
3. Debug 显示完整调试细节（tool/error/control/closure）。
4. Debug 开关仅影响显示，不影响 WS、发送、会话绑定、状态推进。
5. 不允许本地占位 turn（例如“思考中…”假消息）。

---

## 2. Prompt-to-Artifact Checklist

### 需求 R1：分成 Normal / Debug 两个通道
- 代码落点：
  - `android-client/app/src/main/assets/mobile-shell.html`
- 证据：
  - `renderNormalTurn(turn)` 与 `renderDebugTurn(turn)`（或等价拆分函数）
  - 设置页 debug toggle 可开关 debug 区块

### 需求 R2：都消费订阅 turn 信息
- 代码落点：
  - `rust/crates/debug-server/src/mobile_ws.rs`
  - `android-client/app/src/main/assets/mobile-shell.html`
- 证据：
  - WS `onmessage` 仅从 `turn.rendered` 入库 turn
  - 不存在第二条“debug 专用消息链路”作为真源

### 需求 R3：不耦合
- 代码落点：
  - `mobile-shell.html` store + renderer
- 证据：
  - debug 开关不触发重连、不重发、不改 session bind
  - turn store 数据结构与写入逻辑不依赖 debug 状态

---

## 3. 设计拆解（先后顺序）

### F-A Contract 层
1. 明确 `turn.rendered.normal` 必要字段：
   - `user_input`
   - `assistant_response`
2. 明确 `turn.rendered.debug` 字段：
   - `tool_execution_records[]`
   - `error_records[]`
   - `control_feedback_summary`
   - `tool_execution_summary`
   - `closure_stop_source`
3. 缺省策略：字段缺失时显示空，不伪造。

### F-B 服务端聚合层
1. `mobile_ws.rs` 在返回 `turn.rendered` 时附带完整 debug 字段。
2. tool/error 仅从 runtime artifacts 读取（不拼假数据）。
3. 用户可见 answer 只保留模型输出，不注入 framework 额外提示文本。

### F-C 客户端状态层
1. 单一 `turnStore[]`。
2. 单一入口：`onWs(turn.rendered) -> push(turn)`。
3. `render(mode, turns)` 纯渲染分发：
   - normal path
   - debug path

### F-D 客户端交互层
1. 设置页增加 debug toggle。
2. 默认 normal。
3. debug 开启时显示“同一 turn 的 debug 扩展块”。

---

## 4. 测试矩阵（仅针对该需求）

### Unit
- U1：同一 turn 输入在 normal/debug 渲染差异正确。
- U2：debug toggle 不改变 turnStore 内容。

### Contract
- C1：`turn.rendered` 字段 schema 校验（normal/debug）。
- C2：tool/error 空与非空两种 case。

### E2E
- E1：真实发送 3 轮，normal 对话连续。
- E2：切 debug 后，历史同一 turn 出现 tool/error/control/closure。
- E3：切 debug 前后 turn 数量与顺序一致。

### Evidence
- 日志：`reports/android-mvp-logs/turn-channel-*.log`
- 截图：
  - `reports/android-mvp-screenshots/turn-normal.png`
  - `reports/android-mvp-screenshots/turn-debug.png`
- 验收索引：`reports/android-mvp-validation.md` 增加“turn-channel”段落。

---

## 5. 风险与约束
1. 禁止前端占位 turn。
2. 禁止 debug 通道自建真源。
3. 禁止 framework 根据用户输入直接做用户可见判断文本注入。
4. 任何失败必须显式展示到 debug 错误区块。

---

## 6. 完成定义（DoD）
- 以上 U/C/E2E 全部通过。
- 证据文件全部落盘。
- 用户态界面不出现调试噪音。
- debug 可按需打开查看完整推理 turn 细节。
