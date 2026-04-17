# 22 M1 Minimal Agent Core Module Cut

本文档定义 `fin` 在进入实现前，M1 最小 agent core 应如何切分模块。

目标不是展开所有未来能力，而是回答四件事：

1. M1 最小 agent core 到底包含哪些模块
2. 这些模块的 owning boundary 是什么
3. 哪些是必须先做的最小垂直切片
4. 当前 `rust/crates/*` 应如何承接这些边界

---

## 1. 核心判断

M1 的最小 agent core 不是完整多 agent 系统，而是：

```text
single runtime
  + role/provider policy
  + inference operation -> provider event slice
  + recording
  + debug/projection
  + subscription-ready read boundary
```

冻结判断：

1. M1 先做单 runtime、单 worker、单 closure 可闭环
2. agent core 先围绕 execution truth 与 debug truth 切分
3. role / worker / provider policy 必须先有边界，但不追求完整调度
4. 外部通道、RPC、mailbox、eventbus 先保留接口，不进入 M1 核心实现

---

## 2. M1 最小 agent core 的七个模块

当前建议把 M1 核心切成七块：

### A. Identity / Role / Runtime Policy

负责：

- `AgentId`
- `RoleProfile`
- `WorkerRuntime`
- provider path / strategy policy
- timeout tier / stream policy

这是“谁在执行、默认怎么执行”的边界。

### B. Inference Operation Builder

负责：

- 把用户输入 / task 状态 / context view 组装成 inference operation
- 只产出请求意图，不消费响应语义

这是 operation 真源入口。

### C. Provider Gateway Slice

负责：

- 接收 inference operation
- 调 LiteLLM
- 产出 provider 事件

这是 provider 执行切片，不拥有 task 业务语义。

### D. Recording Slice

负责：

- progress block
- execution note
- digest
- closure 落盘

这是 agent 持续输出与压缩的最小骨架。

### E. Event Store + Projection Slice

负责：

- append-only event 写入
- current projection
- latest snapshot

这是 debug / Web / CLI 的基础读取层。

### F. Debug Entry Slice

负责：

- CLI debug entry
- Web debug MVP
- 当前 projection / event stream 可视化

这是最小可观测入口。

### G. Subscription-ready Read Boundary

负责：

- 给后续 subscription registry 留稳定读取接口
- 先不做完整 push bus
- 但当前 event/projection 不应把自己写死成单一 UI 读取模型

这是给 M2 的读侧接口准备。

---

## 3. M1 当前不进入的模块

这些模块必须有接口/预留，但不进入 M1 主实现：

- multi-worker scheduler
- health monitor / timeout escalation 全套
- external RPC transport
- mailbox / eventbus runtime
- cross-process delivery
- cross-network cluster topology
- replay/fault injection 完整平台

原则：

```text
先冻结边界
后做 transport / scheduler / cluster
```

---

## 4. 模块之间的最小主流水线

M1 的最小主流水线应固定为：

```text
role/runtime policy
  -> build inference operation
  -> provider gateway execution
  -> provider events
  -> progress/note/digest
  -> append-only event store
  -> projection/snapshot
  -> cli/web debug
```

这条主线里：

- 不引入 mailbox/eventbus
- 不引入外部 RPC
- 不引入复杂 scheduler

---

## 5. 模块间的硬边界

### 5.1 Role/Policy 不直接发网络请求

role / worker / provider policy 只决定：

- 用哪个 provider path
- 用哪个 model
- 用什么 timeout/stream policy

但不自己发 LiteLLM 请求。

### 5.2 Provider Slice 不推进 Task 状态机

provider gateway 只：

- 执行
- 发 provider 事件

不直接写 task closed / task failed 等业务语义。

### 5.3 Recording Slice 不决定 Provider 行为

progress / note / digest 只消费事实与模型反馈，不反向决定 provider transport。

### 5.4 Debug Slice 不补真相

Web / CLI 只消费：

- event
- projection
- snapshot

不允许补第二份 runtime 语义。

---

## 6. 对应到当前 crate 的建议承接

结合当前 `rust/crates/*`，建议映射如下：

### `fin-contracts`

承接：

- operation envelope
- event envelope
- progress / note / digest / projection
- 未来 subscription / message envelope 类型真源

### `fin-config`

承接：

- user/system config
- role/provider policy mapping
- runtime/system defaults

### `fin-provider`

承接：

- provider policy resolution
- LiteLLM gateway request/response boundary
- provider operation -> provider event slice

### `fin-runtime`

承接：

- single runtime closure
- inference operation build
- recording push
- runtime current state

### `fin-debug-server`

承接：

- projector
- snapshot
- Web debug data source

### `fin-cli`

承接：

- home-init
- runtime-demo
- debug-projection
- web-debug
- 后续最小 agent core smoke 入口

### `fin-registry`

当前先不承接完整订阅与 cluster，只作为未来：

- runtime/worker registry
- subscription registry
- remote discovery

### `fin-orchestrator`

当前先保持轻量：

- task state machine
- transition rule

先不要在 M1 塞入完整多 agent 调度。

### `fin-transport-http`

当前只预留 future ingress：

- RPC / WS / remote transport

不抢 M1 核心。

---

## 7. M1 第一批必须打通的最小切片

如果只选一条最小切片，我建议固定为：

### Slice-1

```text
role/provider policy
  -> inference operation
  -> LiteLLM request (future real)
  -> provider events
  -> progress/note/digest
  -> event/projection
  -> web debug
```

当前哪怕 provider 还没切到真实 LiteLLM，也必须保证：

- operation 边界已存在
- provider event family 已存在
- recording 已存在
- debug 已存在

这样切换真实 provider 时不会返工主框架。

---

## 8. M1 之后的自然扩展顺序

在这份模块切分下，M1 后续应该按下面顺序扩展：

### M1.x

- 真实 LiteLLM gateway
- provider event normalization
- role/provider policy 更完整

### M2

- subscription registry
- mailbox/eventbus
- external ingress
- richer CLI/Web debug

### M3

- multi-worker
- cluster / cross-process
- cross-network RPC
- remote event consumers

---

## 9. 当前冻结的硬规则

1. M1 agent core 先围绕 execution truth + debug truth 切分
2. role/provider policy、provider slice、recording、projection/debug 必须同时存在边界
3. provider slice 不推进业务 task 语义
4. debug slice 不补真相
5. mailbox/eventbus/RPC 先保留接口，不进入 M1 agent core 主实现
6. 当前 crate 承接优先服从边界，而不是为了“方便”把逻辑塞进 runtime 单体