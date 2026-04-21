# 37 Project Task System And Owner Loop

本文档冻结 `fin` 当前讨论得到的 project task system 工作模型。

目标：

1. 把复杂任务统一纳入 project task system 管理
2. 冻结 `epic -> task -> claim -> review` 的最小工作流
3. 明确 system/project agent 与 worker 的职责边界
4. 给后续任务工具、project board、dispatch/review 实现提供真源

---

## 1. 核心结论

`fin` 的复杂执行不应靠临时聊天语义管理。

冻结为两条路径：

### 1.1 Simple direct path

适用于：

- 简单任务
- 单次 closure 或少量 closure 可收口
- 不需要并行 worker
- 不需要长期 review / unblock / dependency 管理

规则：

- 不建 epic
- 不进入大型 project task system
- 只用 `update_plan` 记录最小步骤与结果
- 可由 system agent 直接完成，或由 project agent 做轻量处理

### 1.2 Managed project path

适用于：

- 复杂任务
- 需要拆分
- 需要并行
- 需要 owner / claim / review
- 需要 dependency / unblock / delivery 闭环

规则：

- 必须进入统一的 project task system
- 大目标建 `epic`
- `epic` 下拆 `task`
- 所有复杂派发都必须经过 project task system，不能绕过真源私聊 worker

---

## 2. Task-system 层级

当前先冻结最小两层：

```text
epic
  -> task
```

说明：

- `epic`：大目标容器
- `task`：最主要的派发与执行单位

当前不强制引入第三层 `subtask/work_item`，避免过早复杂化。

---

## 3. 角色边界

### 3.1 system agent

system agent 是：

- 全局 owner / dispatcher / reviewer
- 用户入口与前台协调者
- 多 project / 多 task 的 portfolio manager

它负责：

- 看当前 backlog / task board
- 接收用户新目标
- 判断优先级
- 创建或更新 epic
- dispatch 给 project agent / worker
- ingest completion/progress/failure
- 做 unblock analysis
- 统一向用户汇报

### 3.2 project agent

project agent 是：

- 单项目 owner / dispatcher / reviewer
- 项目范围内 epic / task 的执行管理者

它负责：

- 看当前 project task board
- 找出 ready / unclaimed task
- 派发给 worker
- review worker 提交结果
- 解锁后续任务
- 更新项目级计划与状态

### 3.3 worker

worker 不是新的 prompt role。

worker 是：

- `project role` 的 runtime 执行体
- 负责 claim task、执行、提交结果
- 不负责全局调度
- 不负责最终任务验收

---

## 4. 发布者与 Review Owner 规则

冻结规则：

> 谁发布任务，谁 review。

具体含义：

- `system agent` 创建并分派的任务，由 `system agent` 做 review owner
- `project agent` 创建并分派的任务，由 `project agent` 做 review owner
- worker 只提交结果，不承担最终验收

这样可以稳定确定：

- 谁批准完成
- 谁决定 reopen
- 谁根据结果更新计划
- 谁负责给上层汇报

---

## 5. Worker 工作流

worker 的最小生命周期冻结为：

```text
claim task
-> execute
-> submit for review
-> wait review decision
-> done or reopen
```

说明：

1. worker 必须先 claim task，再开始执行
2. worker 执行后提交 result / progress / note / artifact
3. review owner 决定 approve / reject / reopen
4. 若 reopen，则 worker 再继续执行

补充冻结（2026-04-20）：

- 当 framework 已知某个 local project agent 处于 `resume_ready`，并且存在 `resume_task_id` 时，
  `resume_project_task` 不能只停留在 supervision 文本上。
- 当前最小执行骨架已升级为：

```text
resume_ready supervision
-> execution handoff materialized
-> runtime handoff_project_task(...)
-> task status = claimed
-> waiting for local project runtime pickup
```

- 这意味着 task system 当前已经出现第一条 framework-owned handoff truth：
  - control plane 决定“该接续哪个 task”
  - runtime task store 负责 claim / persist / board refresh
  - 后续 detached/local runtime 只需要接这份 handoff truth，不再重新猜测 resume 目标

补充冻结（2026-04-21，worker assignment bridge）：

- 当 owner 在同一条 ready-task dispatch turn 中执行：
  - `agent.assign`
  - `project.task.claim`
- 框架现在要求 assignment queue 必须携带足够的执行真相，而不是只留自然语言摘要：
  - `project_id`
  - `session_id`
  - `task_id`
  - `target_worker_id`
  - `target_agent_name`
- 最小闭环升级为：

```text
owner dispatch
-> assignment queue(pending)
-> assignment runtime resume
-> enqueue hidden project.assignment input
-> worker project turn executes
-> project.task.submit
-> task status = submitted
```

- 规则：
  1. `project.assignment` 属于 framework-owned hidden input，不进入用户可见 conversation
  2. worker submit 必须继续走统一 `project.task.submit` truth，不能由 adapter/daemon 直接篡改 task status
  3. assignment queue 是 worker pickup 的 durable truth；`current_assignment_summary` 只是观测投影
  4. owner review 仍然走既有 owner-loop `review_submitted_task` 路径；assignment bridge 只负责补齐 dispatch 到 submit 这段缺口

---

## 6. Owner Loop

system agent 与 project agent 都采用 owner loop。

```text
inspect current task board
-> find ready and unclaimed tasks
-> dispatch if resources allow
-> wait or ingest feedback
-> review submitted tasks
-> run unblock analysis
-> update board
-> report upward / to user
```

冻结规则：

1. 只要还有未阻塞、未认领、可推进的任务，owner agent 就应优先派发
2. 直到当前 ready task 为空，或所有可执行任务都已有人 `working`
3. 之后再处理其它新输入、新任务或优先级变化

### 6.1 Framework-owned owner-loop decision truth（2026-04-21）

为避免 owner loop 只停留在提示词层，当前冻结一份 framework-owned 中间真相：

- runtime/scheduler 会从 managed task registry 派生 `owner_loop_action`
- 该 artifact 先只表达“当前 owner 应做什么”，暂不直接替模型执行
- 最小 action kinds：
  - `review_submitted_task`
  - `dispatch_ready_task`
  - `wait_worker_feedback`
  - `stay_idle_no_actionable_task`
  - `no_managed_tasks`

当前接线顺序冻结为：

```text
managed task registry
-> owner_loop_action artifact
-> scheduler/supervisor decision surface
-> status/debug/web 观测
-> 后续再把 decision 接成真实 dispatch/review control action
```

规则：

1. `submitted` 优先于 `ready`
2. `ready` 优先于单纯 `working/waiting`
3. 没有 actionable managed tasks 时，不伪造 dispatch/review
4. owner-loop artifact 必须从 task registry 真源派生，不能从聊天文本猜测

### 6.2 Minimum executable handoff（2026-04-21）

在 `OwnerLoopActionRecord` 已稳定后，当前再冻结一层**最小可执行 handoff**：

- 仅对以下 action kinds 自动触发 framework-owned inference：
  - `review_submitted_task`
  - `dispatch_ready_task`
- 触发方式不是 framework 直接改 task，而是注入一条隐藏的 framework prompt：
  - `framework.owner_loop.review_submitted_task`
  - `framework.owner_loop.dispatch_ready_task`
- 该 prompt 不进入用户可见 conversation，但会进入 runtime/provider/debug truth

边界：

1. `wait_worker_feedback` 仍然只作为 control-plane blocked truth，不自动发起模型推理
2. 单个 scheduler cycle 最多只自动执行一次 owner-loop framework turn，避免在 task state 未变化时无界重复
3. 真正的 claim/review 仍由统一 task tools 完成；framework 只负责把 owner-loop 意图推进到可执行推理

---

## 7. New Input Handling Against Backlog

owner agent 处理新输入前，必须先看当前任务盘。

新输入处理顺序冻结为：

```text
inspect current backlog
-> classify new input
-> compare priority with current tasks
-> decide simple direct / create epic / dispatch now / queue / ask confirm
-> update board
```

## 7.1 Formal task 后的 planning split（2026-04-21）

`TentativeSession -> /formalize` 只完成正式 bind，不直接承担完整 task decomposition。

冻结为：

```text
/formalize
-> framework formalization(task/topic bind)
-> hidden framework.task_kickoff.plan
-> first planning turn
-> choose:
   - simple direct path(update_plan)
   - managed project path(project.task.create ...)
```

规则：

1. formalize 与 planning 分离，避免“bind task/topic”和“拆任务/建 board”混成一步
2. planning kickoff 由 framework 自动 enqueue，不要求用户再次发命令
3. planning kickoff 属于 hidden framework input，不写入用户可见 conversation
4. direct/managed 的判断权在 planning turn；session/task bind 的控制权在 framework
5. `formalize_kickoff` 同一 scheduler cycle 只跑 planning 一轮，不继续串 deeper owner-loop，避免刚 formalize 就连环自动推进多轮

高优先级任务规则：

- 若新任务是高优先级，owner agent 应先做最小分析
- 然后尽快 dispatch 给 worker / project path
- 再回到整体协调，而不是长期沉入该任务执行

---

## 8. Completion Is A Scheduling Signal

任务完成不是简单的 `done` 信号。

冻结规则：

- 每个 completion 到来时，owner agent 都必须做 `unblock analysis`
- completion 可能会：
  - 解锁 blocked tasks
  - 改变 waiting tasks 的状态
  - 推动 epic 进入下一阶段
  - 触发新的 dispatch
  - 改变当前 focus 与优先级

因此：

```text
completion
-> update task state
-> run unblock analysis
-> unlock downstream tasks
-> reprioritize
-> update board
```

---

## 9. Minimal Status Vocabulary

当前只冻结概念，不冻结最终字段 schema。

最小状态词汇建议：

- `created`
- `ready`
- `claimed`
- `working`
- `submitted`
- `reviewing`
- `done`
- `blocked`
- `cancelled`

这些状态后续可细化，但不应破坏最小 owner loop。

---

## 10. Relationship With System Direct Execution

本 task system 不否认简单任务的直接执行路径。

冻结边界：

- 简单任务：走 `update_plan` 轻量路径
- 复杂任务：进入 `epic/task` 管理路径
- system agent 的 direct execution 预算按 closure 控制；一旦连续 2~3 个 closure 仍未明显收口，就应升级到 managed project path

---

## 11. 当前非目标

当前先不在本文冻结：

1. task / epic 的最终字段级 contract
2. claim lease 的精确 TTL 机制
3. review reject / reopen / partial accept 的完整状态机
4. scheduler / daemon 的资源配额与公平策略
5. UI / channel 的最终 task board 呈现格式
