# 06 Web Debug Console

`web/` 第一阶段定位为 **调试后台**，不是面向最终用户的产品前台。

## 当前 M1 页面分区

- Current Projection
- Provider Debug
- Current Context Snapshot
- Recent Context History
- Last Run
- Live Event Stream
- Warnings / Errors

## 当前数据来源

- `runtime/projections/current_projection.json`
- `runtime/projections/current_snapshot.json`
- `runtime/projections/latest_events.jsonl`
- `runtime/current/last_run.json`
- `runtime/current/current_context.json`
- `last_run.json -> session_recent_contexts_path -> sessions/.../context/recent_contexts.json`

## 生命周期规则

1. Web 是观察层，不是状态真源。
2. `web-debug` 当前保持前台阻塞运行，不引入 detached daemon。
3. 前端刷新要节制：避免并发刷新、页面 hidden 时暂停主动轮询。
4. UI 问题排查必须先回到 raw event，再看页面投影。
