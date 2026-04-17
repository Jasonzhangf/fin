# Event Envelope Contract

`EventEnvelope<T>` 是 `fin` 的运行事实壳。

## 最小字段

- `event_id`
- `event_type`
- `timestamp`
- `source`
- `sender_id`
- `trace_id`
- `protocol_version`
- `correlation_id?`
- `causation_id?`
- `session_id?`
- `task_id?`
- `topic_thread_id?`
- `dispatch_id?`
- `worker_id?`
- `operation_id?`
- `sequence`
- `severity`
- `debug_visibility`
- `payload`

## 契约要求

1. Event 是运行事实真源。
2. Event 必须 append-only，不允许覆盖写历史。
3. payload 语义不可在真实传输链路中被裁剪改写。
4. 错误也必须使用结构化 event 表达。
5. 关键 side effect 默认必须有 `started / completed / failed` 三段事件。
6. Event 必须保留可排序字段 `sequence`，保证 replay / subscription cursor / cross-process delivery 可对齐。
7. `source` 表示来源模块，`sender_id` 表示具体发送者实例；例如 runtime-1、provider-gateway-2。
