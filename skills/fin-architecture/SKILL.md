---
name: fin-architecture
description: Architecture ownership and crate-boundary skill for fin. Use when deciding where code or docs should live.
---

# fin Architecture Skill

## 1) Intent

用于判断：
- 这个能力属于哪个层级
- 该进哪个 crate
- 唯一真源在哪
- 是否需要更新 architecture docs

## 2) Canonical sources

1. `docs/architecture/02-layer-boundaries.md`
2. `docs/architecture/09-workspace-and-crate-map.md`
3. `docs/contracts/event-envelope-contract.md`
4. `docs/architecture/16-operation-and-event-model.md`
5. `docs/architecture/17-debug-method-and-observability-workflow.md`
6. `docs/contracts/00-m1-contracts-index.md`
7. `docs/architecture/34-text-channel-activity-cards.md`

## 3) Decision rules

- 配置加载/映射/标准化：`fin-config`
- provider descriptor / registry / adapter：`fin-provider`
- 协议字段：`fin-contracts`
- 纯函数/工具：`fin-shared`
- 生命周期：`fin-runtime`
- 分发推进：`fin-orchestrator`
- 注册发现：`fin-registry`
- HTTP/WS：`fin-transport-http`
- 回放/注入：`fin-harness-core`
- 调试投影：`fin-debug-server` / `web`

## 4) Minimal validation

- crate 边界变更：至少跑相关 crate 测试
- 契约变更：补 contract 验证
- 事件字段变更：确认 Web / harness 消费路径仍一致

## 5) Anti-patterns

- 把所有通用逻辑堆到 orchestrator
- 在 Web 维护第二份状态
- harness 复制协议定义
- peer/discovery/binding 设计还没接线时，就先把执行逻辑写死在 project-only prompt/context；正确顺序是先补 role/context schema 骨架，再接 peer tools / peer events / daemon
- 在 channel adapter 里直接拼第二套用户语义卡；文字 channel 与 WebUI 的 source card / user card / tool semantic render 必须共用 framework 真源
- 活动卡 contract 放 `fin-contracts`，聚合 builder 放 runtime 共享层（当前为 `fin-runtime::build_activity_cards`）；不要在单个 channel / 单个页面里重新定义卡结构
- `target -> session` conversation registry 属于 channel gateway/control truth，不属于 Web/renderer；当前 qqbot 真源路径冻结为 `~/.fin/runtime/channels/qqbot/conversations.json`
- agent taxonomy 真源只允许 `system/project` 两类 role；`worker` 是 project agent spawn/reuse 的 runtime 执行体，不是新的 prompt role。
- startup topology / project registry / wake queue 属于 framework control plane；`always_on` 与 unfinished-work 唤醒必须先落 `~/.fin/runtime/projects/*` 真源，再谈 daemon 自动拉起
- agent busy/idle/offline truth 属于 framework presence，不属于 UI 文案；system/project agent 当前状态应落 `~/.fin/runtime/agents/state/*.json`
