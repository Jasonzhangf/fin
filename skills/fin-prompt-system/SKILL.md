---
name: fin-prompt-system
description: Prompt system routing and execution skill for fin. Use when changing prompt layers, role modules, tool prompt specs, or prompt-related observability.
---

# fin Prompt System Skill

## 1) Intent

用于处理：

1. prompt system 的分层设计
2. prompt source ownership
3. role family / role baseline content
4. model family overlay
5. tool prompt spec
6. prompt 相关的 Web 可观测与验证

## 2) Canonical sources

1. `docs/architecture/25-prompt-system.md`
2. `docs/architecture/26-role-prompt-family-and-model-overlays.md`
3. `docs/architecture/27-stable-core-prompt.md`
4. `docs/prompts/01-stable-core-prompt-v1.md`
5. `docs/prompts/02-role-baselines-v1.md`
6. `docs/prompts/03-gpt-codex-overlay-v1.md`
7. `docs/architecture/24-session-render-truth-and-reasoning-input-assembly.md`
8. `docs/contracts/prompt-module-contract.md`
9. `docs/contracts/00-m1-contracts-index.md`
10. `AGENTS.md`

## 3) Routing rules

### A. 改 prompt 设计时

先改：

- `docs/architecture/25-prompt-system.md`

### B. 改 role prompt content / model overlay 时

先改：

- `docs/architecture/26-role-prompt-family-and-model-overlays.md`
- `docs/prompts/02-role-baselines-v1.md`
- `docs/prompts/03-gpt-codex-overlay-v1.md`

### C. 改 stable core prompt design / text 时

先改：

- `docs/architecture/27-stable-core-prompt.md`
- `docs/prompts/01-stable-core-prompt-v1.md`

### D. 改 prompt schema / structure 时

先改：

- `docs/contracts/prompt-module-contract.md`

### E. 改 prompt build / context block / Web 可见字段时

再改：

- Rust contracts / runtime
- Web debug 展示

## 4) Execution rules

1. 不把 prompt system 做成单一大字符串真源
2. 必须区分：
   - stable core
   - role modules
   - session overlay
   - turn envelope
3. 必须区分：
   - model tools
   - framework capabilities
4. stable core 只写跨角色稳定规则，不得混入当前 project/task/turn 动态内容
5. role baseline 只写稳定角色职责，project/task/turn 语义必须下沉到 overlay
6. project agent 与 system agent 的 project scope 不得混写成单 project 语义
7. schema 演进必须兼容旧 session artifacts；新增字段默认 `serde(default)`，改名字段保留 alias
8. 真实 provider 若出现“语义正确但 schema 不精确”的 drift，优先把 exact output contract 与禁用样式（如 `0.98/1.0`、字符串布尔值、extra keys）重复暴露到推理末尾，而不是先放宽 runtime truth 判定
9. 当前 `<fin_tool_calls>` 只是过渡期 fin contract，不是长期的 provider-native function/tool calling 真源；修改 prompt/tool schema 时要朝“provider-native standard call -> fin internal IR”方向靠拢，而不是继续固化自定义 wire
10. prompt 优化不得以“压缩上下文换通过率”为目标；真实业务默认会塞满上下文，只允许优化 layer/source assembly 与 rebuild，不允许用删减业务上下文冒充稳定性提升

## 5) Minimal validation

prompt 相关改动至少做：

1. `cargo test -p fin-runtime -p fin-cli`
2. `npx tsc -p rust/crates/debug-server/webui/tsconfig.json`
3. `python3 scripts/check-code-line-limit.py`
4. 至少验证一种 role/module 变化能反映到 `current_context.json` / Web debug
5. 至少一轮真实 `web-debug` 闭环，确认：
   - `current_context.json`
   - `recent_contexts.json`
   - Web Context 卡 / modal

## 6) Anti-patterns

- 把所有 prompt 内容直接硬编码成一坨系统提示词
- 把 role baseline、project policy、tool schema 混写成不可解释的大段文本
- 改了 prompt schema 却不兼容旧 session 数据
- 在 Web 层显示 prompt 真相，但 runtime 没有同字段
- 把 framework internal capability 冒充 model tool
- 先写 prompt 文本细节，后补 role/source/layer 边界
- 把 `<fin_tool_calls>` 的临时形态误当作长期标准 function call 协议
- 用 prompt 压缩规避真实 full-context 问题，而不是修正 contract / tool loop / timeout / observability

## 5.1) dispatch tool 归属与 prompt 设计

### dispatch tool 的角色
`dispatch`（`agent.assign`）是 system agent 的主动工具，不是 framework 自动行为。

prompt 必须让模型知道：
- 何时应该 dispatch（任务超出当前 agent cwd/权限边界时）
- dispatch 时必须自己生成任务描述（不允许 harness 填充 task_summary）
- dispatch 后必须等待 project agent 的 progress/result 事件
- 不允许 harness 绕过 model 直接给 project agent 注入业务语义

### 错误设计（当前审计发现的反模式）
- harness 在 system agent 推理前就预先 dispatch，硬编码 task_summary
- system agent 收到 project result 后不继续推理，由 harness 直接收口
- project agent 没有 progress 上报，静默结束
- system self-mailbox loopback 冒充 agent 间通信

### 正确设计
- system agent 首轮推理后，自主判断是否调用 dispatch tool
- dispatch tool call 携带模型生成的任务描述（`task_description` / `instruction`）
- project agent 执行中产生 progress 事件，system agent 可查询或被动接收
- project agent 完成后返回结构化 result，system agent 基于 result 继续推理
- harness 只负责 transport（执行 mailbox send）和 observe（记录事件），不生成业务语义

### dispatch 工具描述必须包含的字段
- `target_agent_kind`: project_agent / 指定 project
- `task_description`: **模型必须填写的任务描述字段**，禁止留空
- `cwd`: project agent 的工作目录
- `report_on_progress`: 是否需要持续回报
- 禁止在 tool description 里预设默认 task_summary
