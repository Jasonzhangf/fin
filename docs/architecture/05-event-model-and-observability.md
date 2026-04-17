# 05 Event Model and Observability

结构化事件是 `fin` 的共同观测真源。

## 事件要求

1. 所有关键状态推进必须发事件。
2. 错误必须发结构化错误事件，不能静默吞掉。
3. event schema 优先稳定，方便 replay 与 CI snapshot。
4. Web、CLI、Harness、CI 都消费同一套事件模型。

## 事件最小字段

- `event_id`
- `trace_id`
- `kind`
- `source`
- `timestamp`
- `task_id?`
- `dispatch_id?`
- `session_id?`
- `payload`

## 观测面

- 实时事件流
- 任务 timeline
- 消息 trace
- heartbeat / health
- 重试 / claim / resume 点
- raw event inspect

## 关键规则

没有可追踪事件链，就不能认为某个多 agent 协作行为已被正确实现。
