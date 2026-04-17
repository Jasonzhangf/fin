# fin architecture note

Updated: 2026-04-17

## Current architecture discussion snapshot

### 1) Agent / Role / Runtime
- Agent 先有角色（Role）。
- 不同 Role 有不同系统提示词、能力边界、行为约束。
- 一个 Agent Role 可以对应很多 runtime / worker。
- 因此要明确区分：
  - Role：角色定义与提示词模板
  - Agent：某个角色身份
  - Worker Runtime：实际执行载体

### 2) Session / Ledger / Context rebuild
- 用户目标：
  - 不同 worker 可以保留各自本地流水账
  - 原始记录不能丢失
  - 多 worker 之间可以共享
  - 可以合并与压缩
  - 最终能 rebuild 为正确 context
  - token 级探索沉淀要能成为后续知识资产
- finger 现有思路：一个 ledger，多 track，session 是动态 view。
- 待进一步设计更优方案：在保留原始流水账的同时，把合并、压缩、共享、重建明确拆层。

### 3) Task model
- Task 明确采用状态机优先。

### 4) Operation + Event
- 系统设计采用 operation + event 模型。
- Event 既是 debug 记录，也是系统正常操作反馈机制的重要组成。
- 反馈、观测、调试优先依赖 event。

### 5) Coordination topology
- 多 agent 网络通信一定有中心编排者。
- 本地模式也保持同样架构。
- 因此 v1/v2 都以 central orchestrator 为骨架。

### 6) Web role
- Web 有两个作用：
  - WebUI 输入 / 输出
  - 渲染推理过程与更多 debug 信息
- Web 既是交互入口，也是观察窗口。

## Proposed interpretation to refine next

### A. Object model candidates
- RoleDefinition
- AgentIdentity
- WorkerRuntime
- Task
- Dispatch
- Session
- Event
- Operation
- ContextView

### B. Better-than-finger session direction candidate
Candidate split:
1. Raw Run Journal
   - 每个 worker/runtime 独立 append-only 原始流水账
   - 保留 token 探索、工具调用、局部思考过程、原始事件引用
2. Shared Collaboration Ledger
   - 共享协作层的事实事件流
   - 用于多 worker 合并、时序关联、ownership 与任务推进
3. Knowledge Extraction Layer
   - 从原始流水账与协作事件中提炼 observation / decision / artifact / summary
   - 每条知识都保留 provenance（来源引用）
4. Context Builder / View
   - 根据 role / task / session / worker 构建动态 context
   - 原始流水账不直接等于 prompt context

This direction keeps:
- raw history preserved
- mergeable multi-worker collaboration
- compressible knowledge artifacts
- rebuildable dynamic context

### C. Open questions
1. Session 是否应该分成“执行流水账”和“共享知识空间”两个概念？
2. Knowledge extraction 的最小单位是什么：message / turn / operation / event / artifact？
3. 多 worker 共享时，谁有写共享知识空间的权限：worker 直接写，还是 orchestrator / reducer 写？
4. rebuild context 时，优先级是否应为：task state > verified knowledge > local worker scratchpad > raw journal reference？
5. token 级探索是否全量落盘，还是只对特定 role / 特定 debug level 保留？

## Current recommendation direction
- Rust kernel
- central orchestrator
- task-state-machine-first
- operation accepted -> event committed
- event as operational truth for feedback/observability
- web as both interaction UI and debug console
- session architecture should evolve beyond single-ledger dynamic-view into a layered model:
  - raw journals
  - shared collaboration ledger
  - extracted knowledge/artifacts
  - dynamic context views

## 2026-04-17 additional architecture points

### 7) Session vs Context
- 需要更轻、更清楚地定义 session 和 context 的关系。
- session 的一部分要动态组合成 context。
- `RunJournal` 最新部分必须作为 context 中最重要的推理 history。
- 需要继续明确：
  - `CollabSpace` 如何进入 context
  - `KnowledgeArtifact` 如何通过 search + classification 进入 context
  - `ContextView` 的构建规则是什么

### 8) Control Block
- `ControlBlock` 很重要。
- 它需要在每次模型推理中持续积累反馈和数据。
- 新的用户请求和 agent 交互请求，应该能在一次推理中通过不同输入模块 / 输出模块让模型并行提供结果。

### 9) Framework-owned subconscious
- 系统框架应承担大量“潜意识式”能力，而不是都让模型显式推理得到。
- 重要基础能力包括：
  - 状态反馈
  - 记录
  - 通信
  - 生命周期推进
  - 调试与观测
- 这些应由框架自动维护，再以受控方式暴露给模型。

## New architecture direction to refine

### Session and Context relationship candidate
- Session 不是 prompt 本身，而是长期执行与协作的容器。
- Context 是某次具体推理调用的动态装配结果。
- 因此：
  - Session = durable substrate
  - Context = ephemeral view assembled for one inference

### Candidate context sources
1. ControlBlock
2. Task / Dispatch state
3. Recent local RunJournal slice
4. Relevant CollabSpace deltas
5. Retrieved KnowledgeArtifacts
6. User input / Agent input modules

### Candidate model I/O frame
- Input modules:
  - user_request
  - agent_request
  - control_state
  - recent_history
  - shared_updates
  - recalled_knowledge
- Output modules:
  - user_response
  - agent_messages
  - operation_proposals
  - artifact_proposals
  - control_feedback
  - local_scratch_updates

### Candidate framework-owned automatic responsibilities
- heartbeat / lease / ownership
- operation validation
- event append
- journal append
- retry bookkeeping
- token budget / compression trigger
- retrieval prefilter
- dispatch routing
- progress snapshots
- provenance tracking

## 2026-04-17 workdirectory-sharing decision draft

### 10) Multiple sessions under one workdirectory
- 同一个 workdirectory 下会同时存在多个 session / worker。
- 这些 session 不应该直接共享原始 session history。
- 有价值内容的共享，优先通过 `KnowledgeArtifact` + retrieval scope 完成，而不是直接合并 prompt 历史。

### Proposed sharing rule
1. `RunJournal`
   - 保持 worker/session 局部私有连续性
   - 不直接跨 session 全量共享
2. `CollabSpace`
   - 用于当前协作链路的共享事实
   - 适合 task/session 级协作，不适合作为长期知识共享真源
3. `KnowledgeArtifactStore`
   - 作为 workdirectory 下多 session 共享价值内容的主通道
   - 通过 scope + retrieval 控制共享范围

### Proposed scope model
- `worker_local`
- `session_local`
- `task_shared`
- `workdir_shared`
- `repo_shared`
- `global`

### Proposed default policy
- 同 task 多 worker：优先共享 `task_shared` + `CollabSpace`
- 同 workdirectory 不同 session：优先共享 `workdir_shared` artifacts
- 跨 repo / 跨项目：默认不共享，除非显式提升到更高 scope

### Proposed promotion rule
- 有价值内容不是直接广播，而是：
  RunJournal / CollabSpace -> candidate artifact -> classify -> verify/grade -> assign scope -> publish

### Why
- 避免 session 污染
- 避免把短期 scratch 当长期知识
- 保留原始记录，同时让高价值内容可复用

## 2026-04-17 accepted architecture conclusions

### A) Session / Context accepted conclusions
- Session = durable substrate，不是 prompt 本身。
- Context = 单次推理调用的动态装配结果。
- ContextView = 从 Session 中按 role / worker / task / current input 临时构建的推理视图。
- ContextView 构建优先级建议：
  1. ControlBlock
  2. Task / Dispatch state
  3. Recent local RunJournal slice
  4. Relevant CollabSpace deltas
  5. Retrieved KnowledgeArtifacts
  6. Current input modules
- `RunJournal` 最新 slice 必须作为 context 中最重要的连续推理 history。
- `CollabSpace` 进入 context 走 delta-first + relevance-first。
- `KnowledgeArtifact` 进入 context 走 classification + retrieval。

### B) Session internals accepted split
- `RunJournal`：worker/session 局部原始流水账
- `CollabSpace`：当前协作链路共享事实空间
- `KnowledgeArtifactStore`：可提炼、可搜索、可压缩、可共享的知识层
- `ControlState / SessionState`：框架控制与重建辅助状态
- `ContextViewBuilder`：按需动态构建 prompt-ready context

### C) Shared knowledge policy under same workdirectory
- 同一 workdirectory 下多个 session 不直接共享原始 session history。
- 实时协作通过 `CollabSpace`。
- 有价值内容共享通过 `KnowledgeArtifactStore`。
- 关键共享 scope：`workdir_shared`。
- promotion 流程：
  `RunJournal / CollabSpace -> candidate artifact -> classify -> verify/grade -> assign scope -> publish -> retrieval`
- scope model draft:
  - `worker_local`
  - `session_local`
  - `task_shared`
  - `workdir_shared`
  - `repo_shared`
  - `global`

### D) Operation + Event accepted conclusions
- 系统采用 operation + event 模型。
- Operation 表示请求 / 意图。
- Event 表示系统接受后的事实。
- projection / debug / replay / UI 以 Event 为主要事实流。
- Event 既是调试记录，也是正常反馈机制的重要组成。

### E) Project / Task / Control accepted conclusions
- Project 是 workdirectory 级协作域。
- Task 是状态机优先的工作单元。
- ProjectLeader 负责拆解任务、语义判断、主动追问、改派与收敛。
- Orchestrator / Scheduler 负责分配、派发、恢复与基础健康控制。
- 多 agent 网络通信与本地执行都采用 central orchestrator 架构。
- 框架负责“潜意识层”能力：
  - heartbeat
  - lease / ownership
  - dispatch bookkeeping
  - event append
  - journal append
  - retry bookkeeping
  - token budget / compression trigger
  - retrieval prefilter
  - progress snapshots
  - provenance tracking
- Web 既是输入/输出窗口，也是 debug / observability 窗口，但不是执行真源。

### F) Health / timeout accepted conclusions
- 心跳追踪由框架负责，不由 ProjectLeader 直接做底层活性检测。
- 需要同时追踪：
  - worker heartbeat
  - progress heartbeat
- soft-timeout：先框架基础健康检查，再升级给 ProjectLeader。
- hard-timeout：框架回收 lease / 标记 stale / 触发 recovery，ProjectLeader 决定重派或恢复。
- 共享给框架的是结构化推理状态，不是 full reasoning：
  - ControlBlock
  - ProgressBlock

## 2026-04-17 framework note / subtask / lessons requirement

### 11) Framework-owned execution note
- 每个 agent 执行过程中，需要有框架自动维护的 note。
- 这个 note 不是原始 chain-of-thought，而是把模型反馈的 `ControlBlock` / `ProgressBlock` 中的重要内容持续沉淀下来。
- 目的：
  1. 模型上下文有限，需要可持续压缩与续写
  2. 框架 / Web / 主 agent 需要看到长期推进情况

### 12) Subtask / update-plan / lessons visibility
- 框架需要持续知道：
  - 子 agent 当前 subtask
  - update plan 的变化
  - 每轮的经验教训 / lessons
- 这些都不应只存在于瞬时上下文里，而应变成可持续读取的结构化记录。

### Proposed direction
- 在 `ControlBlock` / `ProgressBlock` 之外，再引入一个框架拥有的中层对象：
  - `ExecutionNote` 或 `AgentNotebook`
- 作用：
  - 记录每轮最重要的推进摘要
  - 记录 subtask 变化
  - 记录 update plan 变化
  - 记录 blocker / handoff / lesson / decision
  - 为 context rebuild / Web progress / leader supervision 提供可持续材料

### Candidate note entry kinds
- `subtask_started`
- `subtask_updated`
- `plan_updated`
- `meaningful_progress`
- `blocker_detected`
- `decision_made`
- `question_raised`
- `lesson_learned`
- `handoff_prepared`
- `artifact_published`

### Candidate relationship
- `RunJournal`：原始本地流水账
- `ProgressBlock`：当前推进脉搏
- `ExecutionNote`：持续积累的中层工作笔记
- `KnowledgeArtifactStore`：经 promotion 后的长期可复用知识

## 2026-04-17 accepted promotion boundary conclusions

### 13) Recording ownership
- 记录一定由框架负责，不由模型直接拥有最终记录权。
- 模型只提供结构化反馈候选，框架负责分类、规范化、附 provenance、入库。

### 14) Progress / ExecutionNote / KnowledgeArtifact boundary
- 工具执行 snapshot 进入 `ProgressBlock`。
- 每一轮模型 `ControlBlock` 反馈的重要内容进入 `ExecutionNote`。
- 执行结束后，根据结论进行自我总结，形成 `KnowledgeArtifact`。
- 可以在 finish 阶段通过提示词加入 summary / artifact 提炼输出模块，但最终仍由框架接收和落库。

### Proposed final boundary rule
1. `ProgressBlock`
   - 当前执行脉搏
   - tool execution snapshots
   - current phase / blocker / next step / health hints
2. `ExecutionNote`
   - 每轮 control feedback 的重要沉淀
   - subtask / plan change / meaningful progress / lesson / decision / handoff
3. `KnowledgeArtifact`
   - 执行阶段结束后提炼出的稳定结论
   - 可带 verification / scope / provenance
   - 作为后续 retrieval 与共享主通道

## 2026-04-17 digest / rebuild accepted direction

### 15) Finger-style digest baseline
- 这部分以 finger 的做法作为原始基线思路。
- 上下文分块：
  - `KnowledgeArtifact` 通过关键字组合检索历史信息块，并做 digest 聚合（约 20k 预算）
  - 当前推理延续保留 `user -> assistant -> tool` 多轮连续历史
- 每轮推理关键部分由框架落为 digest。
- 当上下文变大时，把当前完整推理中的较早部分抽为 digest 加入 history，并通过窗口滚动滑动保留最新连续部分。

### 16) Digest content
- digest 应包含：
  - user 输入
  - 重要工具内容
  - update plan
  - 任务分派
  - agent 协作交互
  - note
  - summary

### 17) Context rebuild trigger
- 话题变化时进行 context rebuild。
- rebuild 时重新聚合：
  - 历史 `KnowledgeArtifact`
  - 之前连续三轮 task 内容
- 目标：避免丢失连续性，同时完成重组。

### 18) Minimum unit
- 最小单位定义为：
  - `task: user -> 本轮推理正确停止`
- 即一次 task turn / inference closure 作为最小 digest / note / control 归档边界。

## 2026-04-17 accepted digest generation conclusions

### 19) Digest generation timing
- digest 在每次任务 closure 结束时生成。
- 一个 closure 一定要生成一个 digest。
- 如果被中断，则不算一个完整 closure。
- 中断片段不单独形成最终 digest，而是和最近一次有效 closure / 后续恢复后的 closure 合并处理。

### 20) Digest and ExecutionNote relationship
- `ExecutionNote` 是 digest 的组成来源之一。
- `ExecutionNote` 不在任务结束时才生成；它在每个 control block 周期里都可能产生。
- note 由模型选择性给出候选内容，但由框架记录与规范化。
- closure 结束生成 digest 时，需要把 relevant `ExecutionNote` 纳入 digest 记录。

### Proposed rule
- `ExecutionNote` = continuous mid-run note stream
- `Digest` = closure-level compacted context block
- digest 的输入至少包括：
  - closure 内 relevant run slice
  - important tool snapshots
  - plan / subtask changes
  - collaboration messages
  - relevant execution notes
  - closure summary

## 2026-04-17 task id / continuity / simple-question requirement

### 21) Task ID continuity
- 连续会话的多轮讨论应尽量使用同一个 `task_id`。
- 这样对压缩、digest、rebuild、continuity tail 都更友好。
- 需要支持在 rebuild 时回溯修正任务归属，避免错误的话题切换判断。

### 22) ControlBlock continuity fields
- `ControlBlock` 需要明确字段判断：
  - 是否连续任务
  - last task 主题
  - 当前主题
  - 当前是否简单问题

### Draft interpretation
- `task_id` 应作为连续任务线程的稳定标识，但不应轻易直接改写底层原始记录。
- 若 rebuild 发现历史归属判断错误，应优先通过 rebind / reclassification / projection 修正，而不是破坏原始 provenance。
- `ControlBlock` 需要增加 continuity / topic / simplicity 分类字段，用于：
  - 是否延续上一个 task
  - 是否触发新 task
  - 是否走轻量上下文路径

## 2026-04-17 task list + confidence requirement

### 23) Task list in context
- Context 中需要显式保存一个 `task list`。
- 目的：
  - 让模型知道现有 task id 与内容的对应关系
  - 让模型辅助判断当前输入是否属于已有任务，还是应开启新任务

### 24) Confidence-based topic switch decision
- 系统需要模型给出 task continuity / topic switch 的置信度。
- 框架基于置信度判断是否：
  - 继续当前 task
  - 进入待确认状态
  - 切换到新话题 / 新任务

### Draft interpretation
- `task list` 应是 context 中的结构化块，而不是自由文本描述。
- 每个条目至少应包含：
  - task_id
  - title / summary
  - current state
  - current topic signature
  - last updated
- 模型输出应返回：
  - candidate_task_id
  - is_new_task
  - continuity_confidence
  - topic_shift_confidence
  - reason
- 框架不能只看 yes/no，要结合 confidence 做路由与是否触发 rebuild。

## 2026-04-17 topic revival / correction requirement

### 25) Historical topic revival and correction
- 历史话题需要可以复活、修正、复用。
- 过去关闭或挂起的话题，不应因为当前不活跃就永久失去可接续能力。
- 上下文应支持按话题归类、按话题接续，而不是只按线性会话向前滚动。

### Draft interpretation
- 需要把 `topic` 设计成一个可检索、可重绑定、可恢复的长期对象，而不仅是当前 task 的瞬时标签。
- 历史 topic 应可被：
  - revive（复活）
  - rebind（重绑定到当前 task/thread）
  - merge（合并到现有 topic/thread）
  - split（从现有 thread 拆出新 topic）

## 2026-04-17 session bootstrap / user choice / side-topic suggestion

### 26) New session starts as in-memory tentative session
- 每个新 session 可以先以内存态 tentative session 启动。
- 用户输入和模型初步讨论后，待模型更清楚地理解用户真实意图，再决定归属到哪个历史 topic/session，或是否新建。

### 27) User-visible session choice after intent clarification
- 在初步理解用户真实意图后，系统向用户展示已有 session / topic 列表。
- 由用户选择：
  - 新建 session/topic
  - 复用历史话题
- 这一步是用户显式确认，不完全由模型自动决定。

### 28) Side topic mode
- 在已有话题中，可以使用 side topic / btw 模式。
- side topic 默认不保存为正式长期 session/topic 主线，只作为临时旁路讨论。

### 29) Topic drift detection and explicit switch confirmation
- 如果在已有会话中发现话题偏移，模型可以主动询问用户是否切换 session/topic。
- 即 topic drift 可由模型提示，最终切换可由用户确认。

### Draft interpretation
- 需要引入：
  - `TentativeSession` / `TentativeTopicBinding`
  - `SessionChoicePrompt`
  - `SideTopicMode`
  - `TopicDriftPrompt`
- 初始几轮不必立即固化到正式 topic/thread；可先在 tentative 状态运行。
- 经用户确认后，再正式绑定到 existing topic 或 new topic。

## 2026-04-17 correction: session switch / prompt ownership

### 30) Ownership correction
- session/topic 的切换与询问由框架负责，不由模型直接向用户发起控制性询问。
- 模型只负责通过 `ControlBlock` / routing output 提供：
  - continuity confidence
  - topic shift confidence
  - simple-query confidence
  - candidate binding
  - reason

### 31) Framework-driven prompt decision
- 框架基于置信度判断是否触发：
  - session/topic 继续
  - session/topic 切换确认
  - 新建 topic/session 提示
  - side-topic 提示
- 用户可见的询问 / 切换确认属于框架控制流，不属于模型自由输出。

## 2026-04-17 tentative session to formal task rule

### 32) New session bootstrap rule
- 一个新的 session 开始时，先从内存态 `TentativeSession` 启动。
- 在和模型的交互中，框架持续获取模型反馈，用于识别：
  - session 当前内容
  - 用户真实目标
  - 简单一句话描述（用于后续展示给用户选择）
  - 是否 simple chat / simple query
  - 是否应绑定已有 topic/session
- 只有在意图足够清楚、且需要进入正式任务编排时，才正式建立 `task_id`。
- 若未进入正式任务流程，则保持为内存态，按 simple chat 路径处理。

### 33) Tentative session interpretation
- `TentativeSession` 是未正式绑定 task/topic 的临时交互容器。
- 它可以保留短期内存态连续对话和结构化 routing feedback。
- 若后续正式建 task，则 tentative 阶段内容应并入首个正式 closure。
- 若最终只是 simple chat，则 tentative 内容默认不进入正式长期 task/topic 体系。

## 2026-04-17 session creation closed-loop draft

### 34) Session creation framework loop
- 新输入到来时，如果尚无正式 `task_id`，则先进入 `TentativeSession`。
- 在该阶段，模型在回答用户问题的同时，于本轮结束时通过 `ControlBlock` / routing feedback 提供：
  - 任务主题判定
  - 目标确认
  - 任务类型判断
  - 目标拆解判断
- 当这些判断达到足够清晰度后，框架向用户总结：是否要做某个正式任务（xxx）。
- 用户确认后，框架调用 `task creation` 流程 / 工具，创建正式 task。
- 然后把 session 与 task 注册到框架，进入正式闭环推理。

### 35) Ongoing topic-switch loop
- 新输入进入后，模型在每轮推理结束时通过 `ControlBlock` 持续反馈换话题置信度。
- 当置信度高于阈值时，由框架提示用户是否换话题。
- 用户选择后，框架继续推动：
  - 继续当前 task/topic
  - 切换 topic
  - 新建 task/topic
  - side topic

### 36) Important ownership rule
- `task creation` 不是模型直接拥有的控制动作。
- 更准确地说：
  - 模型输出 task creation proposal / routing feedback
  - 框架在用户确认后执行正式 task creation operation

## 2026-04-17 consolidated accepted architecture snapshot (review-ready)

### A) Core identity and execution model
- `RoleDefinition`：角色定义、提示词模板、行为约束、工具策略。
- `AgentIdentity`：角色身份。
- `WorkerRuntime`：真实执行载体；一个 role/agent 可以对应多个 worker runtime。
- Rust 内核负责 runtime / orchestrator / transport / harness core；Web 负责输入输出与调试观察。

### B) Session / Context / Topic / Task
- `Session` = durable substrate，不是 prompt 本身。
- `Context` = 单次推理调用的动态装配结果。
- `ContextView` = 从 Session 中按 role / worker / task / current input 构建的推理视图。
- `TopicThread` = 长期话题主线，支持 revive / rebind / merge / split。
- `Task` = 状态机优先的执行单元；连续多轮讨论默认尽量复用同一个 `task_id`。
- 任务归属判断错误时，优先做 projection/rebind/reclassification，不轻易破坏原始记录 provenance。

### C) Session internals
- `RunJournal`：worker/session 局部原始流水账。
- `CollabSpace`：当前协作链路共享事实空间。
- `KnowledgeArtifactStore`：长期可复用知识层。
- `ControlState / SessionState`：框架控制与重建辅助状态。
- `ExecutionNote`：中层持续工作笔记。
- `Digest`：closure 级压缩上下文块。

### D) Promotion boundary
- `ProgressBlock`：当前执行脉搏；记录 tool execution snapshots、phase、blocker、next step、health hints。
- `ExecutionNote`：每轮 control feedback 的重要沉淀；记录 subtask / plan change / meaningful progress / lesson / decision / handoff。
- `Digest`：每个完整 closure 结束时生成；吸收 relevant run slice、重要工具、plan/subtask change、collab messages、relevant execution notes、closure summary。
- `KnowledgeArtifact`：执行阶段结束后的稳定总结；用于 retrieval / sharing。
- 记录一定由框架负责；模型只提供结构化候选反馈。

### E) Closure / digest rules
- 最小单位 = `task: user -> 本轮推理正确停止`。
- 每个完整 closure 必须生成一个 digest。
- 中断不构成 closure，不单独生成最终 digest；中断片段并入最近有效 closure / 恢复后的 closure。
- 默认保留最近若干 closure 的 continuity tail（当前建议默认 3，可按 role 调整）。

### F) Shared knowledge and scope
- 同一 workdirectory 下多个 session 不直接共享原始 session history。
- 实时协作通过 `CollabSpace`。
- 价值共享通过 `KnowledgeArtifactStore`。
- 关键共享 scope：
  - `worker_local`
  - `session_local`
  - `task_shared`
  - `workdir_shared`
  - `repo_shared`
  - `global`
- promotion 流程：`RunJournal / CollabSpace -> candidate artifact -> classify -> verify/grade -> assign scope -> publish -> retrieval`

### G) Control blocks and context blocks
- `ControlBlock`：框架给模型的控制上下文。
- `RoutingFeedbackBlock`：模型在本轮结束时回给框架的结构化路由判断。
- `TaskListBlock`：context 中显式的结构化 task 列表，帮助模型做 task continuity 判断。
- `TopicListBlock`：context 中显式的结构化 topic 列表，支持 revive / topic routing。
- ContextView 默认包含：
  - ControlBlock
  - TaskListBlock
  - TopicListBlock
  - Current Task / Dispatch State
  - Current Continuity Raw Window
  - Relevant Collab Deltas
  - Historical Digests
  - Retrieved KnowledgeArtifacts

### H) Routing feedback and confidence
- 模型每轮结束时输出 routing feedback，至少包含：
  - `candidate_task_id`
  - `candidate_topic_thread_id`
  - `continuity_confidence`
  - `topic_shift_confidence`
  - `simple_query_confidence`
  - `reason`
- 框架根据 confidence + policy 决定：
  - continue current
  - pending observation
  - prompt switch/confirm
  - start new task
  - rebind existing thread
  - side topic
- session/topic 的切换与询问由框架负责，不由模型直接向用户发起。

### I) Tentative session bootstrap
- 新输入到来且无正式 `task_id` 时，先建立 `TentativeSession`。
- Tentative 阶段由框架收集：
  - 当前内容摘要
  - 用户真实目标
  - 一句话 preview
  - simple query / continuity / topic confidence
  - candidate binding
- 若只是 simple chat，则保持内存态轻量路径，不进入正式 task 体系。
- 若意图足够清晰且需要正式任务编排，则由框架在用户确认后执行正式 `task creation operation`。
- tentative 阶段内容在 formalize 后并入首个正式 closure。

### J) Project / task control
- `Project` 是 workdirectory 级协作域。
- `ProjectLeader` 负责任务拆解、语义判断、主动追问、改派与收敛。
- `Orchestrator / Scheduler` 负责分配、派发、恢复与基础健康控制。
- `TaskGraph` 是工作结构真源。
- `Dispatch` 是执行分配真源。
- `ProgressBlock` 是实时推进真源。
- 需要同时追踪 worker heartbeat 与 progress heartbeat。
- soft-timeout：先框架健康检查，再升级给 ProjectLeader。
- hard-timeout：框架回收 lease / 标记 stale / 触发 recovery，ProjectLeader 决定重派或恢复。

### K) Framework-owned subconscious
- 下列能力必须由框架自动维护，而不是依赖模型显式推理得出：
  - heartbeat
  - lease / ownership
  - dispatch bookkeeping
  - event append
  - journal append
  - retry bookkeeping
  - token budget / compression trigger
  - retrieval prefilter
  - progress snapshots
  - provenance tracking
  - health checks
  - switch / confirm control prompts

### L) Pending next review topic
- 下一步审阅：`TentativeSession / RoutingFeedbackBlock / TaskListBlock / TopicListBlock / FormalizationOperation` 的状态机。

## 2026-04-17 config + provider + m1 scaffolding direction

### 37) Configuration architecture requirements
- 需要独立的配置模块。
- 用户配置尽量简单，只暴露必要且必须由用户配置的项。
- 系统内部模块配置也需要可配置，但不暴露给用户。
- 系统配置统一使用单个 system config 文件，而不是多个零散文件。
- 不做用户配置与系统配置的 merge；采用明确的 user-config -> system-config mapping / conversion。
- 用户配置项与系统配置项尽量互斥，避免双向覆盖和 merge 引发错误。

### 38) AI Provider module requirements
- 需要独立的 AI Provider 模块。
- 项目一开始就要支持多协议。
- 可以先复用 / 迁移 / 改造 finger 里的 provider 模块思路，但在 fin 中应保持独立边界。

### 39) Current development priority shift
- 当前最重要的问题不是继续深挖局部状态机细节，而是：
  - 看整体开发模块
  - 定最小可用脚手架（M1）
  - 定最小可观测 Web debug
  - 定模块分块与迭代顺序

### 40) M1 scaffolding implication
- M1 不只是最小推理模块 + 最小 Web debug。
- 在此之前必须先有两个基础独立模块：
  1. Config module
  2. AI Provider module
- 否则后面的 runtime / debug / task bootstrap 都会被配置和 provider 边界拖垮。


## 2026-04-17 runtime home + install/regression decisions

### 41) `~/.fin` runtime home layout
- `~/.fin` 作为 fin 唯一运行时家目录。
- 顶层固定分层：`config/`、`bin/`、`install/`、`runtime/`、`sessions/`、`workdirs/`、`logs/`、`diagnostics/`、`harness/`、`archive/`、`tmp/`。
- 编译临时物仍在 repo 内；被提升的安装物、日志、session、回归证据进入 `~/.fin`。

### 42) Session vs workdir filesystem split
- `sessions/YYYY/MM/<session-id>/` 参考 codex 的时间分桶。
- session 内保存：`events`、`journal`、`progress`、`notes`、`digests`、`closures`、`context`、`collab`、`tasks`、`topics`、`artifacts/candidates`。
- `workdirs/<workdir-id>/` 作为多 session / 多 worker 的共享域，保存 `task-graph`、`topics`、`collabspace`、`artifacts`、`health`、`retrieval`。
- 原始 session history 不跨 session 直接共享；共享只通过 workdir scope 的 artifact / collab / task graph。

### 43) Global install + regression flow
- 全局入口定义为 `~/.fin/bin/fin -> ~/.fin/install/current/bin/fin`。
- 安装目录固定分为：`staged/`、`versions/`、`current`、`previous`、`receipts/`。
- 每次构建闭环为：源码校验 -> staging -> 安装态回归 -> promote current -> post-install smoke。
- 未经过与改动层级匹配的回归验证，不得提升为 `current`。


### 44) Module-level debug + test rule
- 每个功能模块默认采用：共享函数化 + block 化 + 编排推进 + operation/event/projection debug 设计。
- 每个模块默认验证闭环：unit + function/contract + orchestration regression + installed-binary smoke（按影响范围触发）。
- 该规则沉淀到本地 skills，作为 fin 模块开发默认流程。


### 45) Globalize module design principles
- 模块级通用设计思想上收至全局 `coding-principals`：shared functions + blocks + orchestration、operation + event + projection、module debug baseline、module test baseline。
- fin 本地 skills 改为只保留 fin-specific 规则与证据落点，避免重复堆通用方法论。


### 46) Operation + event + projection architecture
- 运行事实模型冻结为：`operation -> state push -> append-only events -> projection`。
- Operation 是请求，不是事实；Event 是事实真源；Projection 是读取优化，不得发明业务语义。
- 所有关键 side effect 默认都有 `started / completed / failed` 三段事件。

### 47) Debug five-layer method
- debug 默认五层：raw event -> entity timeline -> causality chain -> current projection -> diagnostic bundle。
- Web / CLI / harness / CI 围绕同一事件真源工作；文本日志只是辅助，不是事实真源。
- 没有 raw event、没有 trace/causality、没有 replay 或等价回放，不算闭环调试。


### 48) M1-A minimal contracts frozen
- M1-A 当前先冻结六类 contract：operation envelope、event envelope、progress block、execution note、digest、projection view。
- `docs/contracts/` 作为 contract 文档真源，`rust/crates/contracts` 作为最小类型骨架。
- 下一步实现应围绕这些最小 schema 打通单 runtime 推理闭环与最小 debug MVP。


### 49) M1-B config + provider code skeleton
- 已进入 M1-B，先补 `fin-config` 与 `fin-provider` 的最小代码骨架。
- `fin-config` 先实现 user/system config、mapping、normalization、TOML parsing；`fin-provider` 先实现 protocol、descriptor、registry、最小 client 抽象。
- 为保证 workspace 前进，`fin-contracts` 先补最小兼容类型，避免旧 crate 因 contract 演进而失编译。


### 50) M1-C single runtime closure slice
- `fin-runtime` 已补最小单 runtime closure：接收 operation，产出 operation.accepted / inference.started / progress.updated / execution_note.appended / digest.finalized / operation.completed 事件链。
- `fin-debug-server` 已补最小 in-memory projector：消费 event stream，生成 current projection view。
- 这为后续接入真实 provider 调用、CLI debug entry、Web debug MVP 提供了最小垂直切片。


### 51) M1-D CLI debug entry + provider wiring
- `fin-cli` 已补最小命令入口：`config-check`、`runtime-demo`、`debug-projection`。
- `fin-runtime` 现已接入 `fin-provider` 的 descriptor/request 准备逻辑，并补了 `provider.request_started` / `provider.response_received` 事件。
- 这让 M1 从“模拟 closure”推进到“带 provider 语义的单 runtime debug slice”。


### 52) M1-E runtime home persistence + web data source
- `fin-cli home-init` 现在会真实初始化 `~/.fin` 最小目录骨架，并写入 `config/user.toml` 与 `config/system.toml`。
- `runtime-demo` / `debug-projection` 现在会把 session 事件流、latest progress/note/digest、runtime/current、runtime/projections 写入 runtime home。
- `fin-debug-server` 已补最小 debug snapshot 持久化：`current_projection.json`、`latest_events.jsonl`、`current_snapshot.json`，作为后续 Web Debug MVP 的最小数据源。


### 53) M1-F runtime home real smoke verified
- 已用真实 `~/.fin` 跑通 `home-init`、`runtime-demo`、`debug-projection`。
- 当前已确认落盘证据包含：`config/user.toml`、`config/system.toml`、`runtime/current/last_run.json`、`runtime/projections/current_projection.json`、`runtime/projections/current_snapshot.json`、`runtime/projections/latest_events.jsonl`、`sessions/2026/04/session-cli-demo/{events,progress,notes,digests}`。
- 这说明 M1 的 runtime 记录闭环与 Web 数据源闭环已在真实家目录上可见。


### 54) M1-G web debug MVP uses tiny HTTP + polling
- Web Debug MVP 当前采用最小 Rust HTTP server，直接暴露 `current_projection.json`、`current_snapshot.json`、`latest_events.jsonl`、`last_run.json`，不引入第二份业务语义。
- 前端先用轮询读取当前快照与事件流，展示 projection / last run / warnings / event stream / 基础过滤；WebSocket 留到后续模块阶段。


### 55) Provider architecture switches to LiteLLM gateway
- provider 保留统一核心 loop，不为不同协议复制第二套 runtime loop。
- 不同 agent / role / worker 可以绑定不同 provider policy，但执行后端统一走 LiteLLM gateway，而不是 fin 自己维护多协议 adapter。
- 请求视为 operation，响应视为 event，中间通过 gateway execution phase 隔离，不让 request/response shape 在 runtime 中直接耦合。


### 56) Event model freezes as producer / consumer + subscription
- event 不只是 debug log，而是 producer 发出事实、consumer 订阅消费的统一架构。
- debug、Web、CLI、harness、future channel bridge 只是不同 consumer，依赖同一 append-only event truth。
- 后续不同通道订阅不同 event family，但共享同一事件模型与调试架构，不复制第二套管道。


### 57) Provider path policy is explicit priority, no routing, no fallback
- provider 当前不做 routing；请求只根据显式 `provider.model` 发送。
- 当前不做 fallback；即使配置多个 provider path，也不隐式切换到后备 provider。
- 多 provider path 当前只支持 `priority` 方式分配，未来若扩展别的策略，必须独立插件化，而不是污染 provider gateway 主路径。


### 58) Operation/event headers and subscription ack/lease decision
- operation 与 event 都补充统一头字段：`sender_id`、`sequence`、`timestamp`、`protocol_version`，并继续保留模块级 `source`。
- subscription 可选支持 `ack_policy` 与 `lease_ttl_ms`，但 ack 不是强制；debug/CLI/Web 这类 pull reader 默认可以只靠 cursor。


### 59) event.sequence is truth anchor, task_sequence is optional local order
- `event.sequence` 冻结为 append-only event log sequence，用于 replay、subscription cursor、fan-out 对齐。
- `task_sequence` 只是 task 内的可选局部顺序，适合 digest / task timeline / 局部调试，不替代底层 log sequence。
- `operation.sequence` 可以保留，但它只服务提交侧排序，不是最终消费真源。


### 60) External ingress channel is mailbox/eventbus after minimal agent core
- 等最小 agent 核心稳定后，再进入外部消息通道设计。
- `mailbox` 只做定向同步/异步投递与握手；`eventbus` 只做通知与 fan-out。
- 外部消息最终仍需正规化为 operation 或 event，不能让 mailbox/eventbus 成为第二真源。


### 61) Internal pipeline vs external channel vs multi-agent communication boundary
- 内部主流水线冻结为：`operation -> runtime push -> event -> subscription/projection`，不走 mailbox/eventbus。
- 外部入口先走 `mailbox/eventbus`，用于定向投递、通知、握手，然后再正规化为内部 operation/event。
- 多 agent 通信默认规则：定向消息走 `mailbox`，广播通知走 `eventbus`，系统事实仍然只通过 operation/event 落盘与调试。


### 62) External agent RPC kept as future ingress interface for cross-machine communication
- 外部 agent 的跨进程/跨机器/跨网段调用后续优先考虑 RPC ingress，而不是直接耦合到内部 runtime 主流水线。
- RPC、mailbox、eventbus 的职责拆分为：RPC 做 request/response ingress，mailbox 做定向投递与握手，eventbus 做通知与 fan-out。
- 当前阶段只冻结 RPC 接口边界与最小字段，不做具体 transport / codec / auth / reconnect 实现。


### 63) Common external message header and channel-specific handshake states
- 外部 RPC / mailbox / eventbus 共用一组公共消息头：`message_id`、`protocol_version`、`timestamp`、`source`、`sender_id`、`recipient_id?`、`trace_id`、`correlation_id?`、`causation_id?`、`sequence?`、`channel_kind`、`message_kind`、`delivery_mode`、`ack_policy?`、`lease_ttl_ms?`、`deadline_ms?`、`auth_context?`。
- 握手状态机冻结为共享最小集合：`created`、`delivered`、`accepted`、`rejected`、`expired`、`completed`、`failed`，但按 RPC / mailbox / eventbus 各自裁剪使用。
- 外部 envelope 只是 ingress 格式，不是内部真相；所有外部消息最终仍要 materialize 成 operation 或 event。


### 64) M1 minimal agent core module cut is frozen before further implementation
- M1 最小 agent core 冻结为七块：role/runtime policy、inference operation builder、provider gateway slice、recording slice、event store + projection、debug entry、subscription-ready read boundary。
- M1 主流水线固定为：`role/provider policy -> inference operation -> provider events -> progress/note/digest -> event store -> projection/snapshot -> cli/web debug`。
- mailbox/eventbus/RPC 先保留接口，不进入 M1 agent core 主实现；当前 crate 边界必须优先服务 execution truth 与 debug truth。


### 65) M1 first real implementation order is frozen before coding
- M1 第一批真实实现固定按 4 步推进：`role/runtime policy -> inference operation builder -> provider operation->event slice -> recording/projection/debug`。
- 每一步都指定了 owning crate 与最小文件落点，避免 provider/runtime/debug 一起乱改。
- 当前阶段禁止顺手把 mailbox/eventbus/RPC/cluster 带进这 4 步实现。

### 66) Step 1 lands role/runtime policy without touching provider execution semantics
- Step 1 只落 `contracts/config/runtime` 的 role/runtime policy 最小真边界：`AgentId`/`RoleId`/`ProviderPath`/`ProviderStrategy`、system policy mapping、`RuntimePolicySnapshot`/`WorkerRuntime`。
- user config 继续保持简单，只填 provider 必要信息；system policy 自动映射出 `default role -> explicit provider.model priority path`，不引入 user/system merge 歧义。
- envelope 字段与 provider execution/event 链重构保持到 Step 2/3，避免在 Step 1 提前引发 debug-server/cli/provider 的大面积返工。


### 67) Development gate and test isolation are frozen before deeper module work
- 代码文件默认门禁：除极少数白名单外，单文件必须 `< 500` 行；门禁脚本为 `scripts/check-code-line-limit.py`。
- 功能测试顺序冻结为：`provider config -> provider slice -> runtime builder -> recording/projection -> debug/install`，先基础通，再叠加复杂语义。
- 开发阶段默认测试 provider 来源固定为 `~/.rcc/provider/ali-coding-plan/config.v2.json`，并先生成隔离测试用 `user.toml`。
- 测试 session 不能污染正常运行目录：测试配置、runtime home、session namespace 都要与正常 `~/.fin` 分离，统一进入 `~/.fin/harness/runs/<run-id>/...`。


### 68) Default user provider is frozen to ali-coding-plan / qwen3.6-plus and verified live
- 正常 `~/.fin/config/user.toml` 与测试 `user.toml` 默认首选 provider 统一为 `ali-coding-plan`，默认模型为 `qwen3.6-plus`。
- 凭证优先通过 `api_key_env = "ALI_CODINGPLAN_KEY"` 引用，不把真实 key 写入 `user.toml`。
- provider 连通性必须先通过真实 anthropic 协议探测（`POST {base_url}/v1/messages` + `x-api-key` + `anthropic-version`），确认可用后再继续后续开发。

## 2026-04-17 implementation note: provider real closure diagnosis

### M1 real inference closure current finding
- `fin` 的真实 anthropic-wire provider 已经完成 builder -> provider -> event -> projection 的主链实现。
- workspace unit / contract / runtime tests 已全部通过。
- 真机 provider 闭环在第一次运行时失败，错误为 HTTP 405：`Coding Plan is currently only available for Coding Agents`。

### Verified root cause
- 同一个 endpoint、同一个 model、同一个 payload：
  - Python `urllib` -> 200
  - `curl` 默认 UA -> 200
  - `curl` 清空 `User-Agent` -> 405
  - Rust `reqwest` 默认请求 -> 405
  - Rust `reqwest` 显式加 `User-Agent: fin-coding-agent/0.1` -> 200
- 结论：阿里 Coding Plan anthropic endpoint 会把“无 User-Agent 请求”判为非 Coding Agent 请求。

### Implementation decision
- 在 `fin-provider` 的 anthropic real execution 路径中显式发送 `User-Agent: fin-coding-agent/0.1`。
- 该修复属于 transport / provider owning layer，不进入 runtime / projection / web 层补逻辑。

### Additional maintenance
- 为满足非白名单文件 `<500` 行门禁，`fin-runtime` 测试已从 `src/lib.rs` 拆到 `src/tests.rs`。
