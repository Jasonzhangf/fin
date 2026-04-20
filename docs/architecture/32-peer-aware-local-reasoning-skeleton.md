# 32 Peer-aware Local Reasoning Skeleton

本文档冻结 `fin` 在 **真正 peer plane 接入之前**，本地 runtime 推理层必须先补上的最小 peer-aware 骨架。

目标不是现在就实现 remote peer 执行，而是先保证：

1. prompt 已经知道 system/project 的职责差异，并知道 gateway/router 是 peer/component 类型而不是独立 prompt role
2. context 已经有 peer/topology/binding/daemon/channel 的固定位置
3. 当前仍在本地单 runtime 闭环时，也不会把未来 peer 语义彻底写死在 project-only 结构里

---

## 1. 核心结论

在 `peer plane` 真正接入前，本地推理层必须先完成两类升级：

```text
single-runtime local closure
  + peer-aware role prompts
  + peer-aware context block skeleton
= future-compatible local reasoning baseline
```

冻结顺序：

1. 先补 `role/prompt` 语义
2. 再补 `context block` 骨架
3. 再补 `routing/control` 判断
4. 再补 `peer tools + peer events`
5. 最后才接真正的 local/remote peer execution

原因：
- 如果 prompt/context 里没有 peer 位置，后续接 peer plane 时一定会回头重写上下文结构
- 如果 system/project 不先分 role、同时不把 gateway/router 明确为框架组件类型，后续多 agent 协作会退化成“单 prompt 猜路由”

---

## 2. 本轮冻结：两类 prompt role + 多类 peer/component

本地 runtime 当前只保留两类 prompt role：

- `system`
- `project`

同时，框架还要认识这些 **peer/component 类型**，但它们不是独立 prompt role：

- `capability_router` / `peer_router`
- `channel_gateway`
- `capability_peer` / `channel_peer` / `agent_peer`

冻结规则：

### 2.1 system
- 用户入口与总编排者
- prompt 要显式强调：
  - peer discovery / binding
  - daemon-backed recovery
  - session truth ownership

### 2.2 project
- 负责 bound task execution 与项目内闭环
- prompt 要显式强调：
  - system agent 持有用户 session truth
  - 自己回传 progress / artifact / result
  - execution / review / diagnosis / handoff 是同一 project role 内部的 workflow emphasis

### 2.3 peer/component taxonomy
- `capability_router` / `peer_router` / `channel_gateway` 是框架组件类型，不是独立 prompt role
- runtime/context 要显式保留这些对象的 schema 位置
- 路由判断优先依据 descriptor / capability / presence / binding，而不是靠额外 role 文本猜测

---

## 3. 本轮冻结：peer-aware context skeleton

当前 `MinimalContextView` 除原有：

- `control`
- `role_prompt`
- `tools`
- `history`
- `knowledge`
- `project`
- `current_input`

还必须新增：

- `peer`

`peer` block 先作为统一骨架，内部至少预留：

- `topology_summary`
- `active_peer_ids`
- `peers[]`
- `binding`
- `capabilities[]`
- `daemon`
- `channel_routes[]`
- `routing_hints[]`

冻结原因：
- 后续 peer plane 上线后，context rebuild / debug / session artifacts 都需要消费同一个 `peer` block
- 现在先放骨架，可以避免将来拆成第二套上下文真源

---

## 4. M1 local-only fallback 规则

在 peer registry / daemon / lease 尚未接通前，允许 `peer` block 进入 **local-only placeholder mode**。

placeholder 规则：

1. 可以声明“当前处于 local-only M1 mode”
2. 可以挂一个 `local-<worker_id>` 的本地 peer 摘要
3. 可以显式标记：
   - `binding_state=controller_local_only` 或 `local_execution_only`
   - `daemon.supervision_state=not_attached`
4. 不允许假装 remote peer 已真实存在
5. 不允许把 placeholder 当成真实 registry/binding 事实

也就是说：

```text
placeholder = schema anchor + future slot
not placeholder = fabricated peer truth
```

---

## 5. 本轮冻结：assembled prompt 顺序

静态块尽量靠前，连续增长的块靠后。

当前推荐顺序：

1. context summary
2. continuity tail
3. role prompt
4. tools
5. project
6. peer
7. history
8. current input
9. mandatory response contract

冻结规则：
- `history` 放后面，因为它天然会增长
- `project/peer` 先于 `history`，这样模型先看到结构边界和执行位置，再读连续历史

---

## 6. 本轮未实现但已冻结的下一步

### 6.1 control / routing block
后续必须补：
- route target kind
- route confidence
- daemon ensure needed
- bind/rebind needed
- missing peer evidence

### 6.2 peer tools
后续必须补抽象：
- `peer.list`
- `peer.describe`
- `capability.invoke`
- `agent.assign`
- `binding.open`
- `binding.close`
- `daemon.ensure_peer`
- `peer.status_probe`

### 6.3 peer events
后续必须补事件族：
- `peer.discovered`
- `peer.connected`
- `peer.authenticated`
- `lease.opened`
- `binding.opened`
- `binding.closed`
- `heartbeat.missed`
- `daemon.peer_spawned`
- `daemon.peer_reaped`

---

## 7. 实现落点

本轮骨架对应源码真源：

- contracts：`rust/crates/contracts/src/context.rs`
- context build：`rust/crates/runtime/src/context_view.rs`
- peer block build/render：`rust/crates/runtime/src/context_blocks.rs`
- prompt role family：`rust/crates/runtime/src/prompt_assembly.rs`
- assembled prompt：`rust/crates/runtime/src/model_input_assembler.rs`

---

## 8. 验收标准

本轮不要求 remote peer 真执行，但要求：

1. `MinimalContextView.peer` 已进入上下文真源
2. assembled prompt 中能看到 `Peer topology`
3. role prompt 已区分 `system_agent / project_agent / channel_gateway / peer_router`
4. 当前 runtime 在没有 peer registry 时，仍能生成受控的 local-only peer placeholder
5. 相关 runtime / cli / debug-server 回归通过

