# 06 Web Debug Console

`web/` 第一阶段定位为 **调试后台**，不是面向最终用户的产品前台。

## 当前 M1 页面分区

- 左侧 `Conversation`
  - user 永远在右侧
  - assistant / 其他 agent 类消息永远在左侧
  - 点击消息后以 `operation_id` 作为 request focus
- 右侧 `Selected Request`
  - 顶部只保留本轮请求摘要：operation / trace / provider / model / finish / message snippets
- 右侧 `Debug Dashboard`
  - 首层只显示 4 个 **digest summary cards**，尺寸一致，不直接展开全部字段
  - 点击 card 后，在下方 `detail pane` 展示完整结构化明细（递进树 + 折叠）
  - 4 个 card 主题固定：
    - `Provider`
    - `Context`
    - `System`
    - `Operation & Event`

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
3. 前端刷新采用消息驱动：通过 `/api/watch` SSE 推送触发 refresh；页面 hidden 时关闭 watch，恢复可见后重连。
4. UI 问题排查必须先回到 raw event，再看页面投影。

## 当前交互规则

- 左侧聊天区点击某条消息后，以该条消息对应的 `operation_id` 作为 focus。
- 旧的 tab inspector（overview/provider/context/history/alerts）不再作为主交互；当前主交互改为 dashboard summary cards + detail pane。
- focus pane 收敛为顶部 selected request summary，不再承载大面积明细 tab。
- Web 展示时间使用本地时间；runtime demo/transcript/web-debug 当前生成的时间戳也写本地时区格式。
- 前端聊天区禁止点击后自动跳回底部；只有在用户原本接近底部且收到新消息时才自动滚到底。
