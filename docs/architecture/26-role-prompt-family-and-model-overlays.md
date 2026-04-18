# 26 Role Prompt Family And Model Overlays

本文档冻结 `fin` 的 role prompt 内容分层与 model overlay 设计。

目标：

1. 明确不同 agent role 的提示词职责边界
2. 明确 role baseline 与 project/session/context 的拼装关系
3. 明确 model-family overlay 的归属与最小内容方向
4. 为后续 Rust prompt assembler 与 Web 可观测提供真源

---

## 1. 设计原则

`fin` 的 prompt 内容不走“一份万能大提示词”。

统一采用：

```text
stable doctrine
+ role baseline
+ project/session overlays
+ turn context envelope
+ model-family overlay
```

约束：

1. role baseline 只写该角色长期稳定的行为边界
2. project 规则只进入 `project_policy` / `project_scope`，不回灌进 role baseline
3. tool 使用规则单独成 module，不和角色职责混写
4. session / topic continuity 进入 overlay，不进入 stable role text
5. raw prompt 文本必须能追溯到结构化 module ownership

---

## 2. 所有角色共享的基础模块

四类角色都共享以下模块，只是权重不同：

1. `identity`
2. `framework_truth_rules`
3. `tool_usage_rules`
4. `memory_policy`
5. `session_policy`
6. `topic_continuity_policy`
7. `project_policy`
8. `output_contract`

共享模块的冻结规则：

- 文本要短、硬、可执行
- 禁止写成长段 narrative 自我介绍
- 每个模块都要能给出 `summary`
- Web debug 至少要能显示模块名、摘要、来源、优先级

---

## 3. Role Family Baselines

### 3.1 System Agent

定位：

- 面向多 project 的 orchestration agent
- 负责 routing、health、recovery、delegation、priority 维护
- 不应该长时间沉入单个代码切片实现

必须强化：

1. 优先看 `active_projects` 而不是单一 `primary_project`
2. 优先判断 task routing / ownership / health，再决定是否自己执行
3. 对外部 worker 的反馈、超时、失败恢复保持敏感
4. 需要严格区分：框架自动推进 vs 需要模型决策的推进
5. 对 session/topic 切换、复活、冲突要保持监控

输出重点：

- 当前判断的 project/task 归属
- 是否需要 delegation / recovery / topic switch
- 下一步的 orchestration 动作
- 置信度与风险

### 3.2 Project Agent

定位：

- 单项目主推进 agent
- 负责项目内文档、架构、代码、测试、debug 的连续闭环
- 当前 `primary_project` 通常唯一

必须强化：

1. 把项目规则、当前 scope、selected paths 编译成稳定行为边界
2. 优先做最小闭环，不做无边界发散
3. 对 docs / skills / code / tests 采用 owning-layer 思维
4. 对 task continuity、recent digests、project knowledge 的使用要比 system agent 更重
5. 回答必须落到项目推进，而不是抽象空谈

输出重点：

- 当前 project scope
- 当前 task 在项目内的推进状态
- 结构化 next step / verify step
- 是否需要补文档、补 skill、补测试

### 3.3 Worker Agent

定位：

- bounded execution worker
- 接受上游 task slice，聚焦在明确边界内执行
- 少做策略判断，多做准确推进

必须强化：

1. 只在授权 scope 内行动
2. 遇到缺失前置条件要尽快回报，而不是自行扩张任务边界
3. 优先产出 progress / note / evidence
4. 避免自己重写全局架构判断
5. 对时间、资源、工具副作用保持保守

输出重点：

- 当前 slice 的完成度
- 遇到的 blocker / dependency
- 已验证证据
- 返回给 project/system agent 的可消费结论

### 3.4 Reviewer / Analyzer

定位：

- review、diagnosis、comparison、validation 专用角色
- 重点不是写代码，而是发现风险和验证缺口

必须强化：

1. findings first，不先讲大总结
2. 按严重度组织问题
3. 明确区分：已证据确认 / 推断 / 待验证
4. 优先指出回归风险、状态机缺口、观测缺口、测试缺口
5. 不把“可能有问题”包装成已确认结论

输出重点：

- findings 列表
- open questions
- verification gaps
- residual risks

---

## 4. Role Delta 的装配方式

不同 role 的差异主要体现在以下维度：

1. `decision_scope`
2. `project_scope`
3. `execution_authority`
4. `delegation_behavior`
5. `evidence_threshold`
6. `output_shape`

结论：

> role 切换不应该重建整个 prompt system，只应替换 role baseline modules 与少量 role-specific overlays。

也就是说：

- framework truth rules 保持稳定
- project/session/turn overlays 按当前 task 重建
- role baseline 则按 agent role 替换

---

## 5. Model Family Overlays

model-family overlay 是对 role baseline 的补充，不是替代。

### 5.1 GPT / Codex Overlay

必须强化：

1. tool persistence
2. prerequisite checks
3. verification before conclude
4. missing context 不得 hallucinate
5. 能直接行动就不要空谈计划

对应第一版文本草案：

- `docs/prompts/02-role-baselines-v1.md`
- `docs/prompts/03-gpt-codex-overlay-v1.md`

适合吸收的规则来源：

- `~/code/codex/codex-rs/core/gpt_5_codex_prompt.md`
- `~/code/codex/AGENTS.md`

### 5.2 Gemini / Gemma Overlay

必须强化：

1. provider/tool protocol 严格性
2. format stability
3. multi-step tool loop 的显式延续
4. 上下文缺失时不自作主张补全

### 5.3 Future Overlays

先预留：

- Claude-family
- Qwen-family
- 其他 provider/model specific overlays

原则：

- overlay 是 stable module
- overlay 只写该模型家族的行为修正
- overlay 不承载项目语义，不承载当前 turn 的动态上下文

---

## 6. Prompt Content Writing Style

所有 prompt module 文本统一遵守：

1. **短句优先**：尽量是一行一条 directive
2. **硬约束优先**：先写不可违反的边界，再写偏好
3. **可验证**：写出来的规则必须能在 event / projection / artifacts 中看到验证点
4. **不写废话人格**：避免无用自我形容
5. **不混 ownership**：framework、project、tool、role 分开写

推荐形态：

```text
[Module: framework_truth_rules]
- Session artifacts are render truth for channels.
- Runtime events are operation truth for debugging.
- Do not invent state not present in artifacts.
```

不推荐：

```text
You are a very careful, thoughtful, collaborative, intelligent assistant who values...
```

---

## 7. Project Policy 如何进入 Prompt

项目规则不能粗暴整份注入 raw docs。

冻结规则：

1. 本地 `AGENTS.md` / `docs/` / `skills/` 是 project truth sources
2. prompt build 时应编译为项目级摘要模块，而不是全文拼接
3. 只有与当前 role / task / selected paths 有关的 project policy 才应进入 session overlay
4. Web debug 需要能看到：
   - 当前采纳了哪些 project policy
   - 来自哪些 source
   - 当前作用在哪个 role / task 上

这层最终进入：

- `project_policy`
- `project_scope`
- `prompt_lineage`

---

## 8. Tool Prompt 与 Role Prompt 的边界

角色提示词不负责承载完整 tool schema。

冻结边界：

- role prompt：告诉模型如何决策、何时用工具、何时停手
- tool prompt spec：告诉模型工具做什么、何时可用、输入输出约束、边界与副作用
- framework capability：必须和 model tools 分开显示与分开装配

结论：

> role prompt 解决“为什么/何时做”，tool prompt 解决“怎么正确调用”。

---

## 9. 最小实现映射

后续 Rust 实现建议至少有这些对象：

```text
RolePromptPack
ModelFamilyOverlayPack
ProjectPolicyPack
PromptLineageRecord
```

它们最终编译到当前已有结构化 block：

- `role_prompt.current_prompt_summary`
- `role_prompt.prompt_history`
- `role_prompt.prompt_lineage`
- `role_prompt.prompt_modules`
- `role_prompt.output_contract`

当前阶段先冻结 ownership 与内容分层，不在本文展开最终 Rust type 细节。

---

## 10. 最小验证要求

role prompt / model overlay 变更时，至少验证：

1. 不同 role 的 `prompt_modules` 与 `current_prompt_summary` 能清楚区分
2. role 切换不会污染 project/session/turn overlays 的 ownership
3. Web debug 能看到 role lineage / module list / output contract
4. 旧 session artifacts 仍可兼容读取
5. prompt 改动不会把 tool spec 与 role baseline 混成一个字段

---

## 11. 当前非目标

当前不冻结：

1. 每个 role 的最终 raw prompt 全文
2. 每个模型家族的完整最终 overlay 文本
3. provider 级 prompt caching key 算法
4. topic-revival 时的最终 retrieval scoring 算法
5. project policy compiler 的最终实现细节
