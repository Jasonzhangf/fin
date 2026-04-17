# 11 M1 Scaffolding and Iteration Plan

本文档定义 `fin` 当前最重要的开发问题：

- 什么是 **M1 最小可用脚手架**
- M1 包含什么，不包含什么
- 先做什么，后做什么

## 1. 当前项目真实状态

当前仓库已经有：

- 治理骨架（AGENTS / skills / docs）
- Rust workspace 骨架
- CI 最小门禁

当前仓库还没有：

- 独立 Config 模块
- 独立 AI Provider 模块
- 真正可跑的 inference loop
- 真正的 event/projection
- 最小 Web debug

结论：

- 当前是 **治理骨架已建立**
- 还不是 **最小可用系统**

## 2. M1 的目标

M1 不是完整多 agent 系统，而是：

> 一个单 runtime、单进程、最小推理闭环 + 最小框架记录闭环 + 最小 Web 可观测闭环。

M1 必须证明四件事：

1. Rust 推理主循环能跑
2. 框架能自动记录
3. 事件流能产出
4. Web 能看到当前系统状态

## 3. M1 的六个模块

### A. Config Module

职责：

- 用户配置最小化
- 系统配置单文件
- user-config 到 system-config 的单向映射
- 不做 merge

规则：

- 用户只配置必须由用户决定的东西
- 系统模块配置统一在一个 system config 文件里
- 用户配置项与系统配置项尽量互斥

### B. AI Provider Module

职责：

- 独立 provider 边界
- 多协议支持
- provider registry / descriptor / adapter
- 与 runtime / task 语义解耦

约束：

- 可以参考 `finger` 现有 provider 结构
- 但在 `fin` 中必须保持独立模块边界

### C. Inference Kernel

职责：

- 单 runtime
- 单 inference loop
- closure boundary
- 输出结构化结果

### D. Recording / Context Module

职责：

- Control / RoutingFeedback
- Progress / ExecutionNote / Digest
- Event append
- 最小 Context packing

### E. Projection / Debug Server

职责：

- 当前状态投影
- live event stream
- 给 Web 提供最小读取接口

### F. Web Debug MVP

第一版只做最基础展示：

- Current Session / Task / Topic
- Live Event Stream
- Current ProgressBlock
- Latest Execution Notes / Digest

## 4. M1 明确不包含什么

M1 暂不包含：

- 完整多 worker
- 复杂 scheduler
- cross-process / cross-node
- replay / fault injection 全套
- topic revive / merge / split 全套
- 高级 Web 调试台
- 复杂 retrieval ranking

这些属于 M2 及以后。

## 5. 建议实现顺序

### Phase 0

治理骨架：已完成

### Phase 1

先做：

1. Config Module
2. AI Provider Module

原因：

- 这两者是所有后续模块的底座
- 若不先定，runtime / debug / task bootstrap 都会返工

### Phase 2

做单 runtime inference + framework recording 垂直切片：

- single inference closure
- control / routing feedback
- progress / execution note / digest
- event append

### Phase 3

打通最小 Debug Server + Web Debug：

- 当前 session/task/topic 状态
- live events
- current progress
- latest note / digest

### Phase 4

再进入：

- TentativeSession
- task formalization
- user confirmation loop

### Phase 5

最后再做：

- leader / worker / dispatch
- heartbeat / timeout / recovery
- 多 worker / 跨进程 / 跨网段

## 6. 当前最重要的决策

当前最重要的，不是继续深挖局部状态机，而是：

1. M1 边界
2. 模块分块
3. 实现顺序

## 7. 完成标准

M1 完成时，至少满足：

- 可以用最小配置启动系统
- 可以通过独立 provider 模块调用一个模型
- 可以跑完一个 closure
- 框架能自动写入 control / routing / progress / note / digest / event
- Web Debug MVP 能看见当前状态和事件流

## 8. 进入 M2 的前提

只有当 M1 跑通后，才进入：

- Topic/Task 更复杂路由
- 单机多 worker
- health monitor / timeout escalation
- cross-process / cross-node
- replay / fault injection