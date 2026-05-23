# Android 推理细节渲染升级计划（基于 fin + 参考 finger）

## 0. 目标定义
在不改变“Rust runtime 作为唯一推理真源”的前提下，让 Android 客户端从“仅展示助手文本 + 工具时间线”升级到“可读、可审计、可回放”的推理细节渲染能力，并且支持持续升级与回归验证。

---

## 1. 当前问题（已审计）
1. Android `mobile-shell.html` 仅消费：`assistant_response / tool_execution_records / error_records`。
2. Runtime 已有 `ReasoningViewRecord` durable artifact，但未进入 Android 实时事件。
3. 前端缺少“推理分层展示模型”：摘要层、步骤层、原始层。
4. 版本兼容策略不足：字段扩展后缺少 schema 版本与向后兼容约束。

---

## 2. 方案原则（唯一真源）
1. **真源唯一**：推理细节只由 Rust runtime 生成；Android 仅渲染，不二次推断语义。
2. **事件优先 + 文件兜底（非业务 fallback）**：
   - 首选实时事件携带 `reasoning_view` 快照。
   - 历史回放从 materialized artifacts 读取（同一真源不同读取路径，不是双语义）。
3. **三层可观测**：
   - L1 摘要层（用户默认看）
   - L2 步骤层（可展开）
   - L3 原始层（debug 开关）
4. **向后兼容**：旧 daemon 无 `reasoning_view` 字段时，UI 明确显示“该轮未提供 reasoning_view”，不伪造内容。

---

## 3. 目标数据契约（M1）
在 `turn.rendered`（或当前等价事件）中增加：

```json
{
  "reasoning_view": {
    "reasoning_id": "...",
    "summary": "...",
    "decision_summary": "...",
    "continuity_summary": "...",
    "tool_intent_summary": "...",
    "risk_summary": "...",
    "next_step": "...",
    "source_refs": ["..."]
  },
  "reasoning_schema_version": "1"
}
```

说明：字段对齐现有 `ReasoningViewRecord`，避免 Android 自创语义。

---

## 4. 实施分阶段

### Phase A（后端契约打通）
- 修改 contracts：为 turn-render payload 增加 `reasoning_view` + `reasoning_schema_version`。
- 修改 runtime finalize/event emission：将当前 turn 对应 `ReasoningViewRecord` 注入事件。
- 增加序列化单测：
  - 有 reasoning_view
  - 无 reasoning_view（兼容旧路径）

**交付物**
- rust/crates/contracts: 新增/更新结构体字段
- rust/crates/runtime: 事件构造链路补齐
- 对应 rust tests 通过

### Phase B（Android 渲染升级）
- `mobile-shell.html` turn 数据结构加入 `reasoningView`。
- 新增“推理细节卡片”：
  - 默认只显示 `summary`（简洁）
  - 点击展开显示 decision/continuity/tool_intent/risk/next_step
  - debug 开启时显示 `source_refs` 与 raw JSON
- 缺失字段时显示“本轮无 reasoning_view（daemon 版本可能较旧）”。

**交付物**
- android-client/app/src/main/assets/mobile-shell.html
- 前端渲染逻辑与样式

### Phase C（历史回放一致性）
- `session.history` 返回的 turn 也带 reasoning_view（或 reasoning_view_ref + 后端展开）。
- Android 历史页面与实时页面保持同一展示模型。

**交付物**
- history payload 对齐
- 历史/实时一致性测试

### Phase D（稳定性与升级）
- 增加端到端回归脚本：
  1) 发起一轮推理
  2) 校验 UI 出现 reasoning summary
  3) 展开后字段完整
  4) 升级 APK 后重复校验
- 文档化 schema 版本策略。

**交付物**
- regression receipt
- docs/contracts + docs/closeout 更新

---

## 5. 验收���准（Done Definition）
1. Android 实时消息卡可稳定显示 reasoning summary。
2. 可展开看到 reasoning 结构字段（至少 5 项）。
3. debug 模式可查看 raw reasoning payload。
4. daemon 新旧版本兼容明确：旧版本不崩溃，不伪造。
5. 回归证据齐全：Rust tests + Android 手工/自动化截图或日志。

---

## 6. 风险与控制
1. **风险**：事件 payload 增大。
   - 控制：只传摘要字段；大文本放 artifact 引用。
2. **风险**：前后端 schema 漂移。
   - 控制：`reasoning_schema_version` + contract tests。
3. **风险**：UI 复杂度上升。
   - 控制：默认折叠，仅按需展开。

---

## 7. 为什么这是唯一正确方向（本目标下）
1. 你要求“稳定可持续升级 + 推理细节渲染”，唯一可持续方式是**继续绑定 runtime 真源**，而不是在 Android 侧再造推理语义。
2. 复用已有 `ReasoningViewRecord` 可最小改造达成目标，避免第二套模型。
3. 事件补字段 + UI 分层渲染，能同时满足实时性、可读性、可回放与升级兼容。
