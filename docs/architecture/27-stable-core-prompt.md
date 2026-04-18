# 27 Stable Core Prompt

本文档冻结 `fin` 的 stable core prompt 设计边界。

目标：

1. 明确什么内容允许进入 stable core
2. 明确 stable core 与 role/session/turn 的边界
3. 明确 Hermes / Codex / fin 自身语义的吸收方式
4. 为后续 stable core assembler 与 prompt 文本实现提供真源

---

## 1. 参考原则

`fin` 的 stable core prompt 采用：

- **结构参考 Hermes**
- **写法参考 Codex**
- **语义服从 fin 自己的 runtime / session / event 架构**

具体含义：

### 1.1 结构参考 Hermes

主要吸收：

1. cached prefix 与 per-turn additions 分离
2. system prompt 必须按稳定 layer 装配
3. memory / profile / context source 的注入顺序要稳定
4. provider prompt caching 优先级高于临时方便

### 1.2 写法参考 Codex

主要吸收：

1. prompt 内容必须短、硬、可执行
2. 少 narrative，多 directive blocks
3. 先写行为边界，再写偏好
4. verification / tool / final answer discipline 必须分块显式出现

### 1.3 语义服从 fin

stable core 最终必须表达的，不是 Hermes 或 Codex 的产品身份，
而是 `fin` 自己已经冻结的框架边界：

- session artifacts 是 channel render truth
- runtime events 是 operation fact truth
- framework 自动负责 materialization / digest / note / projection
- model tools 与 framework capabilities 必须隔离
- context rebuild / topic continuity 由框架与结构化 artifacts 共同支撑

---

## 2. Stable Core 的定义

stable core 是 prompt system 中**最稳定的前缀层**。

它满足：

1. **跨 turn 稳定**
2. **跨 task 大体稳定**
3. **跨 role 尽量稳定**
4. **适合 provider prompt cache**
5. **不承载当前任务的动态细节**

在 `fin` 中，stable core 的位置是：

```text
Stable Core Prompt
+ Role Prompt Modules
+ Session/Task Overlay
+ Turn Context Envelope
= Model Input
```

---

## 3. 什么内容允许进入 Stable Core

当前第一版冻结六类模块。

### 3.1 identity_and_runtime_position

只放最小必要定位：

- 当前 agent 运行在 fin framework 中
- 输出会进入 framework 的 session / event / projection 流程
- 当前 agent 不是脱离框架的自由文本助手

要求：

- 不写多余人格描述
- 不写 provider 细节
- 不写当前 project 细节

### 3.2 framework_truth_rules

这是最核心的 stable core 模块。

负责表达：

- session artifacts 是 channel render truth
- runtime events 是调试与状态事实真源
- 不得捏造 artifacts / events 中不存在的状态
- framework internal capability 不得冒充 model-selected tool

### 3.3 execution_discipline

负责表达跨角色通用的执行纪律：

- verify before conclude
- no silent failure
- destructive action needs explicit authorization
- ambiguity 只有在影响动作时才值得追问
- 不得无证据宣称完成

### 3.4 tool_usage_discipline

负责表达工具边界：

- model tools 与 framework capabilities 隔离
- 只有 tool catalog 中允许的 model tools 可被模型当作直接工具能力理解
- 没有 model tools 时，不得伪造工具调用
- 需要工具时优先依赖结构化 tool catalog，而不是靠记忆假设

### 3.5 memory_and_session_discipline

负责表达稳定的记忆 / session 规则：

- durable memory 只保留长期稳定事实
- task progress / transient logs 不属于 durable memory
- topic continuity 由 digest / history / knowledge artifacts 共同重建
- current turn context 不等于 long-term memory

### 3.6 output_discipline

负责表达稳定输出规则：

- 能直接回答就直接回答
- 区分 confirmed facts / inference / missing information
- 输出要适合被框架沉淀为 note / digest / projection
- 不泄露内部 chain-of-thought
- 用户可见回答必须遵守 truth boundary

---

## 4. Global Skills 在 Stable Core 中的归属

`~/.fin/skills` 已经是全局 skills 运行时真源目录。

但 stable core 中**不应该直接注入整份 skill 正文**。

第一版冻结为两层：

### 4.1 global_skill_usage_policy

这部分属于 stable core。

只表达：

- 当已加载 global skills 与当前任务相关时，应优先复用
- 不应在已有 skill 可复用时重新发明工作流规则
- skill index 是辅助约束，不是 project truth 的替代品

### 4.2 loaded_global_skill_index_snapshot

这部分允许作为 stable-core-adjacent cached snapshot 注入。

要求：

- 只包含 skill id / title / short summary
- 不注入完整 skill 正文
- 同一 session 内应尽量稳定
- 当 global skills 集发生变化时允许重建 cached prefix

结论：

> stable core 负责“如何使用 global skills”的规则；
> runtime snapshot 负责“当前加载了哪些 global skills”的摘要。

---

## 5. 什么内容禁止进入 Stable Core

以下内容不得进入 stable core：

1. 当前 task 描述
2. 当前 topic continuity 内容本身
3. recent history
4. recent digests
5. current input
6. selected paths / current file focus
7. 当前 project 的动态范围与临时规则
8. provider 当前请求参数快照
9. 当前 turn 的 tool state
10. 当前用户即时意图解析结果

这些内容必须分别进入：

- role modules
- session/task overlay
- turn context envelope
- tool catalog block
- project policy snapshot

---

## 6. Stable Core 的装配顺序

第一版建议顺序：

```text
1. identity_and_runtime_position
2. framework_truth_rules
3. execution_discipline
4. tool_usage_discipline
5. memory_and_session_discipline
6. global_skill_usage_policy
7. output_discipline
8. loaded_global_skill_index_snapshot (if any)
```

规则：

- 先定义真相边界
- 再定义执行纪律
- 再定义工具纪律
- 最后才放 skill index snapshot
- loaded skill snapshot 只能放在规则之后，避免模型把 skill index 当作高于 framework truth 的东西

---

## 7. Prompt Writing Rules for Stable Core

stable core 的文本必须满足：

1. **短句化**：尽量一行一条
2. **硬边界优先**：先禁止，再偏好
3. **可验证**：每条规则都应能映射到 event / artifacts / projections
4. **不写废话人格**：不用“careful / thoughtful / collaborative”之类空描述
5. **ownership 清晰**：framework / tool / memory / output 各写各的

推荐形态：

```text
[Module: framework_truth_rules]
- Session artifacts are render truth for channels.
- Runtime events are fact truth for execution debugging.
- Do not invent state not present in artifacts or events.
```

不推荐形态：

```text
You are a thoughtful assistant who should generally try to be helpful and careful...
```

---

## 8. Stable Core 与其他层的边界

### 8.1 与 Role Modules 的边界

stable core 只写跨角色稳定纪律。

role modules 才负责：

- system/project/worker/reviewer 的职责差异
- decision scope
- output emphasis
- delegation style

### 8.2 与 Session/Task Overlay 的边界

session overlay 才负责：

- 当前 task / session summary
- prompt history
- topic continuity
- active projects
- current project policy snapshot

### 8.3 与 Turn Context Envelope 的边界

turn envelope 才负责：

- current input
- recent messages
- recent digests
- knowledge hits
- selected paths / focus
- current tool state

---

## 9. 最小实现落点

后续实现建议最少有这些对象：

```text
StableCoreModulePack
StableCoreAssembler
StableCoreSnapshot
LoadedGlobalSkillIndexSnapshot
```

第一阶段不要求一次性做复杂缓存策略。

只要求：

1. stable core 的内容有独立真源
2. stable core 能单独被 Web debug 解释
3. 后续 role/session/turn 层能在它之上继续叠加

---

## 10. 最小验证要求

stable core 变更时至少验证：

1. stable core 文本与 role/session/turn 内容没有 ownership 混写
2. loaded global skills 只以 index/summary 形式出现
3. 当前结构化 prompt blocks 能解释 stable core 的来源
4. old session artifacts 不因 stable core 演进而读崩
5. web debug 后续能增加 stable core summary / lineage 观察位

---

## 11. 当前非目标

当前不冻结：

1. prompt cache key 算法
2. stable core binary cache 持久化格式
3. loaded skills 的动态热更新协议
4. provider-specific prompt packing 细节
5. 每个 role 的最终完整 raw prompt 正文
