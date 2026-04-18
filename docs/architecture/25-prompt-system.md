# 25 Prompt System

本文档冻结 `fin` 的 prompt system 总体设计。

目标：

1. 明确 prompt system 的分层结构
2. 明确 cached prefix / session overlay / turn context 的边界
3. 明确 role family、model family overlay、project policy 的归属
4. 为后续 prompt 内容实现、Web 可观测、回归验证提供真源

---

## 1. 参考真源

本设计主要参考以下两类系统：

### 1.1 Hermes 的 prompt assembly

参考：

- `~/github/hermes-agent/website/docs/developer-guide/prompt-assembly.md`
- `~/github/hermes-agent/agent/prompt_builder.py`

关键启发：

1. **cached system prompt state** 与 **ephemeral API-call-time additions** 必须分离
2. system prompt 必须按稳定 layer 装配，而不是临时拼接
3. model family specific guidance 应独立成 overlay，而不是混进一份万能 prompt
4. context files / memory / user profile 的注入顺序需要稳定，服务 prompt caching

### 1.2 Codex 的纪律化 prompt

参考：

- `~/code/codex/codex-rs/core/gpt_5_codex_prompt.md`
- `~/code/codex/AGENTS.md`

关键启发：

1. prompt 内容应该是短、硬、可执行的 directive blocks
2. repo 规则与系统规则需要分离
3. model-facing discipline 应显式覆盖：
   - editing constraints
   - planning discipline
   - verification discipline
   - final answer discipline

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

Stable core 的详细边界与文本草案见：

- `docs/architecture/27-stable-core-prompt.md`
- `docs/prompts/01-stable-core-prompt-v1.md`

### 2.2 Role Prompt Modules

按 agent role 组织的模块化 prompt。

它不是 raw markdown 拼接的“长文”，而是可结构化、可编译、可观测的模块集合。

### 2.3 Session/Task Overlay

这层负责：

- 当前 session / task / topic 的稳定上下文
- prompt history / lineage
- primary_project / active_projects / projects
- 用户长期偏好和当前任务类型

它比 stable core 更动态，但比单轮 history 更稳定。

### 2.4 Turn Context Envelope

这层每轮重建：

- recent history
- recent digests
- knowledge hits
- current input
- selected paths / current focus
- 本轮 tool selection policy / current tool state

---

## 3. Cached vs Ephemeral 的边界

`fin` 需要明确 borrowed Hermes 的一个核心原则：

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
6. stable model-family overlay

特点：

- 同一 session 内尽量不变
- 同一 provider/model 路径下尽量可复用
- 不混入每轮 digest/history/current input

### 3.2 Session Overlay

放入：

- topic continuity
- active project set
- task / session summary
- prompt history / prompt lineage

特点：

- 可按 task / topic rebuild
- 比 current turn 稳定
- 允许在 topic switch / session revive 时整体重建

### 3.3 Turn Envelope

放入：

- recent messages
- recent digests
- knowledge artifacts
- current input
- current path focus

特点：

- 每轮重建
- 只对当前推理有效
- 是 Web debug 重点可见对象

### 3.4 Output Contract 的双重暴露规则

`fin` 的结构化输出约束不能只埋在 prompt 摘要里。

第一版冻结为双重暴露：

1. **Prompt module / output_contract block**
   - 进入 `role_prompt.output_contract`
   - 供 Web debug、session artifacts、文档对齐查看
2. **Turn-end mandatory answer format**
   - 在 `ModelInputAssembler` 的末尾再次注入
   - 使用明确的两段式模板：
     - `<fin_user_response>...</fin_user_response>`
     - `<fin_control_feedback>{...}</fin_control_feedback>`
   - 同时列出常见错误禁止项：
     - `0.98 / 1.0` 这类小数 confidence
     - `"true"` 这类字符串布尔值
     - fin 白名单之外的额外 key

原因：

- 真实 provider 只看摘要规则时，容易漂移成“语义正确但 schema 不精确”
- 需要把精确 schema 与常见失败样式都在推理末尾再次钉死，提升 exact parse 命中率

---

## 4. Prompt Source Ownership

`fin` 的 prompt 内容来源冻结为五类。

### 4.1 Identity Source

负责：

- system agent / project agent / worker agent 的身份定义
- 平台与运行环境定位

### 4.2 Role Source

负责：

- 不同角色的 baseline prompt
- 角色行为边界
- 角色输出偏好

### 4.3 Framework Source

负责：

- operation -> event 是 runtime fact truth
- session artifacts 是 channel render truth
- framework 与 model 的 ownership boundary
- progress / note / digest / materialization 的自动化框架职责

### 4.4 Project Source

负责：

- 项目级 policy
- repo / workdir / selected paths 的约束
- 本地 AGENTS / docs / local skills 编译后的项目规则

### 4.5 Tool Source

负责：

- model tools 的真实可调用 spec
- framework capabilities 的不可误用说明
- tool selection policy

---

## 5. Role Family

当前先冻结四类 role family。

### 5.1 System Agent

负责：

- 多 project 视角
- orchestration / routing / health / recovery
- active_projects 可能 > 1

### 5.2 Project Agent

负责：

- 单项目推进
- 当前 project 的 task / docs / code / testing / debug
- primary_project 通常唯一

### 5.3 Worker Agent

负责：

- bounded subtask execution
- 少决策，多执行
- focus 在指定 work slice

### 5.4 Reviewer / Analyzer

负责：

- review / diagnosis / comparison / validation
- 风险、回归、验证缺口

---

## 6. Model Family Overlay

不同模型家族应有单独 overlay。

### 6.1 GPT / Codex overlay

应强化：

- tool persistence
- prerequisite checks
- verification discipline
- missing context 不得 hallucinate

### 6.2 Gemini / Gemma overlay

应强化：

- provider-specific tool usage guidance
- format stability
- operational constraints

### 6.3 Future overlays

后续可加：

- Claude-family
- Qwen-family
- 其他 provider/model specific overlays

原则：

> model family overlay 是 stable prompt module，不是每轮 context 字段。

---

## 7. Prompt Modules 冻结清单

`fin` 当前建议冻结以下 prompt modules：

1. `identity`
2. `role_baseline`
3. `framework_truth_rules`
4. `tool_usage_rules`
5. `memory_policy`
6. `session_policy`
7. `topic_continuity_policy`
8. `project_policy`
9. `output_contract`
10. `model_family_overlay`

说明：

- 不是所有 module 都要一开始有完整文本
- 但 module 名称、职责、ownership 必须先冻结

---

## 8. 运行时 Prompt Build Pipeline

建议冻结为：

```text
PromptSourceRegistry
  -> StableCoreAssembler
  -> RolePromptAssembler
  -> SessionOverlayAssembler
  -> TurnContextAssembler
  -> ModelInputAssembler
```

### 8.1 PromptSourceRegistry

负责收集：

- role baselines
- framework rules
- project policies
- tool prompt specs
- model family overlays

### 8.2 StableCoreAssembler

负责拼：

- identity
- framework stable doctrine
- stable role baseline
- stable project/user snapshot
- model family overlay

### 8.3 RolePromptAssembler

负责产出：

- current_prompt_summary
- prompt_modules
- prompt_lineage
- output_contract

### 8.4 SessionOverlayAssembler

负责产出：

- topic continuity
- session/task summary
- primary/active project set
- prompt_history

### 8.5 TurnContextAssembler

负责产出：

- recent history
- digests
- knowledge hits
- selected paths
- current input

### 8.6 ModelInputAssembler

负责最终形成 provider 请求的模型输入。

---

## 9. Prompt System 与 ContextView 的关系

`ContextView` 不是 prompt 本身，但它必须可解释 prompt build 的结果。

所以：

- `RolePromptBlock` 应可见当前 prompt modules / lineage / output contract
- `ProjectContextBlock` 应可见 primary / active / all projects
- `ToolCatalogBlock` 应可见 tool selection policy 与 tool spec

结论：

> Web debug 观察的是 prompt system 的结构化投影，不是 raw prompt 文本本身。

---

## 10. Web Debug 最低可见要求

Web 侧至少应能看到：

1. 当前 role prompt 摘要
2. prompt modules 列表
3. prompt lineage
4. output contract
5. tool selection policy
6. model tools / framework capabilities 的结构化 spec
7. primary_project / active_projects / projects

后续如需要，再增加 raw rendered prompt preview。

---

## 11. 变更策略

### 11.1 先改设计真源

prompt system 变更，先改：

- `docs/architecture/25-prompt-system.md`
- `docs/contracts/prompt-module-contract.md`

### 11.2 再改执行 skill

如果变更会影响：

- prompt source ownership
- assembler 顺序
- 验证方式

则更新：

- `skills/fin-prompt-system/SKILL.md`

### 11.3 再改实现

实现层再改 Rust runtime / debug Web / tests。

---

## 12. 当前非目标

当前先不展开这些细节：

1. 每个 role 的完整最终 prompt 文本
2. 每个 model family 的完整 overlay 文本
3. 完整 prompt cache key 算法
4. 多 project registry 的最终持久化格式
5. tool calling protocol 的最终 wire shape

这些到模块实现阶段再具体冻结。
