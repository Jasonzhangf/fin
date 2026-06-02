# 25 Prompt System

本文档冻结 `fin` 的 prompt system 总体设计。

目标：

1. 明确 prompt system 的分层结构
2. 明确 cached prefix / session overlay / turn context 的边界
3. 明确 agent identity、role family、project policy 的归属
4. 为后续 prompt 内容实现、Web 可观测、回归验证提供真源

---

## 1. 核心原则：Agent-first，不是 Model-first

`fin` 的 prompt system 必须服从以下硬边界：

1. **用户面对的是 Agent，不是模型**
2. **模型只接收 framework 分配的 role / request / context / tools**
3. **provider / model family / transport 只属于 backend runtime truth，不属于 agent identity truth**
4. **prompt system 不能让模型自我理解为“我是某个模型，正在直接和用户聊天”**

因此，`fin` 的 prompt system 真源必须是：

```text
Agent Doctrine
+ Role Prompt Modules
+ Session/Task Overlay
+ Turn Context Envelope
= Model Input
```

而不是：

```text
provider/model identity
+ model-family self framing
+ user-direct chat framing
```

---

## 2. Prompt System 的总结构

`fin` 不采用“单一 system prompt 字符串”的设计。

冻结为四层：

```text
Stable Core Prompt
+ Role Prompt Modules
+ Session/Task Overlay
+ Turn Context Envelope
= Model Input
```

### 2.1 Stable Core Prompt

稳定前缀，目标是：

- 最大化 provider prompt caching
- 尽量避免每轮波动
- 放入跨轮稳定、跨 task 稳定、跨 agent 稳定的规则
- 只表达 `fin` 框架、agent 身份边界与稳定行为规则

Stable core 的详细边界与文本草案见：

- `docs/architecture/27-stable-core-prompt.md`
- `docs/prompts/01-stable-core-prompt-v1.md`

### 2.2 Role Prompt Modules

按 agent role 组织的模块化 prompt。

它不是 raw markdown 拼接的“长文”，而是可结构化、可编译、可观测的模块集合。

当前 role 真源只允许：

1. `system`
2. `project`

### 2.3 Session/Task Overlay

这层负责：

- 当前 session / task / topic 的稳定上下文
- prompt history / lineage
- current focus / backlog hints / task board digest
- primary_project / active_projects / projects
- 当前 owner / review / dispatch 相关稳定状态

它比 stable core 更动态，但比单轮 history 更稳定。

### 2.4 Turn Context Envelope

这层每轮重建：

- recent history
- recent digests
- knowledge hits
- current request
- selected paths / current focus
- 本轮 tool selection policy / current tool state

注意：

- 这里应表达 `current request` / `routed work item`
- 不应把本层写成“用户正在直接对模型说话”的 framing

---

## 3. Cached vs Ephemeral 的边界

`fin` 继续采用 Hermes 式的核心原则：

```text
cached prompt prefix != per-turn dynamic overlay
```

### 3.1 Cached Prefix

应尽量只包含：

1. agent identity
2. framework doctrine
3. stable role baseline
4. stable project policy snapshot
5. stable user preference snapshot

特点：

- 同一 session 内尽量不变
- 同一 provider 路径下尽量可复用
- 不混入每轮 digest/history/current request
- 不混入 provider/model 自我认知文案

### 3.2 Session Overlay

放入：

- topic continuity
- active project set
- task / session summary
- prompt history / prompt lineage
- task board / backlog digest

特点：

- 可按 task / topic rebuild
- 比 current turn 稳定
- 允许在 topic switch / session revive 时整体重建

### 3.3 Turn Envelope

放入：

- recent messages
- recent digests
- knowledge artifacts
- current request
- current path focus

特点：

- 每轮重建
- 只对当前推理有效
- 是 Web debug 重点可见对象

---

## 4. Output Contract 的双重暴露规则

`fin` 的结构化输出约束不能只埋在 prompt 摘要里。

第一版继续冻结为双重暴露：

1. **Prompt module / output_contract block**
   - 进入 `role_prompt.output_contract`
   - 供 Web debug、session artifacts、文档对齐查看
2. **Turn-end mandatory answer format**
   - 在 `ModelInputAssembler` 的末尾再次注入
   - 使用明确的两段式模板：
     - `<fin_user_response>...</fin_user_response>`
     - `<fin_control_feedback>{...}</fin_control_feedback>`

原因：

- 结构化输出是 runtime truth contract，不是 provider-specific trick
- exact schema 与常见失败样式都需要在推理末尾再次钉死，提升 exact parse 命中率

---

## 5. Prompt Source Ownership

`fin` 的 prompt 内容来源冻结为五类。

### 5.1 Identity Source

负责：

- `system agent` / `project agent` 的身份定义
- Agent 与用户、Agent 与 backend model 的边界
- 运行环境定位

### 5.2 Role Source

负责：

- 不同角色的 baseline prompt
- 角色行为边界
- 角色输出偏好
- 角色的 owner / review / dispatch 责任

### 5.3 Framework Source

负责：

- operation -> event 是 runtime fact truth
- session artifacts 是 channel render truth
- framework 与 model 的 ownership boundary
- progress / note / digest / materialization 的自动化框架职责

### 5.4 Project Source

负责：

- 项目级 policy
- repo / workdir / selected paths 的约束
- 本地 AGENTS / docs / local skills 编译后的项目规则
- project task system 的当前稳定规则

### 5.5 Tool Source

负责：

- model tools 的真实可调用 spec
- framework capabilities 的不可误用说明
- tool selection policy

---

## 6. Role Family

当前只冻结两类 prompt role family。

### 6.1 System Agent

负责：

- 唯一用户入口
- 全局 task portfolio / backlog / priority 视角
- orchestration / routing / dispatch / review / recovery
- 多 project / 多 worker / 多 peer 的协调

system agent 的心智模型应是：

- 指挥家
- 协调者
- leader
- owner / reviewer / dispatcher

而不是：

- 默认沉入执行的 worker
- 长时间单 project 实现者
- 直接和用户裸聊的模型

### 6.2 Project Agent

负责：

- 单项目推进
- 项目内 epic / task / docs / code / testing / debug 闭环
- 作为 project-scope owner / reviewer / dispatcher 管理 worker
- 在同一 role 内承担 execution / review / diagnosis / handoff 等 workflow emphasis

补充规则：

- `worker` / `reviewer` / `analyzer` 不再作为独立 prompt role
- router / gateway / capability 等属于框架组件或 peer taxonomy，不属于 prompt role family

---

## 7. 明确禁止进入 Prompt Identity 的内容

以下内容不得进入 agent identity / role baseline 的自我认知层：

1. provider/model family 身份
2. “你是某个模型”的表述
3. “你正在直接和用户聊天”的 framing
4. backend transport / protocol quirks
5. provider-specific normalization 细节

这些内容若存在，只能属于：

- runtime adapter
- parser / normalization
- backend execution policy
- debug / observability truth

而不是 prompt identity truth。

---

## 8. 当前非目标

当前先不在本文冻结：

1. backend provider adaptation 的精确实现策略
2. prompt cache key 算法
3. provider-specific wire payload
4. task board 字段级 schema
5. review / reopen 的完整状态机
