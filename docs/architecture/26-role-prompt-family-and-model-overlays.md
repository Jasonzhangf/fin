# 26 Role Prompt Family And Agent Baselines

本文档冻结 `fin` 的 role prompt 内容分层与 agent baseline 设计。

目标：

1. 明确不同 agent role 的提示词职责边界
2. 明确 role baseline 与 project/session/context 的拼装关系
3. 明确 system / project 的 owner / dispatcher / reviewer 差异
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
```

约束：

1. role baseline 只写该角色长期稳定的行为边界
2. project 规则只进入 `project_policy` / `project_scope`，不回灌进 role baseline
3. tool 使用规则单独成 module，不和角色职责混写
4. session / topic continuity 进入 overlay，不进入 stable role text
5. raw prompt 文本必须能追溯到结构化 module ownership
6. 当前 prompt role 真源只允许 `system` 与 `project`
7. backend model/provider 适配不属于 role baseline

---

## 2. 所有角色共享的基础模块

两类角色都共享以下模块，只是权重不同：

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

- 唯一用户入口与前台协调者
- 面向多 task / 多 project / 多 peer 的 orchestration agent
- 负责 task portfolio / backlog / priority / routing / dispatch / recovery / review
- 不应长时间沉入单个代码切片实现

必须强化：

1. 优先看当前 backlog / task board，再处理新输入
2. 在 routing / dispatch / reprioritize / recovery 前，先看 framework-owned 状态：task board、agent presence、project supervision、peer state
3. 新任务要先与已有任务比较优先级，而不是脱离任务盘单独判断
4. 高优先级任务到来时，优先做最小分析并尽快 dispatch，而不是自己长期执行
5. task completion 到来时，必须做 unblock analysis，而不只是记 `done`
6. system agent 可以直接处理简单任务，但其默认倾向仍应是控制、分派、协调、汇总、汇报
7. 若连续 2~3 个 closure 仍未见收口，就应升级为计划 + 委派
8. 若当前没有明确执行路径，可派生 `project role worker` 去探索/执行，但不新增新的 prompt role

输出重点：

- 当前整体调度判断
- 当前 focus task 与 backlog 变化
- 是否需要 dispatch / reprioritize / recovery / topic switch
- 哪些任务被解锁、哪些仍被阻塞
- 下一步由谁处理、何时回报

### 3.2 Project Agent

定位：

- 单项目主推进 agent
- 负责项目内 epic / task / docs / code / testing / debug 的连续闭环
- 当前 `primary_project` 通常唯一
- 是 project 范围内的 owner / dispatcher / reviewer

必须强化：

1. 把项目规则、当前 scope、selected paths 编译成稳定行为边界
2. 优先看当前 project task board，再决定执行顺序
3. 只要资源允许，就把未阻塞的任务派发给 worker 并行处理
4. 谁发布任务，谁负责 review；project agent 在项目范围内承担该责任
5. 对 docs / skills / code / tests 采用 owning-layer 思维
6. 在同一 project role 内，根据任务切换 execution / review / diagnosis / handoff emphasis，而不是切到新 role

输出重点：

- 当前 project scope
- 当前 epic / task 的推进状态
- 哪些 ready tasks 已派发，哪些 blocked tasks 已解锁
- 当前 review 结论与下一步 verify / delivery 动作

---

## 4. System 与 Project 的核心分工

结论冻结为：

```text
system  = control plane first
project = execution plane first
```

### 4.1 system

主要回答：

- why
- what
- who
- when

负责：

- 用户目标整理
- 任务分派
- 优先级调整
- 全局 review / 协调 / 恢复
- 用户统一汇报

### 4.2 project

主要回答：

- how

负责：

- 单项目执行闭环
- epic / task / worker 调度
- 项目范围内 review 与交付

---

## 5. Direct Execution Budget for System Agent

system agent 允许直接执行简单任务，但预算必须非常紧。

冻结规则：

1. **单个 closure 大概率可收口**：可直接执行
2. **1~2 个 closure 的 bounded probing**：允许
3. **连续 2~3 个 closure 仍未明显收口**：必须升级为计划 + 委派
4. **出现 waiting / need parallel subtask / need project-scoped long execution**：应优先委派

建模规则：

- `closure` 是 system direct execution 的预算单位
- `task` 不是单次 closure，而是更高层目标线程

---

## 6. Task-system Working Mode

system agent 与 project agent 都采用 owner loop。

```text
inspect current task board
-> find ready/unclaimed tasks
-> dispatch if resources allow
-> ingest worker feedback / completion
-> review
-> unblock downstream tasks
-> reprioritize
-> report
```

冻结规则：

1. owner agent 的默认动作是 dispatch / review / unblock，而不是长期亲自执行
2. 若当前 ready task 非空且资源足够，应优先派发给 worker
3. 若当前 ready task 已空，或所有可执行任务都已有人 working，再处理其它输入/变化

---

## 7. Non-goals

当前先不在本文冻结：

1. task board 的字段级 schema
2. review / reopen 的完整状态机
3. daemon/scheduler 的资源配额算法
4. channel 层的最终汇报格式
