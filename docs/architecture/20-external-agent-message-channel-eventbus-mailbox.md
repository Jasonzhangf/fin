# 20 External Agent Message Channel, EventBus, and Mailbox

本文档冻结 `fin` 后续对外部进入 agent 的消息通道边界。

当前结论不是立即实现完整通信层，而是先明确：

1. `eventbus` 与 `mailbox` 放在 M1 最小 agent 部分完成之后
2. 它们只承担同步/异步通知与握手机制
3. 它们不能取代 operation / event 真源模型
4. 它们不能自带第二套 task/session 业务语义
5. 需要明确内部流水线、外部入口、多 agent 通信分别走哪条通道
6. 外部 agent 后续优先考虑 RPC 方式，支持跨机器/跨网段，但当前阶段只保留接口，不做实现

---

## 1. 核心判断

后续外部进入 agent 的通信层建议拆成两类：

```text
external ingress
  -> mailbox
  -> eventbus
  -> runtime operation / event truth
```

冻结判断：

1. mailbox 是面向 agent/worker 的收件箱语义
2. eventbus 是面向通知/广播/分发的通道语义
3. 二者都只是通信与同步机制，不是事实真源
4. 真正进入系统的业务动作仍然要落成 operation / event
5. 外部 agent 的跨机器调用后续优先通过 RPC ingress 接入

推荐总图：

```text
external caller / remote agent / channel bridge
  -> rpc ingress / mailbox / eventbus ingress
  -> handshake / accept / normalize
  -> operation
  -> internal runtime pipeline
  -> append-only events
  -> subscription registry
  -> debug / web / cli / harness / channel / remote agents
```

---

## 1.1 内部走什么，外部走什么

这是当前必须冻结的边界：

### 内部流水线走

- `operation`
- internal runtime pipeline
- append-only `event`
- projection / subscription

也就是内部系统真源永远围绕：

```text
operation -> runtime push -> event -> projection/subscription
```

### 外部入口走

- `rpc ingress`
- `mailbox`
- `eventbus`
- handshake / notify / delivery state

外部入口不直接改内部状态，而是：

```text
external message
  -> rpc/mailbox/eventbus
  -> normalize
  -> operation/event
```

---

## 1.2 多 agent 通信走什么

多 agent 通信分两类：

### A. 同一 runtime / 同一集群内部协作

优先走：

- operation + event
- subscription 消费
- 必要时 mailbox

也就是：

- 协作事实走 event
- 定向请求走 operation
- 定向握手/投递走 mailbox

### B. 外部 agent / 跨进程 / 跨网段协作

优先走：

- rpc（请求/响应、定向调用）
- mailbox（定向）
- eventbus（广播/通知）

然后再把接收到的动作正规化进本地 runtime：

- 请求 -> operation
- 已发生事实 -> event

因此多 agent 通信不能绕过本地 operation/event 真源。

### 1.3 为什么预留 RPC

对外部 agent，尤其是：

- 不同机器
- 不同网段
- 不同 runtime 进程
- 需要明确 request/response 语义

RPC 更适合作为主入口，因为它天然适合：

- 定向调用
- 能力协商
- 超时控制
- 错误返回
- 鉴权
- 版本协商

当前阶段已落地 v1 Agent RPC ingress；后续再扩展更完整的 RPC runtime。

### 1.4 Agent RPC v1 ingress

fin 的跨机器 agent 协作入口是专用 Agent RPC，不复用 WebUI / QQBot / mobile debug WS。

最小冻结：

1. system agent 通过 `runtime.agent_network.enabled=true` 启动 Agent RPC listener。
2. v1 鉴权只支持 Bearer Lease，token 必须来自 `token_env` 或 `token_file`。
3. 远端 primary agent 用 `POST /agent/v1/handshake` 注册 `machine.agentname`。
4. 在线状态通过 `POST /agent/v1/heartbeat` 续租，system agent 通过 `GET /agent/v1/agents` 枚举。
5. 定向协作消息通过 `POST /agent/v1/mailbox/send` 写入 runtime durable mailbox。
6. 所有成功入口必须落到 runtime agent identity / presence / peer registry / mailbox truth；UI 只消费这些事实。
7. 测试 harness 必须覆盖完整生命周期与错误矩阵：连接不可达、连接中断、丢失心跳变 offline、恢复心跳变 online、执行失败回报、鉴权失败、握手身份错误、subagent 拒绝、project 缺 project_id、重复在线注册、未知/过期 lease、未知目标 mailbox、结构化 route/body 错误。
8. system agent 的 project agent 列表来自静态 startup config 与动态 `runtime/agents/project_agents.json` 合并结果；subagent 不进入跨 agent 网络发现。
9. 动态 project agent 增删查只允许写 `runtime/agents/project_agents.json` 这一份控制面配置；本地 project agent 初次 add 时自动分配 endpoint 端口并持久化，后续启动不得重新漂移端口。
10. WebUI / QQBot / Android 等 channel adapter 默认只连 `system_agent`；project agent listener 允许显式连接或被 Agent RPC 协作使用，但不作为 channel 默认入口。

---

## 2. Mailbox 的定位

mailbox 适合：

- 定向发送给某个 agent / worker
- 等待接收确认
- 持有临时握手状态
- 承担同步请求/响应或异步投递入口

mailbox 不负责：

- 全局广播
- 事件事实存储
- projection 真源
- task 状态机推进

推荐理解：

```text
mailbox = addressed delivery + handshake state
```

### 2.1 Mailbox 适合的场景

当前建议 mailbox 只承担：

- agent A -> agent B 的定向消息
- 请求接收/拒绝
- lease / claim / handoff 握手
- 需要明确 recipient 的同步或异步投递

不建议 mailbox 承担：

- 全局事件广播
- timeline 真源
- debug 主事实流

---

## 3. EventBus 的定位

eventbus 适合：

- 广播通知
- 多 consumer 扇出
- 状态变化通知
- 外部系统监听

eventbus 不负责：

- 覆盖 event truth
- 直接变成业务事实库
- 替代 subscription registry

推荐理解：

```text
eventbus = notification fan-out layer
```

### 3.1 EventBus 适合的场景

当前建议 eventbus 只承担：

- 状态通知
- topic / dispatch / digest / failure 广播
- 多个 consumer 同时监听
- 外部系统只读订阅

不建议 eventbus 承担：

- 定向请求-响应握手
- 事实真源存储
- task 控制状态机

---

## 4. 同步 / 异步边界

### 4.1 同步消息

同步消息适合：

- 握手
- 请求被接受/拒绝
- 短路径协商
- 需要明确 response 的控制动作

### 4.2 异步消息

异步消息适合：

- 通知
- 任务派发提醒
- 状态更新
- 外部 subscriber 监听

无论同步还是异步，最后进入系统都应转成：

- operation
- event

而不是只停留在 mailbox/eventbus 临时通道里。

---

## 4.3 内外边界的明确划分

建议把边界冻结成下面这组：

### 内部边界（inside runtime truth）

内部只认：

- operation
- event
- projection
- subscription

这是系统真源边界。

### 外部边界（outside runtime truth）

外部只负责：

- transport
- addressing
- notify
- handshake
- delivery bookkeeping

这包括：

- rpc ingress
- mailbox
- eventbus
- future transport-http / ws / remote connector

### 关键原则

1. 外部通道不直接写 task/session 真相
2. 内部 runtime 不直接依赖外部 transport 细节
3. 内外之间只能通过 normalize/materialize 连接

即：

```text
outside transport layer
  -> normalize/materialize
  -> inside runtime truth
```

---

## 4.4 RPC 与 mailbox/eventbus 的关系

三者不是互斥替代关系，而是职责不同：

### RPC

适合：

- 外部 agent 请求本机 agent 执行某个动作
- 明确 request/response
- 跨机器定向调用
- 需要返回 accepted/rejected/result handle

### Mailbox

适合：

- 已知 recipient 的投递
- 握手
- claim / handoff / lease 协商
- 轻量异步或半同步定向消息

### EventBus

适合：

- 广播通知
- 多方订阅
- 状态变化扇出

推荐关系：

```text
rpc = remote request/response ingress
mailbox = addressed delivery + handshake
eventbus = notification fan-out
```

---

## 5. 握手机制

mailbox / eventbus 当前冻结的握手目标：

1. 确认消息到达
2. 确认接收方身份/实例
3. 确认是否接受处理
4. 为异步后续处理留下 refs / trace / cursor

推荐最小握手字段：

- `message_id`
- `sender_id`
- `recipient_id?`
- `trace_id`
- `protocol_version`
- `timestamp`
- `handshake_state`

对于 RPC，建议额外预留：

- `rpc_method`
- `request_id`
- `deadline_ms?`
- `auth_context?`

### 5.1 建议的最小握手状态

当前建议最小状态机：

- `created`
- `delivered`
- `accepted`
- `rejected`
- `expired`
- `completed`

说明：

- mailbox 更常用 `accepted/rejected`
- eventbus 更常见 `delivered`，不一定需要 `accepted`

因此握手机制不是全通道同构，而是共享最小状态集合。

---

## 6. 与 operation / event 的关系

这是最关键的边界：

### 6.1 mailbox / eventbus 不是事实真源

它们只是通道层。

### 6.2 进入系统的业务动作必须正规化

例如：

- 外部请求 agent 执行推理
  -> `Operation`
- agent 已接受 / 已完成 / 已失败
  -> `Event`

因此：

```text
rpc/mailbox/eventbus
  -> handshake / notify
  -> operation/event materialization
  -> append-only event truth
```

### 6.3 内部流水线不走 mailbox/eventbus

内部主流水线不建议写成：

```text
runtime -> mailbox -> eventbus -> runtime
```

这会引入第二套事实流与调试路径。

内部正确主线应保持：

```text
operation -> runtime push -> event -> subscription/projection
```

mailbox/eventbus 只在以下情况进入：

- 外部 RPC ingress
- 外部进入
- 跨 agent 定向投递
- 广播通知
- 握手补充

---

## 6.4 RPC 只保留接口，不做短期实现

当前阶段冻结：

1. 预留 RPC ingress 的 owning boundary
2. 预留 RPC message / handshake / auth / timeout 字段
3. 不在当前 M1/M2 里实现跨机器 RPC runtime
4. 不让“先做 transport”打断当前最小 agent 核心与 event truth

也就是说：

- 现在先把接口想清楚
- 以后再做 transport / codec / auth / reconnect / retry

---

## 6.5 跨机器分布通信的最低要求

未来进入跨机器实现时，建议默认满足：

1. 每条外部消息必须带 `trace_id`
2. 必须带 `sender_id`
3. 必须带 `protocol_version`
4. 必须能落成 operation 或 event
5. 必须能在 debug 中看到 ingress -> normalize -> materialize 全链路

否则不算可维护的分布式通信。

---

## 7. 为什么先不做完整实现

当前阶段优先级仍然是：

1. 最小 agent 核心
2. provider / debug / event truth
3. subscription framework

只有这些稳定后，再进入：

- external ingress channel
- mailbox
- eventbus
- handshake state

否则会把通信层和业务真源耦合在一起。

---

## 8. 当前冻结的硬规则

1. eventbus / mailbox 留到最小 agent 核心完成后再进入
2. mailbox 只做定向投递与握手
3. eventbus 只做同步/异步通知与 fan-out
4. mailbox / eventbus 不取代 operation / event 真源
5. 外部消息最终必须正规化为 operation 或 event
6. 内部主流水线只走 operation -> event -> subscription/projection
7. 外部入口先走 mailbox/eventbus，再正规化进入内部真源
8. 多 agent 通信默认：定向走 mailbox，广播走 eventbus，事实落盘仍走 operation/event
9. 外部 agent 后续优先考虑 RPC ingress，尤其是跨机器/跨网段场景
10. 当前只保留 RPC 设计接口，不做实现
