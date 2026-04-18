# Turn Record Contract

本文档定义 `fin` 多轮历史中的 canonical `TurnRecord` contract。

它的定位是：

- 作为一次完整 closure 的 durable 会话单元
- 连接 `messages / context snapshot / reasoning / tool records / digest / closure trace`
- 成为 Web、replay、rebuild、harness 共同消费的中间真源

---

## 1. 目标

- 让“本轮到底发生了什么”有单一结构化对象
- 避免会话语义分散在 `messages.json`、`recent_digests.json`、`recent_closures.json` 多处而没有总锚点
- 为后续 multi-step inference loop 提供稳定 turn 边界

---

## 2. 最小对象

```text
TurnRecord
```

建议最小字段：

- `turn_id`
- `closure_id`
- `operation_id`
- `trace_id`
- `turn_index`
- `status`
- `created_at`
- `completed_at?`
- `refs`
- `user_input`
- `assistant_visible_output?`
- `progress_summary?`
- `control_feedback_ref?`
- `execution_note_refs[]`
- `context_snapshot_ref?`
- `reasoning_view_refs[]`
- `tool_record_refs[]`
- `provider_request_ref?`
- `provider_response_ref?`
- `closure_trace_ref?`
- `digest_ref?`
- `step_ids[]`

其中 `refs` 建议沿用 `EntityRefs`，至少包含：

- `session_id?`
- `task_id?`
- `topic_thread_id?`
- `dispatch_id?`
- `worker_id?`

---

## 3. 字段语义

### 3.1 identity

- `turn_id`：turn 唯一 id
- `closure_id`：本 turn 对应的 closure id
- `operation_id`：本轮入口 operation
- `trace_id`：本轮链路 id
- `turn_index`：session 内单调递增序号

### 3.2 lifecycle

- `status`：`running | completed | failed | interrupted | cancelled`
- `created_at`：turn 开始时间
- `completed_at?`：turn 正常收束或失败结束时间

规则：

- interrupted execution 不得伪装成 completed closure
- 若 turn 未完成，`digest_ref` 可以为空

### 3.3 visible conversation payload

- `user_input`：本轮用户/外部输入
- `assistant_visible_output?`：最终对用户可见的输出
- `progress_summary?`：给前台或状态探针消费的简版推进摘要

### 3.4 structured refs

- `control_feedback_ref?`：本轮 control feedback 记录位置
- `execution_note_refs[]`：本轮相关 notes
- `context_snapshot_ref?`：本轮送入模型的上下文快照
- `reasoning_view_refs[]`：本轮 reasoning view 引用
- `tool_record_refs[]`：本轮工具执行记录
- `provider_request_ref?` / `provider_response_ref?`：原始 provider 收发引用
- `closure_trace_ref?`：完整 closure trace
- `digest_ref?`：本轮 closure digest
- `step_ids[]`：turn 内部 step 链接

---

## 4. 硬规则

1. 每个有效 closure 必须对应一个 `TurnRecord`
2. `TurnRecord` 是 closure 级 canonical index，不是 provider raw trace 替代品
3. `messages.json` 仍可保留给 channel render，但 turn 级语义以 `TurnRecord` 为准
4. `TurnRecord` 不直接内嵌大块 raw payload，尽量持有 refs
5. UI / replay / rebuild 若需要“本轮完整事实”，必须先从 `TurnRecord` 开始追索

---

## 5. 与当前 M1 对象的关系

当前 M1 已有：

- `conversation/messages.json`
- `ContextSnapshotRecord`
- `ReasoningViewRecord`
- `ToolExecutionRecord`
- `ClosureTraceRecord`
- `DigestRecord`

但它们缺少统一 turn 索引。

后续实现原则：

- 不推翻现有 records
- 在现有 records 之上补出 `TurnRecord`
- `TurnRecord` 负责把分散 records 串成闭环
