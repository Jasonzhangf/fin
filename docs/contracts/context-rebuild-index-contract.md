# Context Rebuild Index Contract

本文档定义 `fin` 的 `ContextRebuildIndex` 最小 contract。

它的目标是让 framework 在以下场景中稳定重建上下文：

- token pressure
- topic shift
- task switch
- session revive
- worker handoff
- long-gap resume

---

## 1. 最小对象

```text
ContextRebuildIndex
```

建议最小字段：

- `rebuild_index_id`
- `session_id`
- `updated_at`
- `active_task_id?`
- `active_topic_thread_id?`
- `recent_turn_ids[]`
- `recent_closure_digest_ids[]`
- `recent_note_ids[]`
- `candidate_task_digest_ids[]`
- `candidate_topic_digest_ids[]`
- `candidate_artifact_refs[]`
- `preferred_scopes[]`
- `budget_hint?`
- `rebuild_reason?`

---

## 2. 字段语义

### current anchors

- `active_task_id?`
- `active_topic_thread_id?`

表示当前最可能延续的主线。

### continuity anchors

- `recent_turn_ids[]`
- `recent_closure_digest_ids[]`
- `recent_note_ids[]`

表示 rebuild 时必须优先考虑的连续材料。

### retrieval anchors

- `candidate_task_digest_ids[]`
- `candidate_topic_digest_ids[]`
- `candidate_artifact_refs[]`
- `preferred_scopes[]`

表示 rebuild 时可以参与检索和装配的候选池。

### control hints

- `budget_hint?`：如 token budget / max blocks
- `rebuild_reason?`：如 `token_pressure | topic_switch | revive | handoff | resume`

---

## 3. 设计边界

`ContextRebuildIndex` 不是最终 context payload。

它只是 framework 用来做 rebuild 的索引和候选集。

最终 `ContextView` 仍由：

- prompt layers
- control / routing state
- project / collab state
- retrieved knowledge
- recent continuity tail
- current input

共同装配产生。

---

## 4. 硬规则

1. rebuild index 必须由 framework 维护
2. rebuild index 不直接保存大块 raw history
3. rebuild index 只保存 anchor / candidate / scope / reason
4. rebuild 不能跳过 recent continuity tail
5. rebuild 不能把未验证 artifact 当成稳定事实强塞进 context
