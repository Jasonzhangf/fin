# Digest Family Contract

本文档定义 `fin` 的 digest family contract。

它是对现有 `DigestRecord` 的扩展收口，用来明确：

- 哪些 digest 类型存在
- 它们的职责边界是什么
- 哪些用于 continuity，哪些用于 rebuild，哪些用于会话选择

---

## 1. Digest family

`fin` 的 digest family 冻结为四类：

1. `ClosureDigestRecord`
2. `TaskDigestRecord`
3. `TopicDigestRecord`
4. `SessionDigestRecord`

---

## 2. ClosureDigestRecord

这是当前 `DigestRecord` 的直接延续。

### 最小字段

- `digest_id`
- `closure_id`
- `operation_id`
- `trace_id`
- `refs`
- `summary`
- `continuity_tail[]`
- `note_refs[]`
- `artifact_candidates[]`
- `created_at`

### 规则

1. 每个有效 closure 必须生成一个 `ClosureDigestRecord`
2. interrupted segment 不单独形成 closure digest
3. 它是最近连续性的主材料

---

## 3. TaskDigestRecord

职责：

- 汇总某个 `task_id` 当前阶段的稳定状态
- 用于 task continuation、task revive、handoff

### 最小字段

- `task_digest_id`
- `task_id`
- `session_id?`
- `topic_thread_id?`
- `updated_at`
- `goal_summary`
- `current_phase?`
- `done_items[]`
- `in_progress_items[]`
- `open_questions[]`
- `risk_items[]`
- `next_steps[]`
- `source_closure_digest_ids[]`
- `artifact_refs[]`

### 规则

- 采用 iterative update
- 不应每次从零总结整个 task 历史

---

## 4. TopicDigestRecord

职责：

- 汇总某条长期话题主线
- 用于 topic revive / rebind / switch 提示

### 最小字段

- `topic_digest_id`
- `topic_thread_id`
- `updated_at`
- `title`
- `one_line_summary`
- `goal_summary?`
- `stable_facts[]`
- `active_tasks[]`
- `related_task_ids[]`
- `source_task_digest_ids[]`
- `artifact_refs[]`

### 规则

- 更偏长期稳定总结
- 应少写瞬时推进细节

---

## 5. SessionDigestRecord

职责：

- 给 session 选择、恢复、列表展示提供概览

### 最小字段

- `session_digest_id`
- `session_id`
- `updated_at`
- `title`
- `one_line_summary`
- `active_task_id?`
- `active_topic_thread_id?`
- `recent_focus`
- `status_summary`
- `source_digest_ids[]`

---

## 6. Shared rules

### 6.1 Digest is not raw truth

digest 不替代 raw ledger / turn records / notes。

### 6.2 Iterative update

`TaskDigestRecord` 与 `TopicDigestRecord` 默认走 iterative update。

### 6.3 Rebuild usage

context rebuild 时优先消费：

- relevant task digests
- relevant topic digests
- recent closure digests

### 6.4 Knowledge promotion

digest 里的 `artifact_candidates` 不等于正式 knowledge artifact。

它们还需要：

- classify
- verify/grade
- assign scope
- publish

---

## 7. 与现有 digest-contract 的关系

`docs/contracts/digest-contract.md` 当前仍可视为：

- M1 的 `ClosureDigestRecord` 极简版本

后续扩展时：

- 不直接废弃旧 contract
- 让旧 `DigestRecord` 平滑演进到 closure digest 子集
