# fin architecture note

Updated: 2026-04-18

## 2026-04-18 compact rebuild implementation snapshot

- `/compact` 不再走 `session select/rebind` 占位逻辑。
- 当前已升级为真正的 framework rebuild pipeline：
  - 输入：`recent_messages + recent_digests + recent_reasoning_views + recent_tool_records + recent_turn_ids + latest note`
  - 装配：`ContextRebuildService -> ContextViewBuilder`
  - 输出：
    - `runtime/current/current_context.json`
    - `runtime/current/current_rebuild_index.json`
    - `sessions/.../context/recent_contexts.json`
    - `sessions/.../context/rebuild-index.json`
    - `sessions/.../events/stream.jsonl` 追加 `context.rebuild_completed`
- rebuild 不调用 provider，不生成新 closure，不改写 `messages.json` / `recent_digests.json`。
- Web debug 已接上新 endpoint：
  - `POST /api/session/rebuild`
  - Web slash command `/compact` 现在调用这个 endpoint，而不是 `/api/session/select`

## 2026-04-18 web debug UI scaffold refinement

- 用户新口径已经收敛为三条独立平面：
  1. `Project`：project context
  2. `Team`：team status plane（不是 project metadata）
  3. `Progress Update`：project progress update framework
- 当前最小实现策略：
  - 顶部先落 `Project / Team` 两个 strip，`Team` 允许先占位
  - 左侧底部先落 `Progress Update` bar，当前从 `taskDigest` 取数
  - 后续再把真源升级为 `update_plan record -> runtime/session artifact -> UI + system agent fanout`
- 推理动画只是 progress/update 的可视化，不是第二事实源。
- 右侧 debug window 的布局规则继续收敛为：
  - category tabs 切类别
  - 当前类别内容占满整个宽度
  - 详情只做 inline vertical expand，不做 fullscreen modal
  - detail / summary 默认单列优先，避免右侧两列堆积

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

## 2026-04-18 webui + command/tool alignment snapshot

### A) WebUI immediate fixes applied
- provider call 不再默认展开为整张工具卡片，改为消息内的最小 bullet button，点击才展开 detail。
- assistant 等待动画已放慢，避免过快闪烁。
- session sidebar 已进入最小可用态：`list/select/new` 已接上 Rust 后端。

### B) Codex slash command truth source
- 参考真源：
  - `~/code/codex/codex-rs/tui/src/slash_command.rs`
  - `~/code/codex/codex-rs/tui/src/bottom_pane/slash_commands.rs`
- 当前最相关的 built-ins（和 fin 最小闭环强相关）：
  - `/new`
  - `/resume`
  - `/compact`
  - `/status`
  - `/clear`
  - `/diff`
  - `/review`
  - `/plan`
- codex 还区分：
  - 是否允许 inline args
  - 是否允许在 task 执行中调用
  - feature flag / sandbox gating

### C) Finger tool / command truth source
- 参考真源：
  - `~/code/finger/src/agents/chat-codex/agent-role-config.ts`
  - `~/code/finger/src/server/routes/message-super-command.ts`
  - `~/code/finger/src/agents/finger-system-agent/capability.md`
- finger 的核心工具族可归纳为：
  1. execution tools：`exec_command` / `write_stdin` / `patch` / `view_image`
  2. coordination tools：`agent.*` / `orchestrator.*` / `user.ask`
  3. mailbox tools：`mailbox.*`
  4. memory/context tools：`context_ledger.memory` / `context_history.rebuild`
  5. session/project tools：`session.list` / `project.task.*`
  6. clock / control tools：`clock` / `reasoning.stop`

### D) 当前 fin 的落差
- fin 目前已有：
  - framework tool context 描述
  - session list/select/new 基础框架
  - web debug / session 真源渲染
- fin 目前缺少：
  - 用户可调用的 slash command router
  - 真正可执行的工具注册表 / tool dispatch
  - `/new` `/resume` `/compact` 等会话级控制命令
  - knowledge digest 独立于 session 生命周期的保留 / 提升机制

### E) 当前建议的实现顺序
1. 先做 slash command router（最小先上 `/new` `/resume` `/status` `/compact`）。
2. `/compact` 不走大模型压缩，直接走 `context rebuild`。
3. 再引入最小 tool registry，把现有 framework-owned abilities 正式注册为 tool specs。
4. session delete 之前必须先有 `knowledge digest promote`，避免删除 session 时丢失已验证结论。

### F) Knowledge digest / wiki-link / graph draft
- `session digest`：会话内 closure 级摘要，跟 session 走。
- `knowledge digest`：经过验证/提升后的知识节点，独立于 session 生命周期。
- 删除 session 时：
  - 原始 session 可删（需授权）
  - 已 promote 的 knowledge digest 保留
- 最小图模型建议：
  - node: `knowledge/{id}.json`
  - edge: `links/{source}->{target}.json`
  - refs: 来源 session/task/closure/artifact
- 原则：
  - 唯一真源知识只在 knowledge store 提升后复用
  - 错误认知不覆盖旧记录，而是追加 `supersedes` / `invalidates` 边

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


## 2026-04-17 implementation note: provider custom user-agent and headers

### Feature
- `user.toml` 的 provider 现在支持两个额外字段：
  - `user_agent = "..."`
  - `[providers.<name>.headers]`
- user config -> system config 仍然是单向映射，不做 merge。

### OpenCode reference used
- 本机 OpenCode CLI 版本：`1.2.27`
- 静态字符串证据显示 OpenCode 使用 `User-Agent: opencode/${Installation.VERSION}` 风格。
- 因此默认 normal/test `user.toml` 生成时写入 `user_agent = "opencode/1.2.27"`。

### Runtime rule
- provider transport 允许附加自定义 headers。
- 但 `x-api-key` / `anthropic-version` / `content-type` / `accept` / `user-agent` 这类运行必需头由框架最终兜底写回，避免用户 header 配置破坏真实链路。


## 2026-04-17 M1 observability slice accepted and verified

### Provider debug visibility
- provider 事件现在携带 `debug` 字段：
  - `user_agent`
  - `request_headers`（sanitized / redacted）
- projection 现在暴露：
  - `latest_provider_user_agent`
  - `latest_provider_header_names`
- Web debug 读取 projection + `current_context.json`，可以直接观察：
  - 当前 provider 活动
  - 当前 UA
  - 当前 header names
  - 当前 context 结构

### Snapshot bounded-write rule
- context snapshot 不做无界散写。
- 当前冻结实现：
  - `~/.fin/runtime/current/current_context.json`：只保留最新一份，覆盖写
  - `~/.fin/sessions/YYYY/MM/<session-id>/context/recent_contexts.json`：bounded recent window（当前 8 条）
- 不做每轮/每事件一个 context 文件的无限增长策略。

### Lifecycle / resource rule
- M1 `web-debug` 仍是前台阻塞型命令，不启动 detached daemon。
- 本轮没有引入后台子进程，因此不会制造孤儿进程。
- Web 前端轮询做了最小节流：
  - 单次刷新互斥
  - 页面 hidden 时不主动刷
  - 3s 轮询

### Verified evidence
- `cargo test --workspace`：通过
- 真实 provider 隔离回归：通过
  - `FIN_RUNTIME_HOME_OVERRIDE=/tmp/fin-runtime-verify.*`
  - `FIN_SESSION_NAMESPACE=test-context-bounded`
  - `cargo run -p fin-cli -- runtime-demo ~/.fin/config/user.toml '请只回复 OK'`
  - 返回 `OK`
- 隔离 runtime home 已验证生成：
  - `runtime/current/current_context.json`
  - `sessions/2026/04/session-test-context-bounded/context/recent_contexts.json`
  - `runtime/projections/current_projection.json`

### 69) fin-cli build/versioning slice was modularized and main.rs left whitelist
- `fin-cli` 的 build/versioning/install/smoke 逻辑已从单一 `main.rs` 拆成 `cli / install_flow / install_smoke / runtime_home / versioning / demo / config / process_utils / fs_utils`，保持原命令行为不变。
- `main.rs` 现仅保留二进制入口，已从 `scripts/line-limit-whitelist.txt` 移除；当前 whitelist 只剩 `rust/crates/config/src/lib.rs`。
- 这次拆分的验证证据为：`python3 scripts/check-code-line-limit.py`、`cargo test -p fin-cli`、`cargo test --workspace` 全通过。

### 70) transcript-demo now forms a real multi-turn provider closure
- `transcript-demo` 已落地为最小多轮闭环：同一 `session/task` 下按 turn 顺序重建 `MinimalContextView`，并把 recent digest continuity/summary 真正编入 provider request。
- Web debug 新增 `Recent Context History`，通过 `/api/recent_contexts.json` 读取 `last_run.json -> session_recent_contexts_path`，可以直观看每轮请求时的 context 结构。
- 真实 provider 隔离验证已通过：三轮 transcript 中第三轮成功仅回复 `BANANA-42`，证明 context 不只是记录，而是已经真正进入模型请求。


## 2026-04-18 multi-turn history architecture conclusion

### Reference takeaways
- Finger:
  - session is the durable execution substrate
  - raw ledger and compact memory must be split
  - different agent/worker runtimes can keep separate ledgers
  - shared value should be retrieved from memory/ledger-derived artifacts rather than injected from skills
- Codex:
  - persistent history and local rich state must be separated
  - cross-session history should stay lightweight
  - current-session history can retain richer working payloads
- Hermes-agent:
  - context compression must be structured and iterative
  - compression should preserve the tail
  - tool-call/tool-result pairs must remain intact across compression

### fin accepted full multi-turn history model
fin should freeze the multi-turn history stack into six layers:

1. Operation / Event Ledger Layer
   - append-only runtime fact truth
   - records operation accepted, provider/tool progress, control feedback, dispatch, mailbox, heartbeat, failures
   - never treated as direct prompt history

2. Turn / Closure Record Layer
   - one complete closure = one durable turn unit
   - interrupted execution does not create an independent closure
   - each turn record should bind user input, assistant visible output, control feedback, progress, reasoning/tool/provider refs, context snapshot ref, digest ref

3. Progress / Execution Note Layer
   - ProgressBlock: high-frequency phase, tool snapshots, blocker, health, next step
   - ExecutionNote: continuous notes promoted from control blocks, plan changes, lessons, handoff facts
   - execution note is part of digest input, but is generated continuously rather than only at task end

4. Digest / Compact Memory Layer
   - closure digest generated at every valid closure
   - task digest and topic digest updated iteratively
   - used for rebuild and continuity, not as raw truth replacement

5. Knowledge Artifact Layer
   - promoted only from verified, reusable, durable conclusions
   - shared via retrieval scope instead of direct session-history sharing
   - suitable for cross-worker, cross-session, workdir/repo-level reuse

6. Working Context Layer
   - framework-built dynamic view for one inference
   - context is assembled from prompt blocks, routing/control state, project/collab state, retrieved knowledge, recent continuity tail, current input
   - session is not context; context is an ephemeral view over session state and retrieved materials

### Session vs context
- Session = durable substrate for execution, rebuild, replay, debug, and collaboration
- Context = one inference-time assembled view
- ContextView must prioritize the latest runjournal/recent continuity tail as the most important reasoning continuation material

### Recommended context assembly order
1. Stable core prompt / role prompt / skills / output contract
2. Control + routing blocks (session/task/topic/task-list/topic-list/current routing hints)
3. Project and collab blocks (project root, active projects, cwd, selected paths, collab deltas)
4. Retrieved knowledge artifacts + task/topic digests
5. Recent continuity tail:
   - recent closure digests
   - recent execution notes
   - recent reasoning summaries
   - recent tool activities
   - recent visible messages / turn tail
6. Current input

### Compression and rebuild rules
- Never compress raw event ledger
- Compress turn/digest/rebuild layers only
- Always preserve:
  - latest N closures
  - unfinished tool/result pairs
  - latest control state
  - active task/topic binding
  - recent runjournal + execution note tail
- Rebuild should be triggered by:
  - token pressure
  - task/topic switch
  - session revive
  - worker handoff
  - long-gap resume
- Rebuild should combine:
  - relevant task/topic digests
  - retrieved knowledge artifacts
  - recent continuity tail
  - latest runjournal / note tail

### Multi-worker sharing rule under same workdir
- workers must not directly share raw session history
- each worker keeps its own runjournal / worker ledger / worker-local notes
- sharing happens through:
  - collab deltas for current collaboration
  - approved digests
  - promoted knowledge artifacts
  - retrieval scopes
- recommended retrieval scopes:
  - worker_local
  - session_local
  - task_shared
  - workdir_shared
  - repo_shared
  - global

### Directory/model implications for fin
The future session layout should evolve toward:
- ledger/: operations/events/step-ledger/worker-ledgers
- conversation/: messages + turn records
- progress/ + notes/ + control/
- context/: recent contexts + rebuild index
- digests/: closure/task/topic digest families
- reasoning/ + tools/ + closures/
- collab/ + tasks/ + topics/
- artifacts/: candidate/knowledge/verified

### Implementation priority after architecture freeze
1. Add a canonical TurnRecord / ClosureRecord layer
2. Add StepLedger inside each turn
3. Split digest family into closure/task/topic
4. Add retrieval scope + rebuild index
5. Only then upgrade runtime to true multi-step inference loop

### Final architectural sentence
fin should adopt the following canonical model:
- raw truth lives in ledger
- closed-loop conversation units live in turn records
- ongoing process state lives in progress/execution notes
- continuity and rebuild live in digest families
- cross-worker sharing lives in knowledge artifacts plus retrieval scope
- model input always comes from a framework-built ContextView rather than directly from raw history

## 2026-04-18 WebUI multi-turn history truth wiring
- New debug truth exposed to WebUI:
  - `recent_turns` -> canonical turn anchor
  - `recent_steps` -> per-turn step timeline
  - `task_digest` / `session_digest` / `rebuild_index` -> digest family + rebuild truth
- Backend additions:
  - added `/api/recent_steps.json`
  - `DebugBinding` now includes `recent_steps_path`
- Frontend wiring rules:
  - `FocusTurn` now anchors on `TurnRecord.operation_id`
  - message/context/reasoning/tool/closure/events merge into the turn anchored by `TurnRecord`
  - `StepRecord[]` attaches to the same focus turn and is rendered in inspector as `Step Timeline`
- Inspector layout update:
  - `System` card now shows task digest / session digest / rebuild index summary
  - `Operation` card now shows turn record + step timeline before lower-level request/debug details
- Validation evidence:
  - `cargo test -p fin-debug-server -p fin-cli` passed after TS + Rust wiring
- Remaining next step:
  - review live WebUI rendering against real runtime artifacts, then continue inference-core completion and context assembly hardening


## 2026-04-18 multi-step inference core snapshot

- `M1Runtime::run_closure` 已从单轮 provider closure 升级为多步 loop：
  - `context -> provider -> parse -> tool dispatch -> context update -> provider -> ... -> final answer / failed closure`
- 模型输出 contract 新增 `fin_tool_calls`：
  - 第一块现在允许二选一：`<fin_user_response>` 或 `<fin_tool_calls>`
  - 第二块仍固定为 `<fin_control_feedback>`
- runtime 当前已真正可执行的 model-selected tools：
  - `update_plan`
  - `session.list`
  - `context_history.rebuild`
- runtime 对未实现工具不再静默忽略：
  - 会产出 `tool.dispatch_failed`
  - closure 以 `status=failed` 收口
  - 追加 `operation.failed` 事件、失败 progress、failure summary
- provider 失败也不再直接中断为无 artifacts 的裸错误：
  - 当前 closure 会合成失败结果并正常落 note/digest/closure/event 链，便于 session truth / Web 继续观察
- event 链已升级为 step-aware：
  - 每轮 provider 事件保留 step 语义
  - tool dispatch 增加 `tool.dispatch_started/completed/failed`
- final closure 现在带：
  - `status`
  - `failure_summary`
  - 多个 `provider.call` records（多步时每次 provider round-trip 都单独记录）
  - model-selected tool records
- context 在 closure 内会持续更新：
  - interim reasoning -> `history.recent_reasoning`
  - tool result -> `history.recent_tool_activity`
  - tool output summary -> `history.recent_messages`
  - `context_history.rebuild` 会直接替换后续 provider 使用的 context
- tool registry 已从 staged-only 向最小可执行推进：
  - `update_plan / session.list / context_history.rebuild` 已从 disabled 列表移出
- 新增回归：
  - parser 能解析 `fin_tool_calls`
  - provider 第 1 轮请求工具、第 2 轮给最终答案的多步 loop 测试
  - 请求未实现工具时的 failed closure 测试

## 2026-04-18 peer taxonomy and supervision snapshot

- `fin` 后续不应把远端对象只理解成 agent，而应统一理解成 `peer`
- peer 至少分为三类：
  - `capability peer`
  - `agent peer`
  - `channel gateway`
- `project agent` 属于 `agent peer`
- `system agent` 是唯一用户入口与总编排者，不属于“被路由执行的 peer”
- `daemon` 是本机生命周期与资源管理真源，负责：
  - spawn / restart / reap / drain local peers
  - orphan cleanup
  - local peer registry
  - health / crash / quarantine
- 后续架构要冻结为两层平面：
  - `peer plane`: discover / auth / lease / heartbeat / health / capability advertisement
  - `execution plane`: job / task / message
- presence 与 binding 必须拆开：
  - `presence = existence + liveness`
  - `binding = ownership + assignment`
- 冻结规则：
  - presence 对称
  - binding 非对称（只有 system agent 能做 task/session binding）
- `fin start --slave` 的语义冻结为：
  - 启动远端 `project agent` service mode
  - idle/listening
  - 等待 system agent discover/connect/auth/lease/bind
- 本机 unattached project agent 的正确启动路径应为：
  - `system agent -> daemon.ensure_peer(...) -> daemon spawn/reuse -> system agent connect + bind`
- 用户会话主真源始终属于 `system agent`
- `project agent / capability peer / channel gateway` 只拥有各自的执行账本、能力结果或 channel 适配态，不直接接管用户主会话

## 2026-04-18 local reasoning gap audit under peer model

在新冻结的 peer 模型下，当前本地 runtime 推理部分还缺以下几类东西：

### A. Role / Prompt 缺口
- 当前 role family 只有：
  - `system`
  - `worker`
  - `reviewer/analyzer`
  - 默认 `project`
- 还缺明确的：
  - `project_agent`
  - `capability_router` 或 `peer_router`
  - `channel_gateway`（即使不直接推理，也应有 prompt/contract 位）
- 当前 `system` prompt 仍偏“单机总控”，还没有显式声明：
  - peer discovery
  - presence/binding ownership
  - daemon 协作
  - capability vs agent 路由优先级

### B. Context 结构缺口
- 当前 context blocks 主要是：
  - `control`
  - `role_prompt`
  - `tools`
  - `history`
  - `knowledge`
  - `project`
  - `current_input`
- 缺少新的 peer 相关 block：
  - `peer_topology`
  - `peer_presence`
  - `binding_state`
  - `capability_catalog`
  - `daemon_state`
  - `channel_routes`
- 当前 `project` block 仍是 repo/workdir 视角，不足以支撑 `system agent` 的多 peer 编排

### C. Tool / Dispatch 缺口
- 当前可执行 model tools 只有：
  - `update_plan`
  - `session.list`
  - `context_history.rebuild`
- 下一层除了 `exec_command / write_stdin` 以外，还需要预留 peer 相关工具抽象：
  - `peer.list`
  - `peer.describe`
  - `capability.invoke`
  - `agent.assign`
  - `binding.open`
  - `binding.close`
  - `daemon.ensure_peer`
  - `peer.status_probe`
- 当前 tool dispatch 仍默认都是本地 runtime 内工具，不支持把“执行动作”路由到 capability peer / agent peer

### D. Control Block 缺口
- 当前 `ControlFeedback` 只有连续性/话题/simple query 相关字段
- 在 peer 模型下，后续需要增加的控制判断包括：
  - 是否需要 peer 路由
  - 更适合 capability peer 还是 agent peer
  - 是否需要 daemon ensure/spawn
  - 是否需要 bind / rebind
  - 对目标 peer/task route 的置信度
- 这些不一定要直接塞进现有 `ControlFeedback`，但至少需要一个并行的 control/routing block

### E. Event / Runtime Fact 缺口
- 当前 runtime 事件主要还是：
  - provider.*
  - tool.dispatch.*
  - progress/note/digest/closure
- 还缺最小 peer 事件族：
  - `peer.discovered`
  - `peer.connected`
  - `peer.authenticated`
  - `lease.opened`
  - `heartbeat.missed`
  - `binding.opened`
  - `binding.closed`
  - `daemon.peer_spawned`
  - `daemon.peer_reaped`
- 没有这些事件，就很难把 system/project/daemon 协作纳入统一 debug 真源

### F. 执行闭环缺口
- 当前 inference loop 已支持：
  - provider -> tool -> provider 的本地多步闭环
- 但还不支持：
  - capability peer 异步 job
  - agent peer task binding
  - remote peer progress merge
  - peer failure / reconnect / rebind
- 所以当前闭环仍然是“单 runtime 本地闭环”，还不是“多 peer 协作闭环”

### 当前建议的优先顺序
1. 先把 `exec_command / write_stdin` 接入，完成本地通用工具闭环
2. 然后补 `system_agent / project_agent` role prompt 分层
3. 再补 `peer_topology / presence / binding / capability_catalog` context blocks
4. 再引入最小 peer tools 与 `peer.* / binding.* / daemon.*` 事件族
5. 最后再进入真正的 local/remote peer 执行接入

## 2026-04-18 peer-aware local reasoning skeleton landed

已将“peer 设计如何先进入本地推理骨架”冻结到：

- `docs/architecture/32-peer-aware-local-reasoning-skeleton.md`

本轮已实际接入的代码骨架：

1. `MinimalContextView` 新增 `peer` block
2. `ContextViewBuilder` 会在没有 peer registry 时生成受控的 `local-only M1 mode` placeholder
3. `ModelInputAssembler` 新增 `Peer topology` 段，并把 `history` 下移到 `project/peer` 后面
4. `prompt_assembly` 已补：
   - `system_agent`
   - `project_agent`
   - `capability_router / peer_router`
   - `channel_gateway`
   的 role baseline 差异

当前刻意未做的事情：

- 不伪造 remote peer registry
- 不伪造真实 lease/binding 事实
- 不把 placeholder 当作真实运行事实
- 不提前接入 peer tools / peer events / daemon IPC

这意味着：

- 当前仍是单 runtime 本地闭环
- 但 prompt/context 结构已经不再是 project-only 视角
- 后续接 peer plane 时，不需要再次推翻上下文 schema

## 2026-04-18 peer routing control skeleton landed

本轮继续完成：

1. 新增 `PeerRoutingFeedback`
2. runtime finalize 阶段会生成 routing artifact，并写入：
   - `runtime/current/current_peer_routing_feedback.json`
   - `sessions/.../routing/latest.json`
   - `ExecutionNote.peer_routing_feedback`
   - `DigestRecord.peer_routing_feedback`
3. event 链新增：
   - `peer.routing_feedback_recorded`
4. 若上下文本身已有 peer block，则 runtime 会发 observation skeleton：
   - `peer.discovered`
   - `binding.opened`
   - `daemon.state_observed`
5. projection 已可消费：
   - `latest_route_target_kind`
   - `latest_route_target_peer_id`
   - `latest_route_confidence`
   - `latest_route_origin`

本轮新增真源文档：

- `docs/architecture/33-peer-routing-control-and-observation-events.md`
- `docs/contracts/peer-routing-feedback-contract.md`

当前刻意保持的边界：

- 没有 remote peer 真执行
- 没有 auth/connect/lease/reconnect
- 没有 daemon.ensure_peer IPC
- 没有 peer.list / capability.invoke / agent.assign 真动作

所以当前 routing 仍是 framework-owned heuristic truth，不是 peer plane 完整实现。

## 2026-04-18 peer tools skeleton cleanup + validation

本轮继续“peer-aware local reasoning skeleton”收口，完成了最小可验证闭环：

1. contracts/context:
   - `MinimalContextView.peer` 已稳定接入（`PeerContextBlock` family）
2. runtime assembly:
   - `ContextViewBuilder` 挂接 `peer` block（local-only placeholder）
   - `ModelInputAssembler` 渲染 `Peer scope`
3. tool catalog:
   - 新增独立模块 `runtime/src/tool_catalog.rs`
   - peer tools skeleton: `peer.list`, `peer.describe`, `daemon.ensure_peer`
   - 当前全部处于 contract-frozen placeholder（在 `disabled_tools` 中显式标注）
4. 模块拆分收口：
   - 清理 `context_blocks.rs` 残留 tool catalog helper，避免重复语义
   - `context_view.rs` 改为从 `tool_catalog` 模块装配工具目录
   - `runtime/lib.rs` 注册 `mod tool_catalog`
5. 验证：
   - `cargo fmt --all --manifest-path rust/Cargo.toml`
   - `cargo test -p fin-contracts -p fin-runtime --manifest-path rust/Cargo.toml`
   - 结果：通过

边界声明：
- 本轮只完成 schema/context/prompt 可见性与最小工具目录骨架；
- 未接入真实 peer dispatch / handshake / daemon IPC 执行链。

## 2026-04-18 model tools completion + async wait reminder (system self wakeup)

本轮补齐了缺失工具的最小可执行闭环（runtime 真源）：

1. 模型工具调用协议
   - `ModelOutputParser` 新增 `<fin_tool_calls>...</fin_tool_calls>` 解析
   - 支持 `[{"tool_name":"...","arguments":{...}}]` 或单对象
   - 解析结果进入 runtime tool dispatcher

2. tool dispatcher（已可执行）
   - `peer.list`（读取当前 context.peer 快照）
   - `peer.describe`（按 peer_id 描述）
   - `daemon.ensure_peer`（placeholder intent + event）
   - `wait.remind`（异步等待调度）
   - 未注册工具显式 failed record（不静默）

3. wait.remind 异步提醒闭环
   - 参数固定两项：`wait_minutes` + `reminder`
   - 调度事件：`system.reminder_scheduled`
   - SessionMaterializer 会把调度持久化到 `~/.fin/runtime/reminders/pending.json`
   - Web debug 每次收消息前执行 due-check，超时提醒注入 session `system` 消息，推动下一次推理
   - wake role 固定为 `system`（system self wakeup）

4. Prompt / Tool 提示词规则
   - 新增规则：若预计等待超过 1 分钟，优先 `wait.remind`，不要 busy waiting
   - 输出契约支持可选第三块 `<fin_tool_calls>`（前两块仍为强制）

5. 验证
   - `cargo fmt --all --manifest-path rust/Cargo.toml`
   - `cargo test -p fin-runtime -p fin-contracts -p fin-cli --manifest-path rust/Cargo.toml`
   - 全部通过

## 2026-04-18 reasoning.stop migration (from finger semantics)

已按你的要求把“停止判定”切到 `reasoning.stop`：

1. 新增模型工具：`reasoning.stop`
2. runtime 闭环停止信号改为工具调用：
   - `operation.completed.status=stopped` 仅在收到 `reasoning.stop` 时成立
   - 若未收到 `reasoning.stop`，状态为 `continued`
3. 明确不再把 provider `finish_reason=stop/end_turn` 作为闭环停止依据
4. prompt/tool policy 已更新：
   - 结束当前推理 turn 时必须调用 `reasoning.stop`
   - 超过 1 分钟等待优先用 `wait.remind`

验证已通过：
- 新增单测 `runtime_closure_uses_reasoning_stop_as_stop_signal`
- 全量命令：`cargo test -p fin-runtime -p fin-cli -p fin-contracts --manifest-path rust/Cargo.toml`

## 2026-04-18 tool loop + slash commands + qqbot builtin gateway peer bootstrap

本轮新增了三块关键能力：

1) 多轮自动 tool loop（同一 turn 内）
- runtime 现在支持最小自动 roundtrip：
  - 第一轮若产出 `fin_tool_calls` 且未 `reasoning.stop`
  - 框架会自动基于“原始问题 + 上轮回答 + 最新工具结果”发起第二轮 provider 推理
- 新增事件：`reasoning.auto_tool_roundtrip_completed`
- 停止语义仍保持：只有 `reasoning.stop` 才算 `status=stopped`；否则 `continued`

2) slash command router（web chat path）
- 已接入本地命令：
  - `/new`：创建并绑定新 session/task
  - `/resume <session_id>`：恢复已有会话绑定
  - `/compact`：不调用 provider，直接 rebuild context 并写
    - `runtime/current/current_context.json`
    - `runtime/current/current_rebuild_index.json`
    - `sessions/.../context/recent_contexts.json`
    - `sessions/.../context/rebuild-index.json`
- 命令会写入 session conversation 的 `local_command + system notice`

3) QQBot 内置 gateway peer 启动骨架
- `web-debug` 启动时自动执行 `ensure_builtin_qqbot_peer`
- 新增运行时状态文件：
  - `runtime/peers/qqbot/state.json`
  - `runtime/peers/registry.json`
- 先冻结为 lifecycle bootstrap：`idle_unpaired + pairing_required=true`

附带：
- tool catalog 扩展了下一步将接线的工具族（exec_command / write_stdin / mailbox / agent.assign / capability.invoke）
  目前先完成 contract/prompt 可见性，逐步接 dispatcher 真执行。

验证：
- `cargo test -p fin-cli -p fin-runtime -p fin-debug-server --manifest-path rust/Cargo.toml`
- `cargo test -p fin-contracts --manifest-path rust/Cargo.toml`
- 结果：通过

## 2026-04-18 继续推进（tool dispatcher 收口）

- 修复 runtime 编译断点：补齐 `tool_dispatch_extended` 及相关模块声明，恢复 `fin-runtime` 构建。
- 将大文件拆分为可维护模块（全部 <500 行）：
  - `tool_dispatch_extended_exec.rs`
  - `tool_dispatch_extended_collab_mailbox.rs`
  - `tool_dispatch_extended_collab_coordination.rs`
  - `tool_dispatch_extended*.rs` 作为薄编排层。
- 新增可执行模型工具处理：
  - `exec_command`
  - `write_stdin`（基于 exec replay session）
  - `mailbox.send`
  - `mailbox.poll`
  - `agent.assign`
  - `capability.invoke`
- 补充 runtime 单测：
  - `exec_command + write_stdin` replay 闭环
  - `mailbox.send + mailbox.poll(consume)` 闭环
- 完整回归：`fin-runtime + fin-cli + fin-debug-server` 全部通过。
- 推理自动工具循环从“固定一轮 follow-up”升级为“多轮 loop + 最大轮次保护（6）”：满足同一 turn 内连续工具调用，且避免无限循环；触发保护时写 `reasoning.auto_tool_roundtrip_limit_reached`。
- QQBot 内置 gateway peer 生命周期第二阶段已落地（CLI 框架层）：
  - `channel_peer` 新增 session/pairing 生命周期状态字段（pairing_required/session_valid/session_id/session_expires_at/reconnect_count/heartbeat）。
  - 新增结构化 peer 事件日志 `~/.fin/runtime/peers/qqbot/events.jsonl`，事件包含 `event_id/sequence/timestamp/sender/source/protocol_version/payload`。
  - 事件类型落地：`channel.peer.pairing_required`、`channel.peer.pairing_completed`、`channel.peer.session_expired`、`channel.peer.heartbeat_recorded`。
  - `ensure_builtin_qqbot_peer` 现在具备 TTL 到期检测：到期后自动置 `pairing_required=true` 并产出 `session_expired + pairing_required` 事件，实现“session失效需重配”的框架闭环。
  - `web_debug` 每次接收消息前会调用 `ensure_builtin_qqbot_peer` 做生命周期同步检查。
- QQBot 配对入口已接入本地 slash command 路由：支持 `/qqbot status|pair|heartbeat|expire`。其中 `/qqbot pair` 默认绑定当前 active session，并将 local_command + system notice 写入当前 session conversation，保证 channel 渲染仍以 session 文件为真源。
- debug-server 已新增 qqbot peer 观测入口：`/api/qqbot_state.json`、`/api/qqbot_events.jsonl`，便于后续 Web debug 面板直接消费 peer lifecycle 事实。
- 本轮顺手抽出了 `local_command_notice.rs`，把本地命令写 session conversation 的逻辑下沉复用；同时把 `channel_peer.rs`、`session_commands.rs` 拉回 500 行内，line-limit 当前只剩历史超限文件。
- QQBot gateway 与 active session 的绑定失配现在会被框架主动失效化：当 peer 已 paired 到旧 session，而当前会话切到新 session（包括 `/new`、`/resume`、后续正常消息入口），框架会产出 `channel.peer.session_invalidated` + `channel.peer.pairing_required`，并把 peer 状态恢复到 `idle_unpaired`，避免旧绑定继续伪装为有效。

## 2026-04-18 finger 配对码机制核查结论

本轮只做证据核查，不改 fin 流程。结论：

1. **finger 仓库里没有现成的“配对码 / pairing code”握手实现可直接复用**
   - 全仓 grep 未发现稳定的 `pairing code / pair code / link code / device code / 验证码 / 配对码 / qr code` 机制落地。
   - 命中内容主要是 `gateway process session`、`thread binding`、`session binding`、`mailbox`、`qqbot gateway bridge`。

2. **finger 的 qqbot 接入是“凭证启动 + thread/session binding”，不是“用户配对码绑定”**
   - `src/cli/openclaw-gateway-bridge.ts`
     - `connect <channel-id>` 要求 `appId + clientSecret`
     - `sendStartAction(..., appId, clientSecret, ...)`
     - `handleStart(payload)` 明确校验 `Missing appId or clientSecret`
   - `tests/e2e/gateway-bridge-qqbot.test.ts`
     - 测试也是直接发 `action:start` + `payload:{ appId, clientSecret }`
   - `docs/reference/templates/system-agent/OPENCLAW-INTEGRATION.md`
     - SOP 也是安装插件 + 写 `~/.finger/config/channels.json` / runtime plugin config + 重启 daemon

3. **finger 有可借鉴的不是配对码，而是“绑定语义”**
   - `src/inputs/openclaw.ts`
     - 入站消息会提取 `senderId / threadId / messageId`
   - `memory/2026-03-10-openclaw-mailbox-design-decision.md`
     - finger 自己做 `thread binding`、权限策略、mailbox 回流
   - `memory/2026-03-12-qqbot-channel-architecture.md`
     - 核心是消息进入统一 MessageHub / session route，而不是做配对码认证

4. **对 fin 的直接含义**
   - 不能说“复用 finger 现成配对码连接”，因为 finger 当前没有这个机制。
   - 可以复用 / 借鉴的是：
     - channel 接入后的 `thread/session binding`
     - gateway 生命周期
     - ready / error / stopped 事件模型
   - 如果 fin 要“首次配对、session 失效后重配”，需要做 **fin-native pairing code flow**，而不是照搬 finger 代码。

### 同日更正：pairing 需要拆成两个正交层面

用户补充后，结论修正为：

1. **channel ↔ upstream service 配对 / 鉴权**
   - 这是渠道自身接入层。
   - 例如 qqbot 通过 `appId + secret` 与上游服务建立认证和连接。
   - 有些 channel 可能需要服务器鉴权，有些不需要；这是 channel-specific 生命周期。

2. **peer ↔ agent 配对 / 绑定**
   - 这是 fin 框架内部的协作绑定层。
   - 目标是把某个 channel peer / remote peer 绑定到 system agent / project agent / session route。
   - 本地场景可以有默认 pairing code（如 `fin:welcome`）；远程场景则可走双方显式配置（如 `service + account:password`）后的握手。

3. **设计规则**
   - 这两个层面不能混为一谈：
     - upstream auth 成功 ≠ peer 已绑定 fin agent
     - peer 已绑定 fin agent ≠ upstream 仍然有效
   - 状态机、事件、debug 面板需要分别显示：
     - `channel auth / connection state`
     - `peer-agent binding state`

### 同日补充：bootstrap 顺序应先 peer 可达，再做上游鉴权

用户确认后的统一顺序：

1. **peer 先自己能起来**
   - 本地先把 peer 进程/服务启动成功。
   - 此时只代表 peer 在本机/本网络可达，不代表它已经能访问上游服务。

2. **本地访问权限先成立**
   - system agent / 本机控制面需要先具备访问 peer 的权限与管理权。
   - 这是本地 control plane 与 peer 的管理关系，不等于 peer 已取得服务器权限。

3. **peer 再通过配对 / 鉴权访问服务器**
   - peer 与上游 server/service 的 pairing/auth 成功后，才进入真正 connected / active。
   - 这层属于 upstream connectivity，不与本地 agent binding 混淆。

4. **channel 也应采用同类分层**
   - channel process / gateway 自己先可启动、可本地管理
   - 再完成 channel-specific upstream auth
   - 再进入 fin 内部的 peer-agent binding / session route binding

5. **统一状态视角**
   - `peer runtime state`：进程/服务是否活着、可达、可管理
   - `upstream auth/connectivity state`：是否已和服务器配对鉴权并连通
   - `peer-agent binding state`：是否已绑定到 fin 的 system/project/session 路由

## 2026-04-18 qqbot peer 三层状态模型已落地（最小实现）

本轮已把当前内置 qqbot peer 的状态真源从“单 lifecycle + pairing/session bool”升级为三条显式状态线：

1. `runtime_state`
   - 当前最小实现：`ready_local`
   - 表示 peer 已在本地可达、可管理

2. `connectivity_state`
   - 当前最小实现：`local_only`
   - 表示当前只是本地 peer bootstrap 完成，还没有实现上游 server auth/connectivity 闭环

3. `binding_state`
   - 当前使用：
     - `pairing_required`
     - `bound`
     - `expired`
     - `invalidated`

兼容策略：
- 旧字段 `lifecycle_state / pairing_required / session_valid` 继续保留，但改为从三条状态线派生。
- `state.json`、`registry.json` 现在都会带三条状态线，旧消费者仍可继续读兼容字段。

本轮还补了：
- 初始 bootstrap 事件：`channel.peer.runtime_ready`
- 原有事件 payload 中补入：
  - `runtime_state`
  - `connectivity_state`
  - `binding_state`
- `/qqbot status` 和 `/qqbot expire` 输出已切到显示三条状态线
- 相关 Rust 单测已通过
