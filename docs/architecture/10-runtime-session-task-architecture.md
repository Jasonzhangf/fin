# 10 Runtime, Session, Topic, and Task Architecture

本文档收口 `fin` 当前已确认的核心运行时对象与边界。它只回答：

- 哪些对象存在
- 它们负责什么
- 谁拥有控制权
- 哪些东西由框架做，哪些由模型做

## 1. 核心身份模型

- `RoleDefinition`
  - 角色定义、提示词模板、行为约束、工具策略
- `AgentIdentity`
  - 某个 role 的逻辑身份
- `WorkerRuntime`
  - 真实执行载体；一个 role / agent 可以对应多个 worker runtime

原则：

- Rust runtime 是执行真源
- Web 是输入输出与调试窗口，不是执行真源

## 2. Session 与 Context

- `Session`
  - durable substrate
  - 不是 prompt 本身
  - 是执行与重建的长期容器
- `Context`
  - 单次推理调用的动态装配结果
- `ContextView`
  - 从 Session 中按 role / worker / task / current input 构建出来的本轮上下文

### Session 内部分层

- `RunJournal`
  - worker/session 局部原始流水账
- `CollabSpace`
  - 当前协作链路共享事实空间
- `KnowledgeArtifactStore`
  - 长期可复用知识层
- `ControlState / SessionState`
  - 框架控制与重建辅助状态
- `ExecutionNote`
  - 中层持续工作笔记
- `Digest`
  - closure 级压缩上下文块

## 3. Topic 与 Task

- `TopicThread`
  - 长期话题主线
  - 支持 revive / rebind / merge / split
- `Task`
  - 状态机优先的执行单元
  - 连续多轮讨论默认尽量复用同一个 `task_id`
- `Dispatch`
  - 执行分配真源
- `ProgressBlock`
  - 实时推进真源

### Topic 与 Task 的关系

- topic 解决“长期属于哪条主线”
- task 解决“当前在做什么”
- 一个 `TopicThread` 下可以挂多个 `Task`

## 4. Promotion 边界

### `ProgressBlock`

负责：

- tool execution snapshots
- current phase
- blocker
- next step
- health hints

特点：

- 高频
- 实时
- 偏监控

### `ExecutionNote`

负责：

- 每轮 control feedback 的重要沉淀
- subtask / plan change
- meaningful progress
- lesson / decision / handoff

特点：

- 持续流式积累
- 比 progress 更稳定
- 比 digest 更细

### `Digest`

负责：

- 每个完整 closure 的压缩归档
- continuity 的历史块
- rebuild 的关键材料

规则：

- 每个完整 closure 必须生成一个 digest
- 中断不构成独立 closure
- 中断片段并入最近有效 closure / 恢复后的 closure

### `KnowledgeArtifact`

负责：

- 执行阶段结束后的稳定总结
- retrieval / sharing 的主通道

## 5. TentativeSession 与正式任务

- 新输入到来且无正式 `task_id` 时，先建立 `TentativeSession`
- Tentative 阶段由框架收集：
  - 当前内容摘要
  - 用户真实目标
  - 一句话 preview
  - simple query / continuity / topic confidence
  - candidate binding
- 若只是 simple chat，则保持内存态轻量路径
- 若意图足够清晰且需要正式任务编排，则由框架在用户确认后执行正式 `task creation operation`
- tentative 内容在 formalize 后并入首个正式 closure

## 6. Routing 与控制权

### 模型负责输出结构化判断

通过 `RoutingFeedbackBlock` 在每轮结束时输出：

- `candidate_task_id`
- `candidate_topic_thread_id`
- `continuity_confidence`
- `topic_shift_confidence`
- `simple_query_confidence`
- `reason`

### 框架负责控制动作

框架根据 confidence + policy 决定：

- continue current
- pending observation
- prompt switch / confirm
- start new task
- rebind existing thread
- side topic

规则：

- session/topic 的切换与询问由框架负责
- 模型不直接向用户发起控制性询问

## 7. Workdir 级共享

- 同一 workdirectory 下多个 session 不直接共享原始 session history
- 实时协作通过 `CollabSpace`
- 价值共享通过 `KnowledgeArtifactStore`

关键 scope：

- `worker_local`
- `session_local`
- `task_shared`
- `workdir_shared`
- `repo_shared`
- `global`

## 8. ContextView 默认构成

`ContextView` 默认包含：

- `ControlBlock`
- `TaskListBlock`
- `TopicListBlock`
- Current Task / Dispatch State
- Current Continuity Raw Window
- Relevant Collab Deltas
- Historical Digests
- Retrieved KnowledgeArtifacts

## 9. 当前非目标

以下内容属于后续模块阶段再展开的细节，不在当前总框架文档中深挖：

- 具体存储格式
- wire shape
- 排序公式
- UI 组件细节
- crate 内部模块拆分
