# Provider Event Contract

本文档定义 `fin` 中 provider 执行结果侧的 contract 真源。

目标：

- 明确 provider 响应属于 event，而不是 operation
- 冻结 provider event family
- 明确 producer / consumer + subscription 的消费边界

---

## 1. 核心判断

provider 的执行结果必须通过 event 进入系统。

event 表达：

- 已经发出了请求
- 已经收到了响应
- 已经收到 stream delta
- 已经完成
- 已经失败

event 不是：

- 下次请求模板
- 可反写的内存态
- 特定 UI 专用 DTO

---

## 2. 最小事件族

当前冻结的 provider 事件族：

- `provider.operation_accepted`
- `provider.dispatch_started`
- `provider.gateway_request_sent`
- `provider.gateway_stream_delta`
- `provider.gateway_response_received`
- `provider.response_normalized`
- `provider.completed`
- `provider.failed`

失败扩展建议：

- `provider.failed.auth`
- `provider.failed.timeout`
- `provider.failed.http_status`
- `provider.failed.transport`
- `provider.failed.parse`
- `provider.failed.route`
- `provider.failed.capability_mismatch`

---

## 3. 最小 envelope 关联字段

provider event 应至少能关联：

- `event_id`
- `event_type`
- `occurred_at`
- `trace_id`
- `operation_id`
- `session_id?`
- `task_id?`
- `dispatch_id?`
- `worker_id?`
- `source`
- `sequence`
- `severity`
- `debug_visibility`
- `payload`

---

## 4. payload 原则

provider event payload 建议包含：

- provider name
- target model
- actual model（若 gateway 返回）
- request id / response id
- duration_ms
- usage
- status code
- finish reason
- error category
- redacted preview
- raw sample path

不建议在 event 中直接塞入完整原始请求/响应大 body。

完整原始样本应落到：

- `~/.fin/logs/provider/`
- `~/.fin/diagnostics/error-samples/`
- `~/.fin/diagnostics/traces/`

---

## 5. producer / consumer 语义

provider event 默认采用 producer / consumer 设计。

### producer

可以产出 provider event 的模块：

- provider gateway client
- runtime
- replay engine

### consumer

可以订阅 provider event 的模块：

- projector
- Web debug
- CLI
- harness
- channel bridge

producer 不关心谁消费；consumer 不得回写历史。

---

## 6. subscription 语义

consumer 默认按 subscription/filter 消费 provider event。

可按以下维度订阅：

- `event family`：如 `provider.*`
- `trace_id`
- `task_id`
- `session_id`
- `worker_id`
- `debug_visibility`
- `severity`

这意味着：

- debug 页面可以订阅 provider 相关事件
- channel bridge 可以只订阅失败或完成事件
- harness 可以只订阅断言需要的事件

它们使用的是同一 event truth，而不是不同架构。

---

## 7. owning layer

此 contract 的 owning layer：

- `docs/contracts/`
- `rust/crates/contracts`
- `rust/crates/provider`
- `rust/crates/debug-server`

Web / CLI / harness 只能消费，不应复制语义。
