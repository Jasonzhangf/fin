# Operation Envelope Contract

`OperationEnvelope<T>` 表示系统接收到的一次请求 / 控制动作。

## 最小字段

- `operation_id`
- `operation_type`
- `timestamp`
- `source`
- `sender_id`
- `trace_id`
- `sequence`
- `protocol_version`
- `correlation_id?`
- `causation_id?`
- `session_id?`
- `task_id?`
- `topic_thread_id?`
- `dispatch_id?`
- `worker_id?`
- `idempotency_key?`
- `timeout_ms?`
- `payload`

## 契约要求

1. Operation 不是事实真源，只表示请求或控制动作。
2. Operation 的状态推进应尽量通过 event 表达。
3. 同一 operation 的幂等语义必须能通过 `idempotency_key` 或等价字段表达。
4. 涉及跨模块链路时必须带 `trace_id`。
5. Operation 也必须可排序、可回放，因此需要稳定的 `sequence` 与 `protocol_version`。
6. `source` 表示来源模块，`sender_id` 表示具体发送者实例；两者不能混用。
