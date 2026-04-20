# Prompt Module Contract

本文档定义 `fin` prompt system 的最小结构契约。

目标：

- 给 prompt sources / assemblers / Web debug 一个共同结构边界
- 保证 prompt system 可观测、可测试、可演进
- 避免重新回到“单一大字符串 system prompt”模式

---

## 1. Prompt Module Entry

最小结构：

```text
module_id: string
title: string
summary: string
source: string
priority: number
```

字段说明：

- `module_id`：稳定标识
- `title`：显示名
- `summary`：当前模块摘要
- `source`：来源（role/profile/framework/project/tooling）
- `priority`：装配优先级或稳定排序值

约束：

- `module_id` 必须稳定
- `summary` 应短而硬，避免大段 narrative
- 同一 role family 下，module order 必须可重复

---

## 2. Role Prompt Block

最小结构：

```text
role_id: string
current_prompt_summary: string
prompt_history: string[]
prompt_lineage: string[]
prompt_modules: PromptModuleEntry[]
behavior_rules: string[]
output_contract: string[]
```

字段说明：

- `current_prompt_summary`：当前生效 prompt 的摘要
- `prompt_history`：最近连续任务的 prompt continuity 摘要
- `prompt_lineage`：本轮 prompt 来源链
- `prompt_modules`：当前加载的模块集合
- `behavior_rules`：硬行为规则
- `output_contract`：输出要求

兼容要求：

- 旧字段 `prompt_summary` 必须允许兼容映射到 `current_prompt_summary`

---

## 3. Tool Catalog Entry

最小结构：

```text
tool_name: string
kind: string
summary: string
purpose: string
when_to_use: string[]
when_not_to_use: string[]
input_schema_summary: string
output_schema_summary: string
side_effects: string[]
example_uses: string[]
```

要求：

- `model_tools` 与 `framework_tools` 使用同样的 entry shape
- 但语义必须清楚区分：
  - `model_tools`：模型可直接选择
  - `framework_tools`：框架内部能力，不可伪装为模型直接调用
- `when_to_use / when_not_to_use / input_schema_summary / output_schema_summary / example_uses`
  不是仅供 Web 展示的“装饰字段”，而是 prompt contract 的一部分；`ModelInputAssembler`
  必须把这些信息真实暴露给模型，避免退化回“只有工具名 + 一句 summary”的不可调用 catalog
- 对存在显式调用策略的工具，contract 必须把策略钉死到可见字段里。例如：
  - `apply_patch`：默认优先 `mode=replace`（单点精确改动）
  - 仅在多文件 / add / delete / move 时使用 `mode=patch`

---

## 4. Tool Catalog Block

最小结构：

```text
model_tools: ToolCatalogEntry[]
framework_tools: ToolCatalogEntry[]
tool_selection_policy: string[]
disabled_tools: string[]
hard_guards: string[]
```

要求：

- `tool_selection_policy` 必须解释模型可否选工具
- `disabled_tools` 必须说明本轮禁用或系统永远禁用的路径
- `hard_guards` 必须覆盖 truth boundary 与 side-effect boundary

---

## 5. Project Ref

最小结构：

```text
project_id: string
label: string
root: string | null
state: string | null
```

用于：

- `primary_project`
- `active_projects`
- `projects`

---

## 6. Project Context Block

最小结构：

```text
primary_project: ProjectRef | null
active_projects: ProjectRef[]
projects: ProjectRef[]
project_label: string | null
project_root: string | null
runtime_home: string | null
cwd: string | null
selected_paths: string[]
relative_selected_paths: string[]
scope_summary: string | null
focus_summary: string | null
active_task_id: string | null
task_board_summary: string | null
known_task_ids: string[]
active_agent_ids: string[]
agent_presence_summary: string | null
supervision_actions: string[]
project_supervision_summary: string | null
```

语义要求：

- `primary_project`：当前主要 project
- `active_projects`：当前推理涉及的活跃 projects
- `projects`：当前 agent 已知 projects 列表
- `active_task_id / task_board_summary / known_task_ids`：当前 task board 摘要
- `active_agent_ids / agent_presence_summary`：framework-owned agent presence 摘要
- `supervision_actions / project_supervision_summary`：framework-owned supervision 摘要

---

## 7. Prompt Build Output Contract

`ModelInputAssembler` 最终至少要能解释出：

```text
stable_core_prompt
role_prompt_modules
session_overlay
turn_context_envelope
rendered_model_input
```

当前 M1 Web 不要求把以上所有 raw text 全量显示，
但必须能从结构化 block 推断：

- 当前 role 是谁
- 当前 prompt modules 是哪些
- 当前 output contract 是什么
- 当前 tools 的选择政策是什么
- 当前 project scope / active projects 是什么

另外，tool catalog 在最终 `rendered_model_input` 里至少要暴露这些子行：

- `use: ...`
- `avoid: ...`
- `input: ...`
- `output: ...`
- `example: ...`

否则视为 prompt contract 未被真正装配到模型输入。

---

## 8. 最小验证要求

prompt contract 变更时，至少验证：

1. Rust schema serialize / deserialize 正常
2. 旧 session artifacts 可兼容反序列化
3. Web debug 可显示新增字段
4. `current_context.json` / `recent_contexts.json` 可真实落盘

---

## 9. 当前非目标

当前不在本 contract 中冻结：

- 最终 raw system prompt 文本模板
- prompt cache key 算法
- provider-specific wire payload
- tool calling 最终协议细节
