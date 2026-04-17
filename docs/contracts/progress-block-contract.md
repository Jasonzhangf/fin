# Progress Block Contract

`ProgressBlock` 表示当前执行脉搏。

## 最小字段

- `progress_id`
- `session_id?`
- `task_id?`
- `dispatch_id?`
- `worker_id?`
- `phase`
- `blocker?`
- `next_step?`
- `health_hint?`
- `tool_snapshots[]`

`ToolSnapshot` 最小字段：

- `tool_name`
- `status`
- `summary`

## 契约要求

1. ProgressBlock 是高频、实时、偏监控的数据块。
2. 它用于表示当前推进状态，不替代 digest / note。
3. 若模块有关键工具执行，必须能投影为 tool snapshot。