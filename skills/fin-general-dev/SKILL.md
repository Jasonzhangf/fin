---
name: fin-general-dev
description: Project-local default development workflow for fin. Use for feature work, refactors, and bug fixes before touching repo code.
---

# fin General Dev Skill

## 1) Intent

这是 fin 的默认开发入口 skill。

它只回答四件事：
1. 任务属于哪一层
2. 哪个文件/模块是真源
3. 最小正确改动点在哪
4. 最小验证矩阵是什么

## 2) Canonical sources

1. `AGENTS.md`
2. `docs/architecture/02-layer-boundaries.md`
3. 对应子系统 architecture 文档
4. 相关 crate / 模块源码

## 3) Workflow

1. 先读 `AGENTS.md`
2. 先找 owning layer
3. 再找唯一真源
4. 先定义最小验证，再实现
5. 只改 owning layer，避免跨层复制逻辑
6. 代码文件默认必须 `< 500` 行；仅允许极少数白名单文件豁免，并通过门禁脚本检查
7. 如新增稳定规律，更新 docs 或本地 skill

## 3.1) fin project-specific sink

- 本 skill 只沉淀 `fin` 项目特有规则，不重复跨项目通用方法论。
- `fin` 当前应沉淀到本地的内容包括：
  - session / context / tentative session
  - task / topic / routing / confidence
  - digest / rebuild / continuity
  - control block / progress block / execution note / knowledge artifact
  - project leader / orchestrator / heartbeat / recovery
- 这些内容进入本地 `skills/` 与 `docs/architecture/`，不回灌到全局 skill。


## 3.2) Module delivery baseline（路由到全局 skill）

模块级通用骨架不再在本地重复展开，统一遵循全局 `coding-principals`：

- Shared Functions + Blocks + Orchestration
- Operation + Event + Projection
- Module Debug Baseline
- Module Test Baseline
- Evidence-first Delivery

本地 skill 只补 `fin` 自己的附加要求：

- 结构化事件默认要能落到 `~/.fin` 对应证据目录
- 模块设计要兼容 session / workdir / runtime home 的分层
- Web 只能做观察层，不能补 runtime 真相
- 若模块影响多 agent / runtime / debug 链路，必须补 replay 或等价回放证据
- context / debug snapshot 默认采用 bounded-write；禁止无界散写与每轮碎片化落盘
- 生命周期设计默认前台可控；未获批准不引入 detached daemon / orphan process
- 正式 build / install / promote 统一走 build-versioning flow；不要把裸 `cargo build` 当成交付闭环
- 非白名单代码文件必须遵守 500 行上限；门禁脚本真源为 `scripts/check-code-line-limit.py`
- Web debug 实例纪律必须单一；当前对外只允许引用 `4040`，禁止把历史测试端口混进当前验证与汇报
- `ControlFeedback` / continuity / topic-shift / simple-query 判断必须由 runtime 产出单一事实，并进入 event + session artifact + note/digest；Web 只消费，不得二次推断
- 模型输出的结构化 block 必须做 schema 识别；错 shape 的 JSON 只能当 raw provider output 观察，不能冒充有效 control feedback
- 若模型输出的 control block 只是在 value type 上不合法但包含白名单 key，runtime 应做 mask 白名单提取与受控 coercion；未知字段一律丢弃，不允许半结构化脏数据进入 control truth
- normal conversation 与 debug 共享同一份 session render truth；所谓 richer UI 是 richness level 的差异，不允许做两套前后端真源
- `Minimal / Rich / Full Trace` 这类 UI richness 开关只能控制展示层次，不能切换事实来源；所有 richness 都必须基于同一份 `operation_id -> session artifacts` 绑定结果
- 用户在任务进行中询问“当前状态”的请求应视为特殊并行 inquiry；必须带显式标记，走 non-interrupting side path，从最新 progress/note/control/tool state 组装回复，不能打断主推理

## 4) Minimal validation matrix

- L1 Unit：纯函数、状态机、schema
- L2 Contract：HTTP / WS / envelope
- L3 Harness：回放与故障注入
- L4 Cluster：多 worker / 多 node
- L5 Manual Debug：Web 调试后台观察

## 5) Anti-patterns

- 没确认 owning layer 就直接改
- 在 web 层补逻辑掩盖 runtime 真相
- 只改 docs 不补验证
- 只跑 unit 就宣称多 agent 行为完成
- 模块实现没有 operation/event/debug/test 骨架
