# M1 Scope and Freeze

本文档冻结 `fin` 当前 M1 的边界，目标是把系统收口成“可用、可验证、可观察”的最小闭环，而不是继续扩框架。

## 1. 当前阶段结论

截至 2026-04-19，`fin` 已进入 **M1 closeout**：

- 目标：冻结边界、补回归矩阵、只修阻塞 M1 的缺口
- 原则：不再继续引入新的大框架概念
- owning truth：Rust runtime / session artifacts / event archive
- Web 角色：观察与调试，不得补 runtime 真相

## 2. M1 必须包含的能力

### 2.1 单 agent 推理闭环

M1 的最小可用闭环必须已经具备：

1. 真实 provider 请求与响应
2. 同一 session 下的多轮推理
3. 同一 turn 内的自动 tool loop
4. tool execution 记录与落盘
5. stop / wait / reminder
6. 普通回复、错误回复、status probe 的结构化返回

### 2.2 上下文与重建

M1 必须具备：

1. stable core prompt + role prompt + overlays
2. tool catalog 与 tool prompt 基础装配
3. history / recent reasoning / recent tool activity
4. project context
5. `/compact` 走 framework rebuild，而不是大模型压缩

### 2.3 durable truth 与可观测性

M1 必须具备：

1. session render truth
2. event stream + archive + index
3. provider / control / note / digest / tool / closure / turn / step 持久化
4. Web debug 与 status probe 的同源读取

### 2.4 最小 control plane

M1 允许的控制面范围冻结为：

1. `RoutingDecisionRecord`
2. `RoutingActionRecord`
3. `SchedulerDecisionRecord`
4. `SchedulerTickRecord`
5. `SupervisorCycleRecord`
6. `SupervisorHeartbeatRecord`
7. `DaemonStateRecord`
8. `DaemonRecoveryActionRecord`

它们的职责是：

- 表达当前任务是否继续
- 表达 queue 是否可推进
- 表达 tick / supervisor / heartbeat / daemon 当前观察结论
- 作为 Web / CLI / archive 的统一控制面真源

## 3. M1 明确不包含的范围

以下能力不属于 M1，不得为了“顺手完善”继续扩张：

1. 真多 agent 执行面
2. 跨机器 project agent 协作
3. detached / headless daemon 常驻体系
4. 真正可恢复的 pause/resume 执行栈
5. 普通推理请求的真正并行执行
6. 完整 session / task / topic 产品化状态机
7. 成熟 knowledge graph / wiki / memory graph
8. channel / gateway / remote peer 的完整生产化接入

这些能力统一进入 M2 backlog。

## 4. M1 冻结规则

### 4.1 默认策略

从本文件生效起，任何新增工作默认按以下顺序判断：

1. 是否直接阻塞 M1 可用性？
2. 是否直接破坏已有 durable truth？
3. 是否属于回归矩阵缺口？
4. 否则进入 M2 backlog，而不是立即实现

### 4.2 允许继续修改的范围

当前只允许继续做：

1. M1 阻塞性 bug 修复
2. 推理链闭环缺口补齐
3. 回归测试补齐
4. 文档收口与证据整理
5. 已有 truth 的一致性修复

### 4.3 暂停继续扩展的范围

当前暂停：

1. 新的 peer / network / daemon 大模块
2. 新的 UI 架构重做
3. 新的 memory system 形态扩展
4. 非 M1 必需的 slash / tool / gateway 大族迁移

## 5. M1 的完成判据

M1 只有在以下条件都成立时才算收口：

1. provider smoke 可用
2. 多轮推理闭环可用
3. tool loop 可用
4. stop / wait / reminder 可用
5. `/compact` rebuild 可用
6. status probe / tick / supervisor / heartbeat / daemon state 可观察
7. 关键 durable truth 能从 session artifacts 重建
8. 自动测试与手工验证矩阵可追踪

## 6. closeout 期间的开发纪律

1. 先补证据，再说“完成”
2. 不为了“更优雅”重写稳定路径
3. 不把 Web 观察逻辑回灌成 runtime 真相
4. 新规则优先写入 `docs/closeout/`、`MEMORY.md`、本地 skills
5. 任何超出本文边界的工作，先登记到 `docs/closeout/m1-known-gaps-and-m2-backlog.md`
