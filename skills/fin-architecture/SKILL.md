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
