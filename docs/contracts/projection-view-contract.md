# Projection View Contract

`ProjectionView` 表示给 Web / CLI / harness 读取的当前视图。

## 最小字段

- `session_id?`
- `task_id?`
- `topic_thread_id?`
- `current_phase?`
- `latest_progress_id?`
- `latest_note_id?`
- `latest_digest_id?`
- `latest_provider_activity?`
- `warnings[]`

## 契约要求

1. Projection 只能消费 event 构建，不得自创业务语义。
2. Projection 可以缓存，但不能覆盖事实流。
3. 调试结论必须始终可以回溯到 raw events。
