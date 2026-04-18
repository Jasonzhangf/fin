# Step Ledger Contract

本文档定义 `fin` 在单个 turn 内部的最小 step ledger contract。

它的目标不是替代 event ledger，而是把一次 turn 内部的多步推进切成稳定的步骤单位，供：

- multi-step inference loop
- debug/replay
- pause/resume
- pending input / status probe

共同消费。

---

## 1. 最小对象

```text
StepRecord
```

建议最小字段：

- `step_id`
- `turn_id`
- `operation_id`
- `trace_id`
- `step_index`
- `step_kind`
- `status`
- `started_at`
- `ended_at?`
- `refs`
- `summary`
- `input_ref?`
- `output_ref?`
- `event_ids[]`
- `progress_ref?`
- `note_refs[]`
- `blocked_by_step_id?`
- `next_step_hint?`

其中 `refs` 仍建议沿用 `EntityRefs`。

---

## 2. Step kinds

`step_kind` 建议最少支持：

- `routing`
- `context_build`
- `provider_request`
- `model_parse`
- `tool_dispatch`
- `tool_wait`
- `tool_result`
- `control_feedback`
- `status_probe`
- `finalize`
- `interrupt_marker`

后续可以扩展，但不应破坏这些基础语义。

---

## 3. 字段语义

### identity

- `step_id`：step 唯一 id
- `turn_id`：所属 turn
- `operation_id` / `trace_id`：链路归属
- `step_index`：turn 内单调递增序号

### lifecycle

- `status`：`started | completed | failed | cancelled | timed_out | skipped`
- `started_at`
- `ended_at?`

### semantic body

- `summary`：本 step 的人类可读摘要
- `input_ref?`：输入材料引用
- `output_ref?`：输出材料引用
- `event_ids[]`：关联 event ids
- `progress_ref?`：对应 progress block
- `note_refs[]`：相关 execution notes

### linkage

- `blocked_by_step_id?`：该 step 被哪个 step 阻塞
- `next_step_hint?`：框架建议的下一步

---

## 4. 设计边界

`StepRecord` 与 `EventEnvelope` 的区别：

- event 记录“发生了什么事实”
- step 记录“这个 turn 内的推进结构”

因此：

- 一个 step 可以关联多个 events
- 一个 event 不一定能单独代表一个 step

---

## 5. 硬规则

1. step ledger 不替代 raw event ledger
2. step ledger 必须能把 turn 内多步推进结构化出来
3. pause/resume 时恢复点优先基于 step，而不是猜测 provider/tool 当前状态
4. status probe 若引用运行中信息，应定位到当前 active step
5. `step_index` 必须在同 turn 内保持单调
