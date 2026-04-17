# Execution Note Contract

`ExecutionNote` 表示每轮推理或控制周期里的重要沉淀。

## 最小字段

- `note_id`
- `session_id?`
- `task_id?`
- `dispatch_id?`
- `worker_id?`
- `summary`
- `decision?`
- `lesson?`
- `blocker?`
- `next_step?`
- `created_at`

## 契约要求

1. ExecutionNote 比 progress 更稳定，但比 digest 更细。
2. 它是 closure 结束后 digest 的输入来源之一。
3. 它可以来源于模型候选输出，但最终写入由 framework 完成。