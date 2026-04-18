# 31 Peer Plane, Execution Plane, and Binding Model

本文档冻结 `fin` 的 peer 接入与执行分层。

本文档回答五个问题：

1. 为什么 peer 协议和执行协议必须拆开
2. 哪些东西属于 `peer plane`
3. 哪些东西属于 `execution plane`
4. capability / agent / gateway 三类 peer 各自怎么走执行协议
5. 握手、lease、presence、binding 的最小关系是什么

---

## 1. 核心结论

`fin` 后续应采用双平面模型：

```text
Peer Plane
  = discover / register / auth / lease / heartbeat / health / capability advertisement

Execution Plane
  = job / task / message execution
```

冻结原因：

1. 发现与接入是通用能力
2. 执行语义会随 `peer_kind` 不同而不同
3. 如果把两者耦合，会让 capability/agent/gateway 无法复用统一接入框架

补充冻结：

4. peer 生命周期观察不能只看一个 `connected`
5. 至少要拆成 `runtime / upstream connectivity / binding` 三条状态线

---

## 1.1 三条状态线（必须分开观察）

后续所有 peer（包括本地 qqbot / channel gateway / remote peer）统一按三条状态线观察：

### A. Runtime State

回答：

- peer 进程/服务是否已启动
- 本地 control plane 是否能访问和管理它
- 它当前是否 `starting / ready_local / unhealthy / stopped`

这是 **本地可达、可管理** 的状态线。

### B. Upstream Connectivity State

回答：

- peer 是否已经完成和外部 server / platform 的配对或鉴权
- 是否已经真正可以对外收发
- 当前是 `local_only / auth_required / authenticating / connected / expired / reconnecting`

这是 **peer 与上游服务** 的状态线。

### C. Binding State

回答：

- peer 是否已经绑定到 `system agent / project agent / session route`
- 当前绑定是否有效
- 是否需要重新配对或重绑

这是 **fin 内部路由归属** 的状态线。

冻结结论：

```text
runtime_state
!= connectivity_state
!= binding_state
```

例如：

- upstream auth 成功 ≠ 已绑定 fin agent
- 已绑定 fin agent ≠ upstream 仍然有效
- peer 本地存活 ≠ 已具备外部连通能力

---

## 2. Peer Plane

`peer plane` 是所有 peer 共用的控制面。

它负责：

- discover
- register
- auth
- lease open / renew / close
- heartbeat
- health state
- capability advertisement
- reconnect / disconnect

不管 peer 是：

- capability peer
- agent peer
- channel gateway

这些能力都应该尽量共用。

---

## 3. Execution Plane

`execution plane` 是 peer 真正承接业务动作的那一层。

这里不再按 “连没连上” 区分，而按 `peer_kind` 分流。

### 3.1 Capability Peer

走标准 job 语义：

```text
job.submit
  -> job.accepted
  -> job.progress*
  -> job.completed | job.failed
```

适合：

- 标准 schema 能力
- 可预测 side effect
- 不需要自主上下文闭环的任务

### 3.2 Agent Peer

走 task/binding 语义：

```text
binding.open
  -> task.assign
  -> agent.progress*
  -> artifact.publish*
  -> task.completed | task.failed
```

适合：

- 需要 agent loop 的任务
- 需要局部 session/task ledger 的协作
- 需要多轮推理与工具闭环

### 3.3 Channel Gateway

走 message/delivery 语义：

```text
message.ingest
message.emit
delivery.ack
delivery.failed
```

适合：

- 输入接入
- 输出渲染
- delivery/report

---

## 4. Peer Descriptor

所有 peer 至少需要一个统一 descriptor。

推荐最小字段：

- `peer_id`
- `node_id`
- `peer_kind`
  - `capability`
  - `agent`
  - `channel_gateway`
- `label`
- `protocol_version`
- `listen_addr`
- `auth_mode`
- `presence_state`
- `health_state`
- `capability_catalog`
- `project_scope`
- `runtime_home`
- `workdir_roots`
- `supports_streaming`
- `supports_session_binding`
- `supports_agentic_execution`

冻结结论：

1. `peer_kind` 是第一层路由
2. `supports_session_binding` 与 `supports_agentic_execution` 不能混为一谈
3. 后续 system agent 的路由应优先看 descriptor，而不是靠 prompt 猜

---

## 5. Capability Catalog

system agent 在 peer discover 之后，首先看的不是连接细节，而是它能干什么。

因此 capability catalog 必须成为统一真源。

推荐每个 capability 至少包含：

- `capability_id`
- `title`
- `kind`
- `description`
- `input_schema_summary`
- `output_schema_summary`
- `side_effects`
- `supports_stream`
- `supports_async_job`
- `supports_cancel`
- `supports_resume`
- `deterministic`
- `stateful`
- `agentic`
- `gateway_only`

冻结结论：

1. `capability peer` 主要通过 capability catalog 路由
2. `agent peer` 也应暴露高层能力摘要
3. `channel gateway` 应暴露支持的 ingress/egress modes

---

## 6. Presence 和 Binding 的关系

### 6.1 Presence

presence 回答：

- peer 是否在线
- peer 是否健康
- peer 是否可连接
- peer 是否 busy / idle / degraded

presence 依赖：

- heartbeat
- lease
- event freshness
- progress freshness

### 6.2 Binding

binding 回答：

- 当前哪个 system session/task 绑定到了哪个 agent peer
- 这个绑定是否仍然有效
- 断线后是否还能 rebind

冻结结论：

```text
Presence = existence + liveness
Binding  = ownership + assignment
```

两者不能混。

---

## 7. Lease 模型

后续 peer 协作不应只看有没有 ping，而应基于租约。

推荐：

- `lease_id`
- `holder_id`
- `peer_id`
- `issued_at`
- `expires_at`
- `ttl_ms`
- `ack_policy`

建议 system agent 观测 peer 存活时，不只看 heartbeat，还看：

1. heartbeat ack
2. event seq 是否前进
3. progress 是否更新
4. 连接是否存活

冻结结论：

- lease 是 presence 的控制结构
- binding 依赖 lease，但不等于 lease

---

## 8. 握手四步

最小握手建议固定为：

```text
discover
  -> authenticate
  -> lease establish
  -> bind / execute
```

### 8.1 discover

- 获取 descriptor
- 获取 capability catalog
- 确认 protocol/capability 是否兼容

### 8.2 authenticate

- 双方身份确认
- 确认允许接入的 system/peer

### 8.3 lease establish

- 建立 heartbeat/ttl/grace

### 8.4 bind / execute

- capability peer：准备 submit job
- agent peer：建立 binding 后 assign task
- gateway：建立 ingress/egress routing

---

## 9. M1 最小控制动作

peer plane 推荐最小动作：

- `peer.hello`
- `peer.describe`
- `peer.capabilities`
- `peer.status`
- `lease.open`
- `lease.renew`
- `lease.close`
- `heartbeat.ping`
- `heartbeat.pong`
- `peer.disconnect`

execution plane 推荐最小动作：

### capability peer

- `job.submit`
- `job.accepted`
- `job.progress`
- `job.result`
- `job.failed`

### agent peer

- `binding.open`
- `binding.close`
- `task.assign`
- `task.status_probe`
- `task.progress`
- `artifact.publish`
- `task.completed`
- `task.failed`

### channel gateway

- `message.ingest`
- `message.emit`
- `delivery.report`

---

## 10. system agent 的路由原则

冻结建议：

### 10.1 标准确定性任务

优先路由到 `capability peer`

例如：

- grep / scan
- build / test
- search / retrieve
- file stat / analyzer

### 10.2 需要自主上下文闭环的任务

路由到 `agent peer`

例如：

- 多轮工具使用
- 子任务长期执行
- 本地 ledger/digest/context rebuild

### 10.3 输入输出转换

路由到 `channel gateway`

例如：

- 外部聊天输入接入
- 消息发送
- 渲染/投递反馈

---

## 11. 对本地推理系统的影响

当前本地 runtime 已经有：

- context assembly
- multi-step inference loop
- tool dispatch
- progress/note/digest/session truth

后续要与 peer 模型对齐时，必须补：

1. `system` 与 `project` 两类 agent 的 prompt/context 明确分层
2. context 中引入 peer/presence/binding 视图
3. tool catalog 中引入 peer invocation / capability submit / agent assign 的抽象
4. control block 中允许输出 peer routing / binding confidence / execution target hints

这些属于后续本地推理系统要增加的内容，但不改变本文件冻结的 peer plane / execution plane 边界。

---

## 12. 一句话冻结

`fin` 的 remote/local 协作模型应冻结为：

> 所有本地/远端协作对象都先归一为 `peer`；peer 共用 discover/auth/lease/heartbeat/health 的 `peer plane`，再按 `capability peer / agent peer / channel gateway` 在 `execution plane` 中分流具体 job、task 与 message 语义。
