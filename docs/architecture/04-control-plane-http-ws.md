# 04 Control Plane: HTTP + WebSocket

`fin` v1 控制面默认采用 **HTTP API + WebSocket 事件流**。

## 选择原因

- 调试可见性高，便于抓包与排查
- 结构简单，适合先把 harness 与 Web 调试台打通
- 跨进程与跨网段都能快速验证

## 平面拆分

## Channel 默认连接边界

- WebUI / Android / QQBot 等用户界面 channel 默认只连接本机 `system_agent` 的 control listener。
- `project_agent` 可以按配置启动自己的 listener，并允许被显式连接，但它不是任何 UI/channel 的默认连接目标。
- UI/channel 不枚举并直连 project agent；需要 project 协作时，由 `system_agent` 读取 runtime truth / Agent RPC presence 后路由或发 mailbox。
- `project_agent` listener 的存在只表示可被 Agent RPC / 显式调试工具连接，不改变 channel adapter 的 system-agent 入口语义。

### Control Plane（HTTP）
- 注册 / 注销 worker
- 查询 capability
- 创建 dispatch
- ack / fail / claim / complete
- 健康检查

### Event Plane（WebSocket）
- `agent.*`
- `worker.*`
- `task.*`
- `transport.*`
- `harness.*`

## 统一要求

每个请求/响应/事件都应带：

- `trace_id`
- `timestamp`
- `source`
- `session_id`（如适用）
- `task_id`（如适用）
- `dispatch_id`（如适用）

## 非目标

- v1 不先做复杂多协议共存
- v1 不让 Web 直接读 runtime 私有内存状态
