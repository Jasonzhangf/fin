# Reasoning View Contract

本文档定义 `fin` 可被 UI 渲染的 reasoning 摘要层 contract。

它不是原始 chain-of-thought，而是框架允许暴露的结构化 reasoning view。

---

## 1. 目标

- 给 Rich conversation mode 提供稳定真源
- 让 reasoning 能被会话前台和 debug 后台共同消费
- 避免前端靠临时字符串拼接“推理说明”

---

## 2. 最小对象

```text
ReasoningViewRecord
```

建议最小字段：

- `reasoning_id`
- `operation_id`
- `trace_id`
- `session_id?`
- `task_id?`
- `topic_thread_id?`
- `created_at`
- `summary`
- `decision_summary?`
- `continuity_summary?`
- `tool_intent_summary?`
- `risk_summary?`
- `next_step?`
- `source_refs`

---

## 3. 字段语义

### identity / refs

- `reasoning_id`：该 reasoning view 的唯一 id
- `operation_id`：所属 closure / turn
- `trace_id`：链路 id
- `session_id?` / `task_id?` / `topic_thread_id?`：所属范围

### content

- `summary`：本轮 reasoning 的一段简述
- `decision_summary?`：做了什么判断
- `continuity_summary?`：为什么继续/切 topic/保留观察
- `tool_intent_summary?`：为什么要用工具或为什么不用
- `risk_summary?`：当前风险、blocker、未确认点
- `next_step?`：推荐下一步

### provenance

- `source_refs`：该 reasoning view 依赖哪些结构化真源生成
  - `control_feedback`
  - `execution_note`
  - `digest`
  - `tool_execution_records`
  - `selected_events`

---

## 4. 硬规则

1. 不得把原始 hidden CoT 当作 `ReasoningViewRecord`
2. 只能由框架可持久化对象生成
3. 可被 Web Rich mode 与 Full Trace 共同消费
4. 若缺少结构化真源，宁可为空，也不由前端臆造

---

## 5. 当前阶段状态

当前 M1 尚未实现独立 `ReasoningViewRecord` 持久化。

现阶段可由以下对象近似支撑：

- `ControlFeedback`
- `ExecutionNote`
- `DigestRecord`

后续实现时再落成独立 contract / runtime record。
