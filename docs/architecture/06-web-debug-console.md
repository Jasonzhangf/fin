# 06 Web Debug Console

`web/` 第一阶段定位为 **调试后台**，不是面向最终用户的产品前台。

## P0 页面

- Cluster Overview
- Task Timeline
- Message Trace
- Raw Event Viewer
- Session Inspector

## P1 页面

- Replay Console
- Fault Injection Panel
- Worker Health / Heartbeat Heatmap

## 数据来源

- HTTP：查询聚合快照
- WebSocket：实时事件订阅
- Event Store / Replay Bundle：历史回放

## 硬规则

1. Web 是观察层，不是状态真源。
2. projection 可缓存，但不能覆盖 runtime 的事实。
3. UI 问题排查必须先回到 raw event，再看页面投影。
