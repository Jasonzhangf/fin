# 21 Message Envelope and Handshake State Machine

本文档定义 `fin` 在外部消息通道上的最小公共消息头与握手状态机。

本文件只覆盖：

1. 公共消息头长什么样
2. RPC / Mailbox / EventBus 三类 message 的最小 envelope
3. 哪些字段是必需，哪些是可选
4. 最小握手状态机如何冻结

本文件不展开：

- 具体 transport 编码
- HTTP / WS / QUIC / gRPC 细节
- 加密实现
- 鉴权实现
- 重试实现

---

## 1. 核心判断

`fin` 的外部消息 envelope 不是内部 operation/event 的替代物，而是：

```text
external transport message
  -> normalize / handshake
  -> operation/event materialization
```

冻结判断：

1. 外部消息先服务 transport / routing / handshake
2. 内部事实仍然由 operation/event 承担
3. 公共消息头尽量统一，减少跨通道分裂
4. 握手机制允许按通道裁剪，而不是强迫三类通道完全同构

---

## 2. 公共消息头（Common Message Header）

所有外部消息通道默认共用一组最小头字段。

推荐最小字段：

- `message_id`
- `protocol_version`
- `timestamp`
- `source`
- `sender_id`
- `recipient_id?`
- `trace_id`
- `correlation_id?`
- `causation_id?`
- `sequence?`
- `channel_kind`
- `message_kind`
- `delivery_mode`
- `ack_policy?`
- `lease_ttl_ms?`
- `deadline_ms?`
- `auth_context?`

---

## 2.1 字段语义

### identity / trace

- `message_id`：外部消息唯一 ID
- `trace_id`：贯穿 ingress -> normalize -> operation/event 的链路 ID
- `correlation_id?`：归组一批相关消息
- `causation_id?`：直接因果来源

### sender / recipient

- `source`：来源模块或入口，例如 `rpc_ingress` / `mailbox` / `eventbus`
- `sender_id`：具体发送者实例
- `recipient_id?`：定向目标；广播消息可为空

### ordering / version

- `protocol_version`：消息 envelope 版本
- `timestamp`：消息创建时间
- `sequence?`：发送侧局部顺序，可选；不是内部 event truth 的替代

### delivery / control

- `channel_kind`：`rpc | mailbox | eventbus`
- `message_kind`：该通道内的消息类型
- `delivery_mode`：`sync | async | notify | stream`
- `ack_policy?`：是否需要 ack，如何 ack
- `lease_ttl_ms?`：该消息或占用的租期
- `deadline_ms?`：请求超时/截止时间
- `auth_context?`：鉴权上下文句柄或摘要

---

## 2.2 公共规则

1. 所有外部消息必须带 `protocol_version`
2. 所有跨机器消息必须带 `trace_id`
3. 所有定向消息必须带 `recipient_id`
4. `sequence?` 只是发送侧辅助顺序，不取代内部 `event.sequence`
5. `ack_policy?` 与 `lease_ttl_ms?` 是可选能力，不是所有消息都必需

---

## 3. RPC Message Envelope

RPC 适合 request/response 型定向调用。

### 3.1 RPC Request

推荐最小字段：

- `header`（公共头）
- `rpc_method`
- `request_id`
- `target_agent`
- `target_capability?`
- `payload`

其中：

- `channel_kind = rpc`
- `delivery_mode = sync | async`
- `message_kind = request`

### 3.2 RPC Response

推荐最小字段：

- `header`
- `request_id`
- `response_kind`
- `accepted`
- `result_handle?`
- `payload?`
- `error?`

其中：

- `channel_kind = rpc`
- `message_kind = response`

### 3.3 RPC 适合的语义

- 明确请求某个 agent 做一件事
- 返回 accepted/rejected
- 返回 result handle 或错误
- 支持跨机器、跨网段定向调用

---

## 4. Mailbox Message Envelope

mailbox 是定向投递 + 握手语义。

### 4.1 Mailbox Message

推荐最小字段：

- `header`
- `mailbox_id`
- `recipient_agent`
- `message_topic`
- `payload`

其中：

- `channel_kind = mailbox`
- `delivery_mode = sync | async`

### 4.2 Mailbox 适合的语义

- claim / handoff / lease 协商
- agent A -> agent B 定向消息
- 半同步或异步交接
- recipient 明确存在

### 4.3 Mailbox 对 ack / lease 更敏感

相较 eventbus，mailbox 更常见：

- `ack_policy`
- `lease_ttl_ms`
- `deadline_ms`

因为 mailbox 常承担：

- 是否接单
- 是否接手
- 是否保留占用

---

## 5. EventBus Notification Envelope

eventbus 是广播通知层。

### 5.1 Notification Message

推荐最小字段：

- `header`
- `topic`
- `audience_scope`
- `payload`

其中：

- `channel_kind = eventbus`
- `delivery_mode = notify | stream`

### 5.2 EventBus 适合的语义

- failure broadcast
- digest published
- task/dispatch/topic 状态变化通知
- 外部 subscriber 监听

### 5.3 EventBus 不要求强握手

eventbus 默认：

- 可以只有 delivered/published 语义
- 不强制每个 subscriber 单独 ack
- 订阅者若需要更强语义，应通过 subscription + cursor + 可选 ack 实现

---

## 6. 最小握手状态机

握手状态不要求三类通道完全一样，但建议共享最小状态集合。

推荐最小集合：

- `created`
- `delivered`
- `accepted`
- `rejected`
- `expired`
- `completed`
- `failed`

---

## 6.1 RPC 的状态子集

RPC 建议常用状态：

- `created`
- `delivered`
- `accepted`
- `rejected`
- `completed`
- `failed`
- `expired`

推荐理解：

```text
created
  -> delivered
  -> accepted | rejected | expired
  -> completed | failed
```

---

## 6.2 Mailbox 的状态子集

Mailbox 建议常用状态：

- `created`
- `delivered`
- `accepted`
- `rejected`
- `completed`
- `expired`

典型场景：

- handoff 消息已送达
- recipient 接受/拒绝
- 交接完成
- 超时失效

---

## 6.3 EventBus 的状态子集

EventBus 建议常用状态：

- `created`
- `delivered`
- `completed`
- `expired`

必要时可以扩展：

- `failed`

但通常不要求：

- `accepted`
- `rejected`

因为 eventbus 默认不是点对点握手通道。

---

## 7. Ack / Lease / Retry 的边界

### 7.1 Ack

Ack 是可选，不是统一强制。

推荐：

- RPC：可选 ack，但通常 response 本身已经承担确认
- Mailbox：更适合显式 ack
- EventBus：默认不要求逐 subscriber ack

### 7.2 Lease

Lease 更适合：

- Mailbox
- claim / handoff / ownership 协商

不建议默认强加到 eventbus broadcast。

### 7.3 Retry

Retry 属于 transport / delivery policy，不属于业务真相。

因此：

- 可在消息头里保留 retry hint
- 但真正的失败/重试事实仍应落成 operation/event

---

## 8. 与内部真源的连接

外部消息被处理后，必须进入下面两种之一：

### 8.1 Materialize to Operation

适用于：

- RPC request
- mailbox directed request
- 外部控制动作

### 8.2 Materialize to Event

适用于：

- 已发生的外部事实通知
- 外部回执
- 外部状态变化

核心规则：

```text
external envelope != internal truth
```

外部 envelope 只是 ingress 格式。

---

## 9. 当前冻结的硬规则

1. 三类外部通道共用公共消息头
2. `protocol_version` / `trace_id` / `sender_id` 为核心字段
3. `sequence?` 只是发送侧辅助序，不替代 `event.sequence`
4. RPC 做 request/response ingress
5. Mailbox 做定向投递与握手
6. EventBus 做广播通知与 fan-out
7. Ack/lease 是按通道裁剪的可选能力
8. 所有外部消息最终必须正规化为 operation 或 event