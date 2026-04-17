# 01 System Overview

`fin` 是一个以 **Rust runtime + Web 调试后台 + harness 回放** 为核心的多 agent 框架。

## 目标

- 支持跨进程、跨网段多 agent 协作
- 支持长期常驻运行与恢复
- 优先建立调试可见性、录制、回放、故障注入能力
- 用共享函数 + 编排结构 + 框架化 + CI 自动化支撑后续扩展

## 核心判断

1. **Rust 负责跑**：runtime、orchestrator、registry、transport、harness 核心都在 Rust。
2. **Web 负责看**：cluster overview、timeline、trace、raw events、replay 操作台在 Web。
3. **结构化事件是全局共同真源**：runtime 发事件，web / harness / cli / ci 消费事件。
4. **先骨架后能力**：先把路由、分层、契约、调试底座建好，再加复杂自治策略。

## 顶层组成

- `docs/`：设计真源
- `skills/`：执行适配
- `rust/`：Rust workspace
- `web/`：Web 调试后台
- `harness/`：录制、回放、故障注入入口
- `fixtures/`：协议样例与测试样例

## 第一阶段实现优先级

当前先不追求完整多 agent 能力，而是先建立 **最小可用、最小可观测** 的脚手架：

1. **Config Module**
   - 用户配置最小化
   - 系统配置单文件
   - user-config 到 system-config 的单向映射
2. **AI Provider Module**
   - 独立 provider 边界
   - 多协议支持
   - 与 runtime / task 语义解耦
3. **Inference + Recording Slice**
   - 单 runtime 推理闭环
   - Control / Routing / Progress / Note / Digest 落盘
4. **Web Debug MVP**
   - 当前 session/task/topic
   - live event stream
   - current progress
   - latest note / digest

## 当前架构讨论的主线

后续架构与模块实现应围绕以下主线持续收敛：

- Session / Context / TentativeSession
- TopicThread / Task / Dispatch / ProgressBlock
- RunJournal / ExecutionNote / Digest / KnowledgeArtifact
- framework-owned subconscious（heartbeat、记录、切换确认、健康检查）
- M1 → M2 的分阶段迭代，而不是一开始做全量系统
