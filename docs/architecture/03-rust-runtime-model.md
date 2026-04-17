# 03 Rust Runtime Model

## 核心实体

- `NodeId`：进程或宿主节点
- `WorkerId`：执行单元
- `AgentId`：能力身份
- `SessionId`：执行上下文
- `TaskId`：业务任务
- `DispatchId`：任务派发实例
- `LeaseId`：所有权租约

## 生命周期

### Worker
`discovered -> registered -> healthy -> degraded -> lost`

### Task
`created -> dispatched -> accepted -> running -> claimed -> verified -> closed`

### Session
`created -> attached -> active -> checkpointed -> resumed -> archived`

## 运行原则

1. worker 的所有权由 lease 与心跳共同约束。
2. task 的推进必须可重放；不能只存在内存中的隐式状态。
3. checkpoint / resume 依赖结构化事件与 state store，而不是 UI 快照。
4. runtime 必须可脱离 Web 独立运行。
