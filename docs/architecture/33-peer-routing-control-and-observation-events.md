# 33 Peer Routing Control and Observation Events

本文档冻结 `fin` 在 peer plane 真正接线前的 **routing control skeleton** 与 **peer observation event skeleton**。

本轮目标不是实现 remote peer 执行，而是先把：

1. 路由判断作为结构化 runtime 事实落盘
2. peer/topology/binding/daemon 的观察结果作为事件发出
3. 让 projection / session artifacts / status probe 都能消费同一份 routing truth

---

## 1. 核心结论

在 peer plane 未接入前，runtime 仍然必须输出一份最小的：

- `PeerRoutingFeedback`
- `peer.discovered / binding.opened / daemon.state_observed`
- `peer.routing_feedback_recorded`

冻结原因：
- 否则 routing 仍然只存在于 prompt 猜测，不是 framework truth
- 后续接 peer registry / daemon / lease 时，无法平滑升级到真正控制面

---

## 2. PeerRoutingFeedback

`PeerRoutingFeedback` 当前冻结的最小字段：

- `origin`
- `route_target_kind`
- `route_target_peer_id`
- `route_target_capability_id`
- `route_confidence`
- `requires_peer_routing`
- `requires_daemon_ensure`
- `requires_binding`
- `requires_rebind`
- `placeholder`
- `missing_facts[]`
- `reason`

它不是 provider 控制块的一部分，而是并行的 runtime control artifact。

---

## 3. 当前 M1 的路由判断规则

当前只是最小启发式，不是最终 peer router：

### 3.1 system / system_agent
- 若仍是 local-only placeholder，则固定 `local_runtime`
- 若后续有真实 peer catalog，再升级为 agent/capability 路由

### 3.2 project / worker / project_agent
- 默认走 `bound_local_agent`
- 重点是保持执行闭环，不抢用户 session truth

### 3.3 capability_router / peer_router
- 优先看 capability catalog
- 没 catalog 就保持 `undecided`

### 3.4 channel_gateway
- 默认路由到 `system_agent`

冻结规则：
- 当前 heuristic 可以保守，但必须结构化输出
- 不允许把“路由决定”继续埋在 note 文本里

---

## 4. Observation Events

当前先冻结三类观察事件：

1. `peer.discovered`
2. `binding.opened`
3. `daemon.state_observed`

注意：
- 这些在 M1 可以是 **observation event**，不一定代表真实远端握手已经发生
- 若来自 local-only placeholder，payload 必须带 `placeholder=true`
- 不允许把 placeholder 事件伪装成真实 remote lease/auth/connect 事实

---

## 5. 落盘规则

当前 routing truth 必须同时进入：

- `runtime/current/current_peer_routing_feedback.json`
- `sessions/.../routing/latest.json`
- `ExecutionNote.peer_routing_feedback`
- `DigestRecord.peer_routing_feedback`
- `peer.routing_feedback_recorded` event
- `ProjectionView.latest_route_*`

这样做的目的：
- session/debug/status probe 共用同一份 routing 真源
- 后续 Web / channel 观察时不需要再自己推断

---

## 6. 当前边界

本轮仍然 **没有**：

- 真正 remote peer execution
- lease/open/renew/close
- auth/connect/reconnect
- daemon.ensure_peer IPC
- peer.list / capability.invoke / agent.assign 真执行

所以当前状态应理解为：

```text
peer-aware prompt/context  +  routing truth  +  observation events
!= full peer execution plane
```

---

## 7. 验收标准

1. runtime closure 结束后有 `PeerRoutingFeedback`
2. routing 已落到 runtime/session artifact
3. event 链中有 `peer.routing_feedback_recorded`
4. 若 context 含 peer block，则会发 `peer.discovered` / `binding.opened`
5. projection 能消费最新 route kind / peer / confidence

