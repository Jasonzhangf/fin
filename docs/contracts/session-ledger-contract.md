# Session Ledger Contract

本文档冻结 fin 下一阶段的 ledger-first session 模型。

## 1. Core Principle

1. `Ledger` 是唯一事实源。
2. `Session` 是 ledger 的一个 track / view，不是 ledger 之外的第二事实源。
3. 一个 ledger root 下面只有一个 ledger identity，但可以分文件管理多个 track。
4. 所有 track 共享同一条 timeline，因此跨 track 的事实顺序必须可重建。
5. UI / Android / QQBot / Web 只消费 ledger 派生出的 snapshot / projection，不拥有 session 或 agent 协作语义。

## 2. Ledger Root

推荐布局：

```text
ledger/
  ledger.json
  timeline/
    index.jsonl
  tracks/
    session.snapshot.jsonl
    session.detail.jsonl
    events.jsonl
    turns.jsonl
    steps.jsonl
    tools.jsonl
    provider.jsonl
    control.jsonl
    knowledge.jsonl
  snapshots/
    current_session.json
    current_projection.json
  indexes/
    by-session.json
    by-task.json
    by-agent.json
    by-time.json
    knowledge-index.json
```

规则：

- `ledger.json` 定义 ledger identity、project binding、schema version、created_at。
- `timeline/index.jsonl` 是全局时间线索引，每条记录包含 `seq`、`ts`、`track`、`record_id`、`refs`。
- `tracks/*.jsonl` 是按职责切分的事实记录文件。
- `snapshots/*` 与 `indexes/*` 都是可重建派生物，不是事实源。

## 3. Timeline Semantics

每条 ledger record 必须包含：

- `ledger_id`
- `seq`
- `ts`
- `track`
- `record_id`
- `record_kind`
- `refs`
- `payload`
- `caused_by?`
- `supersedes?`

硬规则：

1. `seq` 在同一 ledger 内单调递增。
2. `ts` 用于展示和跨 track 排序，但冲突时以 `seq` 为最终顺序。
3. record 只追加，不原地改写；事实变更用 `supersedes` 或 correction record 表达。
4. track 文件可以拆分轮转，但 timeline index 必须能找回完整顺序。

## 4. Session Track

Session track 分为 snapshot 和 detail 两部分。

### 4.1 session.detail

`session.detail` 保存每个 turn 的完整累加过程，包括：

- user / channel input
- context snapshot ref
- provider request / response refs
- tool call / tool result refs
- step refs
- control feedback refs
- assistant visible output
- errors / retries / corrections

它回答：这一 turn 完整发生了什么。

### 4.2 session.snapshot

`session.snapshot` 是给用户和 channel 快速消费的精简视图，包括：

- 用户输入
- assistant visible output summary
- 重要工具合集
- 关键状态变化
- turn summary
- artifact refs

它回答：用户需要看到什么、恢复会话时先读什么。

规则：

1. snapshot 必须能从 detail + timeline 重建。
2. snapshot 不得包含无法追溯到 detail / timeline 的独立事实。
3. channel render 优先读 snapshot；debug / replay / audit 优先读 detail + timeline。

## 5. Knowledge Track

Ledger 必须有独立 `knowledge` track。

来源：

- summary / digest 中被验证为稳定事实的结论
- learning / control block 里的经验返回
- 工具执行后确认的 project 事实
- 用户明确要求记住的 project scope 经验

字段建议：

- `knowledge_id`
- `scope`: `project | task | session | agent | global_candidate`
- `statement`
- `evidence_refs[]`
- `source_record_ids[]`
- `confidence`
- `created_at`
- `valid_from`
- `supersedes?`
- `tags[]`

规则：

1. knowledge track 同 project 共享。
2. knowledge 必须带 evidence refs，指向 timeline/detail/tool/control 等事实记录。
3. learning 不能直接覆盖旧知识；必须追加新 record 并用 `supersedes` 表明事实变更。
4. context 组装可检索 knowledge track，但不能把 knowledge 当作未验证事实。

## 6. Local Query and Curation Tools

Ledger 查询与整理必须优先基于本地工具。

工具职责：

- 按 `session_id / task_id / agent_id / time range / track / record_kind` 查询 timeline。
- 从 timeline 重建 session snapshot。
- 从 detail 汇总 turn / tool / provider / error 链路。
- 从 summary / learning / control feedback 中提取 knowledge candidates。
- 校验 snapshot 是否可由 detail 重建。
- 校验 knowledge 是否有 evidence refs。

非目标：

- 不让 UI 自行推断 ledger 语义。
- 不让 prompt 直接吞 raw ledger。
- 不用 summary 替代 raw ledger。

## 7. Migration From Current Implementation

当前实现的映射关系：

- `conversation/messages.json` -> 未来 `session.snapshot` 的可见消息来源之一。
- `events/stream.jsonl + archive` -> 未来 `events` track / timeline 基底。
- `turns/recent_turns.json` -> 未来 `turns` track。
- `steps/recent_steps.json` -> 未来 `steps` track。
- `tools/recent_tool_records.json` -> 未来 `tools` track。
- `provider/recent_provider_*` -> 未来 `provider` track。
- `runtime/current/last_run.json` -> 未来 `snapshots/current_session.json` + index 派生物。

迁移原则：

1. 不再把 `conversation/messages.json` 称为 session 唯一 truth。
2. 先新增 ledger writer 和 query tool，双写现有 artifacts 与 ledger tracks。
3. 验证 snapshot 可从 detail/timeline 重建后，再让 UI/channel 改读 ledger snapshot projection。
4. 旧 recent/latest 文件最终降级为兼容 projection，不再是事实源。
