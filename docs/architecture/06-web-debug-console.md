# 06 Web Debug Console

`web/` 第一阶段定位为 **调试后台**，不是面向最终用户的产品前台。

## 当前 M1 页面分区

- 顶部 `Project / Team`
  - `Project` strip 显示当前 project context
  - `Team` strip 预留为 team status plane；当前可以先是稳定 UI 槽位，后续再接真实 team truth
- 左侧 `Workspace Rail`
  - 承载 project settings / session / skills / plugins / runtime facts 的导航与摘要
  - 左栏不承载主会话正文，不得把聊天主舞台塞回左栏
  - 左栏第一层只显示轻量 digest，不把大量字段直接堆在首页
  - 左栏详情通过点击 digest card 后进入 detail modal / popup 查看
- 中间 `Conversation`
  - 中间是主会话舞台，不是一个窄侧栏聊天盒子
  - user 永远在右侧
  - assistant / 其他 agent 类消息永远在左侧
  - 点击消息后以 `operation_id` 作为 request focus
  - 底部输入器采用 **floating composer**：悬浮在主会话底部，而不是嵌进左栏
  - composer 下方可以挂 project / session / workdir / model / branch 等轻量上下文条
  - 底部 `Progress Update` bar 是 project progress update framework 的 UI 架子
    - 当前可先消费 `taskDigest`
    - 后续真源目标是 `update_plan record -> runtime/session artifact -> UI + system agent fanout`
    - 推理动画只是 progress 的观察视图，不能成为第二事实源
- 右侧 `Selected Request`
  - 顶部只保留本轮请求摘要：operation / trace / provider / model / finish / message snippets
- 右侧 `Debug Dashboard`
  - 整个右侧保持 **单列纵向流**
  - `Provider / Context / System / Operation & Event` 固定为 4 个纵向 digest card
  - 禁止两列 waterfall / masonry 并排压缩右侧信息
  - 首页 card 要更矮，只承担 digest 摘要，不直接展开深层结构
  - 右栏宽度应优先保证可读性；若结构较深，应优先放宽右栏而不是压缩内容
  - 点击 card 后进入 detail modal 查看完整结构；modal 内允许 section 折叠
  - `Context` card 的明细必须按类别分 section 展示（例如 prompt layers / session overlay / growing context / rendered trace），避免把所有 context 一次性堆在同一屏

## 当前数据来源

- `runtime/projections/current_projection.json`
- `runtime/projections/current_snapshot.json`
- `runtime/projections/latest_events.jsonl`
- `runtime/current/last_run.json`
- `runtime/current/current_context.json`
- `last_run.json -> session_recent_contexts_path -> sessions/.../context/recent_contexts.json`

## 生命周期规则

1. Web 是观察层，不是状态真源。
2. WebUI 默认连接 `system_agent` control listener；project agent listener 可以被显式连接，但不是 WebUI 默认目标。
3. `web-debug` 当前保持前台阻塞运行，不引入 detached daemon。
4. 前端刷新采用消息驱动：通过 `/api/watch` SSE 推送触发 refresh；页面 hidden 时关闭 watch，恢复可见后重连。
5. UI 问题排查必须先回到 raw event，再看页面投影。

## 当前交互规则

- 中间聊天区点击某条消息后，以该条消息对应的 `operation_id` 作为 focus。
- 旧的 tab inspector（overview/provider/context/history/alerts）不再作为主交互；当前主交互改为 dashboard summary cards + detail modal。
- focus pane 收敛为顶部 selected request summary，不再承载大面积明细 tab。
- 右侧 detail / summary 统一优先单列占满宽度；若需要更多信息，应通过更宽 modal 展示，避免在窄侧栏里硬塞深层结构。
- 左侧 rail 与右侧 dashboard 首页都只承担 digest；深层内容统一点开再看。
- Web 展示时间使用本地时间；runtime demo/transcript/web-debug 当前生成的时间戳也写本地时区格式。
- 前端聊天区禁止点击后自动跳回底部；只有在用户原本接近底部且收到新消息时才自动滚到底。
