# 19 Minimal Subscription Registry Framework

本文档定义 `fin` 的最小 subscription framework。

目标不是一次性设计完整消息总线，而是先冻结：

1. 谁是 producer
2. 谁是 consumer
3. subscription 最小对象是什么
4. 最小 filter / delivery / lifecycle 怎么做
5. debug / channel / harness 如何共用同一架构

---

## 1. 核心判断

`fin` 的 subscription framework 不是独立于 event 模型之外的新系统，而是：

```text
append-only event truth
  -> subscription registry
  -> consumer-specific delivery
```

冻结判断：

1. 事件真源先存在，subscription 只是消费组织层
2. subscription 不创造业务语义
3. debug、Web、CLI、harness、channel bridge 共享同一 subscription 框架
4. 当前先做最小本地框架，后续再扩到 cross-process / cross-network

---

## 2. 最小对象

### 2.1 Producer

producer 是事件产生者。

当前最小 producer：

- runtime
- provider gateway
- tool executor
- orchestrator
- replay engine

producer 只负责：

- append event
- 提供必要 refs / trace / severity / visibility

producer 不知道有多少 consumer。

### 2.2 Consumer

consumer 是事件订阅者。

当前最小 consumer：

- projector
- Web debug stream
- CLI tail/filter
- harness assertion runner
- channel bridge

consumer 只负责：

- 注册订阅
- 接收匹配事件
- 自己构建 view / payload / assertion

### 2.3 Subscription

subscription 是 consumer 与事件流之间的绑定对象。

最小字段建议：

- `subscription_id`
- `consumer_id`
- `consumer_kind`
- `scope`
- `filter`
- `delivery_mode`
- `cursor`
- `ack_policy?`
- `lease_ttl_ms?`
- `status`
- `created_at`

---

## 3. scope 与 filter

### 3.1 Scope

scope 表示“在哪个事实范围内订阅”。

当前建议最小支持：

- `global`
- `session`
- `task`
- `trace`
- `worker`
- `event_family`

例如：

- 某个 Web 页面订阅 `task=task-123`
- 某个 channel bridge 订阅 `event_family=provider.*`
- 某个 harness 用 `trace=trace-456`

### 3.2 Filter

filter 表示“在 scope 内还要筛什么”。

当前建议最小支持：

- `event_types`
- `severity >=`
- `debug_visibility <=`
- `refs match`
- `producer source`

不做：

- 复杂脚本表达式
- 用户自定义 DSL
- 跨事件聚合条件语言

这些留到后续。

---

## 4. Delivery 模型

当前最小 delivery 只需要支持两类：

### 4.1 Pull / Poll

适用于：

- CLI
- Web debug MVP
- harness

特征：

- consumer 带 cursor 拉取
- 最简单、最稳定
- 适合 M1/M2 初期

### 4.2 Push / Stream

适用于后续：

- WS
- channel bridge
- remote subscriber

当前先冻结接口概念，不强行进入实现。

## 4.3 Ack 是可选能力

subscription 可以带 `ack_policy`，但 **不是所有 subscription 都必须 ack**。

当前建议：

- debug / Web / CLI pull reader：默认不要求 ack
- harness / channel bridge / remote delivery：可选启用 ack

推荐理解：

```text
cursor = 读到哪里
ack    = 明确确认“已成功处理到哪里”
```

二者相关，但不是同一个概念。

---

## 5. Cursor 语义

subscription 默认必须有 cursor，避免“看最新”导致事实丢失。

当前建议：

- cursor 以 event sequence 为主
- 允许同时记录 last_event_id
- 不使用 task_sequence 作为默认 cursor 真源

最小行为：

1. consumer 注册时可指定起点
2. pull 时返回 `events + next_cursor`
3. consumer 自己决定何时提交已消费 cursor

这让：

- debug 页面可继续滚动
- harness 可精确断言
- channel bridge 可避免重复推送

也就是说：

- `event.sequence` 是默认消费锚点
- `task_sequence` 若存在，只用于业务局部观察
- subscription registry 不应把 task 局部序当作底层传输 cursor

## 5.1 Lease TTL 语义

subscription 可选带 `lease_ttl_ms`，用于声明：

- 这个 subscription 的占用/活性保留多久
- 超时后 framework 可以回收或标记失活

这主要服务于：

- channel bridge
- remote subscriber
- 长时运行 consumer

但对本地短生命周期 debug pull consumer，不强制要求。

---

## 6. Consumer kind 最小集合

建议冻结：

- `projector`
- `web_debug`
- `cli`
- `harness`
- `channel_bridge`
- `diagnostics_exporter`

好处：

- 同一类 consumer 可共享默认 filter / delivery
- debug 与 channel 不再是分裂的两套系统

---

## 7. 最小 registry 职责

subscription registry 当前只承担四件事：

1. 注册 subscription
2. 存储 subscription metadata
3. 按 scope/filter 查询匹配事件
4. 返回 delivery cursor

registry 不负责：

- event append
- projection 业务语义
- provider routing
- task orchestration

---

## 8. Debug / Channel / Harness 如何共用

### Debug

订阅：

- `trace=*`
- `task=*`
- `provider.*`
- `progress.*`

### Channel bridge

订阅：

- `provider.failed.*`
- `digest.finalized`
- `recovery.started`

### Harness

订阅：

- 指定 trace/task
- 断言需要的 event family

三者只是：

- scope 不同
- filter 不同
- delivery mode 不同

但 registry 与 event truth 相同。

---

## 9. 生命周期

subscription 最小生命周期建议：

- `created`
- `active`
- `paused`
- `closed`

规则：

1. closed subscription 不再接收新事件
2. paused 只是不继续 delivery，不删除 cursor
3. consumer 崩溃恢复时可用 cursor 继续

---

## 10. M1 / M2 的最小落地顺序

### M1 / 当前审阅目标

只冻结：

- subscription object
- scope/filter/cursor/delivery 概念
- consumer kind
- registry owning layer

### M2

先做：

- 本地内存 registry
- file-backed cursor/state
- CLI/Web pull delivery

### M3

再做：

- WS push
- channel bridge delivery
- remote subscribers
- cross-process / cross-network fan-out

---

## 11. owning layer

此框架的 owning layer 建议为：

- `docs/architecture/`
- future `rust/crates/registry`
- future `rust/crates/debug-server`
- future `rust/crates/transport-http`

runtime / provider / web 不应各自复制第二套 subscription 逻辑。

---

## 12. 当前冻结的硬规则

1. subscription 不是第二事件总线，只是事件消费组织层
2. producer 不关心 consumer
3. consumer 不能回写事实流
4. 当前最小 delivery 先以 pull/poll 为主
5. subscription 默认必须带 cursor
6. `ack_policy` 与 `lease_ttl_ms` 是可选能力，不是所有 subscription 必需
7. subscription cursor 默认跟随 `event.sequence`，而不是 `task_sequence`
8. debug / CLI / harness / channel bridge 共用同一 subscription 架构
