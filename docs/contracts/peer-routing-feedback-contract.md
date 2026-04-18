# Peer Routing Feedback Contract

`PeerRoutingFeedback` 是 `fin` 在 peer plane 落地前的最小 routing control artifact。

## 字段

- `origin`: 当前生成器版本，例如 `runtime_peer_router_v1`
- `route_target_kind`: 例如 `local_runtime` / `bound_local_agent` / `agent_peer` / `capability_peer` / `system_agent` / `undecided`
- `route_target_peer_id`: 候选 peer id
- `route_target_capability_id`: 候选 capability id
- `route_confidence`: 0-100
- `requires_peer_routing`: 是否需要走真正 peer router
- `requires_daemon_ensure`: 是否需要 daemon 补齐 peer 生命周期
- `requires_binding`: 是否需要 task/session binding
- `requires_rebind`: 是否需要恢复或重绑
- `placeholder`: 当前判断是否建立在 local-only placeholder 上
- `missing_facts[]`: 当前缺少的事实前提
- `reason`: 路由理由摘要

## 落盘位置

- `runtime/current/current_peer_routing_feedback.json`
- `sessions/.../routing/latest.json`
- `ExecutionNote.peer_routing_feedback`
- `DigestRecord.peer_routing_feedback`
- `peer.routing_feedback_recorded` event

## 兼容性规则

- 新字段必须 `serde(default)`
- 未知 route kind 不能导致消费方崩溃
- placeholder 必须显式为 `true`，不能靠消费者猜测

