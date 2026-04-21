# fin architecture note

Updated: 2026-04-18

## 2026-04-20 presence registry truth landed

- `agent presence` 不再只有单条 `current_agent_presence.json`。
- framework 现在会同时维护：
  - `~/.fin/runtime/agents/presence_registry.json`
  - `~/.fin/runtime/current/current_agent_presence_registry.json`
- 目的：
  - 让 system/status/debug 能看到所有 agent 的 busy/idle/waiting 并发状态
  - 避免只靠 naming registry 知道“有哪些 agent”，却不知道它们当前在干嘛
- 当前 `status_probe` 的 agent summary 已优先读取 presence registry，格式变为：
  - `agents=2 [mbp.builder:idle, mbp.system:busy]`

## 2026-04-20 system owner-loop query tools landed

- 补齐了 system role 在多轮/异步 owner-loop 里需要的 framework query tools：
  - `agent.presence.list`
  - `project.supervision.list`
- 当前语义：
  - 首轮 provider 请求依然通过 context 直接带 `agent_presence_summary / project_supervision_summary`
  - 但后续 turn 若 system agent 需要主动巡检 busy/idle/waiting、resume/recover intent，不再只靠首轮 summary 和记忆，而是通过 model-callable tool 直接回读：
    - `~/.fin/runtime/current/current_agent_presence_registry.json`
    - `~/.fin/runtime/current/current_project_supervision.json`
- owner-loop 当前最小可执行查询面现在变为：
  - `project.task.list`
  - `project.task.status`
  - `agent.presence.list`
  - `project.supervision.list`
  - `peer.list / peer.describe`
- 已验证：
  - `cargo test -p fin-runtime tool_dispatch_query_tests --manifest-path rust/Cargo.toml -- --nocapture`
  - `cargo test -p fin-runtime prompt_tests --manifest-path rust/Cargo.toml -- --nocapture`
  - `cargo test -p fin-runtime context_view_tests --manifest-path rust/Cargo.toml -- --nocapture`
  - `cargo test -p fin-runtime --manifest-path rust/Cargo.toml --quiet`

## 2026-04-20 task-system write tools landed

- owner-loop 不再只有“看 task/presence/supervision”的 query 面。
- 现在已补齐并接线最小 managed task write tools：
  - `project.task.create`
  - `project.task.claim`
  - `project.task.submit`
  - `project.task.review`
- 当前语义：
  - `create`：把复杂工作正式纳入 session task registry / board truth
  - `claim`：把 task 绑定到当前执行 worker
  - `submit`：worker 提交结果给 review owner
  - `review`：review owner 执行 approve / reopen / block / cancel
- 当前收下的最小 owner-loop 动作面：
  - query：
    - `project.task.list`
    - `project.task.status`
    - `agent.presence.list`
    - `project.supervision.list`
  - write：
    - `project.task.create`
    - `project.task.claim`
    - `project.task.submit`
    - `project.task.review`
  - coordination：
    - `agent.assign`
- 已加的 guard：
  - duplicate task id create -> failed
  - wrong claimer submit -> failed
  - non-review-owner review -> failed
  - terminal task mutate -> failed
- 已验证：
  - `cargo test -p fin-runtime tool_dispatch_task_write_tests --manifest-path rust/Cargo.toml -- --nocapture`
  - `cargo test -p fin-runtime prompt_tests --manifest-path rust/Cargo.toml -- --nocapture`
  - `cargo test -p fin-runtime context_view_tests --manifest-path rust/Cargo.toml -- --nocapture`
  - `cargo test -p fin-runtime --manifest-path rust/Cargo.toml --quiet`

## 2026-04-20 worker pool presence truth landed

- startup 默认 worker budget 现已固定为：
  - `system_agent.local_worker_budget = 4`
  - `project_agent.worker_budget = 4`
- framework 不再只把 worker budget 留在配置字段里；当前会在 startup / presence materialization 时直接落成可观察的 worker pool truth：
  - system worker -> `agent_kind=system_worker`
  - project worker -> `agent_kind=project_worker`
- presence registry 当前会带稳定 `worker_id`，并同步进入：
  - `~/.fin/runtime/agents/presence_registry.json`
  - `~/.fin/runtime/current/current_agent_presence_registry.json`
- `agent.presence.list` 现在也会把 `worker_id` 暴露给模型，后续 `agent.assign` / mailbox / task owner-loop 可以基于同一份 worker truth 做目标选择，而不是让模型盲猜 `target_worker_id`
- 已验证：
  - `cargo test -p fin-cli agent_presence_tests::write_presence_updates_presence_registry --manifest-path rust/Cargo.toml -- --nocapture`
  - `cargo test -p fin-cli startup_topology::tests::materialize_writes_always_on_project_and_wake_queue --manifest-path rust/Cargo.toml -- --nocapture`
  - `cargo test -p fin-cli --manifest-path rust/Cargo.toml --quiet`
  - `cargo test -p fin-runtime --manifest-path rust/Cargo.toml --quiet`
  - `cargo fmt --all --manifest-path rust/Cargo.toml --check`

## 2026-04-20 startup worker defaults moved out of code literals

- `system/project` 默认 worker budget 不再直接硬编码在 `startup.rs` 的 Rust 字面量里。
- 当前默认值真源改为独立配置文件：
  - `rust/crates/config/defaults/runtime-startup.toml`
- 语义：
  - repo 内 baseline default 由配置文件声明
  - 运行时仍优先读取 `~/.fin/config/system.toml`
  - `system.toml` 不存在时，才回落到 embedded startup defaults config
- 这样后续改默认 worker 数，不需要改 Rust 逻辑，只需要改 startup defaults config / runtime system config。

## 2026-04-20 restart/startup updates now have framework-owned startup control summary

- 每次 startup / restart refresh 后，framework 现在会额外落：
  - `~/.fin/runtime/current/current_startup_control_summary.json`
- 它统一回答：
  - 当前 startup 配置预算（system workers / project workers / projects）
  - 当前哪些资源已经启动
  - 当前哪些资源处于 busy
  - 当前 waiting / recoverable_offline / wake_actions 概况
- `status probe` 现已直接显示 `startup=...`
- activity cards 现会在“重启后暂无闭包执行但 framework 已完成 startup refresh”时回退显示 startup config/state 摘要，供 QQ/Web 共享同一份重启更新真源

## 2026-04-20 project role owner-loop bias and minimal collaboration loop landed

- `project role` 不再只偏 `claim/submit` 执行动作；当前动态工具偏置已补齐：
  - `project.task.create`
  - `project.task.review`
  - `agent.assign`
- 这样 project agent 才符合“单项目 owner / dispatcher / reviewer”的设计，不会退化成纯 worker。
- 当前最小协作闭环已验证：
  - system/owner `project.task.create`
  - system/owner `agent.assign`
  - worker `project.task.claim`
  - worker `project.task.submit`
  - owner `project.task.review`
- 这条链说明：system/project/worker 当前已可在同一 runtime truth 上完成最小 managed-task 闭环；后续缺的主要是 detached execution / remote peer 真执行，不是 task truth 本身。

## 2026-04-20 daemon project recovery skeleton landed

- `recover_project_agents` 不再只是 daemon observation result。
- attached daemon 当前已具备最小执行骨架：
  - `derive recovery action`
  - `materialize startup topology`
  - `filter recoverable offline projects`
  - `execute wake queue`
  - `rematerialize startup truth`
- 新增 recovery report：
  - `~/.fin/runtime/projects/recovery_reports.json`
  - `~/.fin/runtime/current/current_project_recovery.json`
- 当前语义：
  - local recoverable project -> 推进到 `idle/project_ready`
  - remote recoverable project -> 推进到 `waiting/await_remote_connect`

## 2026-04-20 project supervision snapshot landed

- framework 新增 `project supervision snapshot`：
  - `~/.fin/runtime/projects/supervision.json`
  - `~/.fin/runtime/projects/supervision/<project_id>.json`
  - `~/.fin/runtime/current/current_project_supervision.json`
- 它不是第二套 presence，而是 framework 对每个 project agent 的“下一步控制判断”：
  - `ready`
  - `resume_ready`
  - `busy`
  - `waiting`
  - `recover_needed`
- 对应的最小动作：
  - `observe_ready`
  - `resume_project_task`
  - `monitor_running_task`
  - `await_remote_connect`
  - `recover_project_agent`
- 另外 wake request 现在会携带 `resume_task_id`，project agent 被唤醒时可以知道应接续哪个 task

## 2026-04-20 project execution handoff skeleton landed

- framework 不再只把 `resume_project_task` 停留在 supervision intent。
- 对本地且 `resume_ready` 的 project，当前会继续 materialize：
  - `~/.fin/runtime/projects/execution_handoffs.json`
  - `~/.fin/runtime/projects/execution_handoffs/<project_id>.json`
  - `~/.fin/runtime/current/current_project_execution_handoffs.json`
- runtime 新增 `handoff_project_task(...)`：
  - task 已 terminal -> skip
  - same worker 已 claim -> noop
  - 否则把 task 推到 `claimed` 并刷新 task registry / board
- status probe 现会直接暴露 `project_execution_handoffs=prepared/noop/missing_task` 摘要。
- 当前边界：
  - 已有 handoff/claim 真源
  - 还没有 detached/local project runtime 自动 pickup 该 task 执行

## 2026-04-20 project runtime pickup truth landed

- framework 新增 `project runtime pickup snapshot`：
  - `~/.fin/runtime/projects/runtime_pickups.json`
  - `~/.fin/runtime/projects/runtime_pickups/<project_id>.json`
  - `~/.fin/runtime/current/current_project_runtime_pickups.json`
- 它基于：
  - `current_project_execution_handoffs.json`
  - session `control/execution_state.json`
  - session `queue/pending_inputs.json`
- 当前最小 pickup state：
  - `running`
  - `waiting_external`
  - `paused`
  - `ready_to_resume`
  - `claimed_idle`
  - `missing_binding`
  - `missing_session`
- status probe 现已直接暴露 `project_runtime_pickups=` 摘要。
- project agent presence 现在也会从单纯的 `resume_ready` 再推进到更贴近运行事实的 busy/waiting/idle。
- 边界：
  - 已经知道“是否具备继续跑的条件”
  - 还没有真正 detached/local runtime 自动执行下一轮 provider closure

## 2026-04-19 build/install gate recovery

- 已完成正式 gate recovery：
  - `cargo fmt --all --check`
  - `cargo test -p fin-runtime -p fin-cli -p fin-debug-server --manifest-path rust/Cargo.toml`
  - `python3 scripts/check-code-line-limit.py`
  - `cargo run -p fin-cli --manifest-path rust/Cargo.toml -- install-dev ~/.fin/config/user.toml 0.1.0001`
- 当前 install truth：
  - `~/.fin/install/versions/0.1.0001` 已生成
  - `~/.fin/bin/fin -> ~/.fin/install/current/bin/fin`
  - install / regression / harness summary 已分别写入：
    - `~/.fin/logs/install/0.1.0001.log`
    - `~/.fin/logs/regression/0.1.0001.log`
    - `~/.fin/harness/reports/0.1.0001/summary.json`
- 结论：
  - M1 closeout 不再被正式 build/install gate 阻断
  - closeout 文档需要同步修正，避免继续保留“仍被 line-limit 阻断”的旧结论

## 2026-04-19 当前状态总结已固定

- 已新增 `docs/closeout/m1-current-state-summary.md`
- 文档结论：
  - 当前 `fin` 已进入“可冻结、可验证、可继续成熟化”的 M1 后段状态
  - 单 agent runtime 内核已经具备：
    - 真实 provider
    - 多轮推理
    - auto tool loop
    - queue / interrupt / reminder / tick / heartbeat / daemon observation
    - durable truth + Web/debug 观察链
    - build/install gate
- 下一阶段建议不再扩新框架，而是进入 `M1.1 stability pass`：
  1. receipt 标准化
  2. 推理主链 review 固化
  3. truth consistency 小范围修正

## 2026-04-19 单 agent 推理主链 review 已固定

- 已新增 `docs/closeout/m1-inference-mainline-review.md`
- 当前主链冻结为：
  - `entry/orchestration -> runtime truth -> materialized/session truth -> web/debug observe`
- owning files 已在文档中明确：
  - runtime 主链：`lib.rs` + `closure_runtime*.rs`
  - control plane：`control_plane.rs` + `routing_actions.rs` + `scheduler.rs`
  - materializer：`session_materializer*.rs` + `session_record_journal.rs`
  - CLI glue：`web_debug.rs` + `status_probe.rs` + `session_commands.rs`
- 当前固定反模式也已写清：
  - 不能在 Web 层补 runtime 结论
  - 不能把完整推理历史塞回 `messages.json`
  - 不能在 CLI wrapper 层私自发明 control plane 语义

## 2026-04-19 receipt 标准化第一步已落地

- 已新增：
  - `scripts/build-receipt-index.py`
  - `docs/closeout/m1-receipt-standardization.md`
- 已实际生成：
  - `~/.fin/harness/reports/0.1.0001/receipt-index.json`
  - `~/.fin/harness/runs/m1-closeout-20260419-163831/runtime-home/harness/reports/0.1.0001/receipt-index.json`
- 当前冻结策略：
  - 先统一 `receipt-index.json` 作为目录页
  - 原始 receipt 继续保留各自真源
  - 不直接一口气重写所有 receipt schema

## 2026-04-19 async control-boundary receipt 已落地

- `fin control-boundary-demo <user.toml>` 已扩展为真实 async control-plane 样本生成器，当前固定链路：
  - `/new`
  - seed
  - `/pause`
  - queued inputs x2
  - `/resume-run`
  - `wait.remind`
  - `reminder_fired`
  - `supervisor_heartbeat_due`
  - `stale_lease`
- 已实际生成 closeout session：
  - `session-20260419191817`
- `mainline-receipts.json` 中 `control_boundary` 现已验证：
  - `recent_tick_sources = [resume_run, reminder_fired, supervisor_heartbeat_due]`
  - `waiting_external_observed = true`
  - `reminder_scheduled_observed = true`
  - `reminder_fired_observed = true`
  - `heartbeat_due_observed = true`
  - `stale_lease_observed = true`
- 后续规则：
  - 需要 control-plane 证据时，优先看 async receipt，不再用“有 heartbeat 文件”冒充完整 control-boundary

## 2026-04-19 installed-binary smoke receipt 已进一步标准化

- `install-dev / build-dev` 当前默认会调用 `scripts/build-receipt-index.py`
- 自动刷新：
  - `~/.fin/harness/reports/<build-version>/receipt-index.json`
- `install_smoke` summary 现已带出：
  - `session_id`
  - `task_id`
  - `operation_id`
  - `verified_paths`
- 这意味着 installed-binary smoke 不再只是“有个 summary.json”，而是已经能稳定挂到 build/install flow 里，作为 receipt-index 的自动一部分

## 2026-04-19 M2 最小入口建议已固定

- 已新增 `docs/closeout/m2-entry-recommendation.md`
- 当前推荐的 M2 第一入口不是 remote peer / channel / detached daemon，而是：
  1. `runtime-owned control-plane hardening`
  2. `session/task/topic formalization boundary clarification`
- 原因：
  - 这条线最延续 M1 已收下的 truth / control / session 主线
  - 最不容易重新发散到分布式、鉴权、网关、后台常驻等大扩张
  - 对后续 daemon / peer / multi-agent 都是前置基础

## 2026-04-19 M1 当前状态总收口

- 当前 M1.1 stability pass 的核心目标已完成：
  - receipt 标准化已落地
  - inference mainline review 已冻结
  - mainline receipts 三类全部 passed
- 当前若继续推进，默认不再把目标表述成“继续补 M1 功能”，而是：
  - 保持 regression/receipt 持续为绿
  - 只做防回退与 truth consistency
  - 若要扩能力，先明确是否正式切入 M2

## 2026-04-19 review-driven truth fixes

- 已按 review 修正三项 P0 truth 问题：
  1. `InterruptedSegmentRecord.segment_id` 现在包含 `session + turn + paused_at`，同一 session 多次 pause 不再复用同 id。
  2. runtime `StepRecord.step_index` 改为与 `step_id` 同步逐步分配，`provider_request / model_parse / control_feedback / tool_dispatch / finalize` 在同一 turn 内严格单调递增。
  3. 多轮自动 tool loop 的 `tool_call_id` 改为带 round 维度：`tool-model-{operation_id}-r{round}-{index}`，避免 round 之间撞 id。
- 顺手补了 round truth 的一致性修正：follow-up round 的 tool dispatch summary 现在使用“当轮结果”，不再错误复用累计 dispatch 状态。
- archive summary 已从“first matching event”改成 `latest-per-type` 读取策略，避免多 round operation 在 archive inspector 中展示第一轮旧状态。
- 验证已通过：
  - `cargo test -p fin-runtime -p fin-cli -p fin-debug-server --manifest-path rust/Cargo.toml`
  - `(cd rust/crates/debug-server/webui && npx tsc -p tsconfig.json)`

## 2026-04-19 activity-card owning layer correction

- `activity cards + tool semantics` 的 builder 已从 `fin-debug-server` 下沉到 `fin-runtime`：
  - 新真源：`fin-runtime::build_activity_cards`
  - contract 仍留在 `fin-contracts`
  - Web debug 与 QQ/text channel 都只消费同一份 runtime/session projection
- 修正原因：
  - `fin-cli` 原先通过 `fin_debug_server::build_activity_cards` 读取活动卡，形成 `CLI -> debug-server` 的反向依赖
  - 这违反了“Web/debug 只能做观察层，不拥有运行语义”的项目硬边界
- 当前固定规则：
  - 活动卡结构定义放 `fin-contracts`
  - 活动卡聚合/工具语义解释放 `fin-runtime`
  - `fin-debug-server` / WebUI / text channel 只负责 transport + render + delivery policy
- 本轮验证：
  - `cargo test -p fin-runtime --manifest-path rust/Cargo.toml`
  - `cargo test -p fin-debug-server --manifest-path rust/Cargo.toml`
  - `cargo test -p fin-cli channel_peer_activity_delivery --manifest-path rust/Cargo.toml`
  - `cargo build -p fin-cli --manifest-path rust/Cargo.toml`
  - `npx tsc -p rust/crates/debug-server/webui/tsconfig.json`
  - live `http://127.0.0.1:4040/api/activity_cards.json` 返回 `200`

## 2026-04-20 qqbot/web-debug/channel delivery corrections

- `serve_web_debug` 不能把 TOML 内容当路径传给 QQ bridge；必须把原始 `user.toml` 路径一路传下去，否则 bridge 会错误回退到默认配置或 env。
- `deliver_pending_messages_for_target` 遇到被清洗为空的 assistant/system 消息时，不能写桥接请求，也不能把 delivered cursor 往前推进。
- activity-card heartbeat 的 active 判定需要把 `waiting` 视为活跃态，否则长等待场景会失去最小心跳更新。

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
- user config 继续保持简单，只填 provider 必要信息；system policy 自动映射出 `project role -> explicit provider.model priority path`，不引入 user/system merge 歧义。
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
- 当前 prompt role 真源已纠偏为只有：
  - `system`
  - `project`
- `worker/reviewer/analyzer` 不再作为独立 role 扩展；这些语义应回收到 `project` role 的 workflow emphasis / tool policy 中。
- 仍需明确的不是新 role，而是框架组件类型：
  - `capability_router` 或 `peer_router`
  - `channel_gateway`（即使不直接推理，也应有 contract/schema 位）
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

## 2026-04-18 qqbot connectivity plane 已落地 + 实测 upstream 可通

本轮新增：

1. `probe_builtin_qqbot_connectivity`
   - 为内置 qqbot peer 增加最小上游探活闭环
   - 使用 `POST https://bots.qq.com/app/getAppAccessToken`
   - 不持久化 access token，只验证上游 auth/connectivity

2. credential 解析优先级（当前为 bootstrap 兼容）
   - `FIN_QQBOT_APP_ID` + `FIN_QQBOT_CLIENT_SECRET`
   - `QQBOT_APP_ID` + `QQBOT_CLIENT_SECRET`
   - `~/.finger/runtime/plugins/openclaw-qqbot.json`
   - `~/.finger/config/channels.json`

3. qqbot peer state 新增字段
   - `connectivity_checked_at`
   - `upstream_authenticated_at`
   - `upstream_expires_at`
   - `credential_source`
   - `last_connectivity_error`

4. 新增事件
   - 成功：`channel.peer.upstream_authenticated`
   - 失败：`channel.peer.connectivity_probe_failed`

5. `/qqbot connect`
   - 通过本地命令触发 connectivity probe
   - `status` 也会显示 `credential_source + last_error`

### 实测结果

已用本机现有 qqbot 凭证做真实 probe（脱敏）：

- `credential_source = legacy_finger_plugin`
- `app_id_masked = 1903…(len=10)`
- `POST https://bots.qq.com/app/getAppAccessToken`
- 返回 `status = 200`
- `ok = true`
- `expires_in = 3977`
- 说明：**当前 qqbot upstream 是通的**

## 2026-04-18 推理核心收口（第一批）

- 已补 canonical turn / step 真源：新增 `TurnRecord`、`StepRecord`、`ProviderRequestRecord`、`ProviderResponseRecord`、`RoutingDecisionRecord`。
- `run_closure` 现在会为每个 provider round 记录 request/response + step ledger；不再只有 closure 末尾的一份 summary。
- session/runtime 当前已新增 durable artifacts：
  - `runtime/current/current_turn.json`
  - `runtime/current/current_step_records.json`
  - `runtime/current/current_provider_requests.json`
  - `runtime/current/current_provider_responses.json`
  - `runtime/current/current_routing_decision.json`
  - `sessions/.../turns/recent_turns.json`
  - `sessions/.../steps/recent_steps.json`
  - `sessions/.../provider/recent_provider_requests.json`
  - `sessions/.../provider/recent_provider_responses.json`
  - `sessions/.../tasks/routing/recent_decisions.json`
- 当前 control block 已不只是展示：会先落成 `RoutingDecisionRecord`，作为后续 tentative->formal task / topic switch / revive 的框架输入锚点。
- 本轮还没有做真正 pause/resume / pending input queue / interrupted merge，只是先把 turn-level canonical truth 补齐。
- 回归：`cargo test -p fin-runtime -p fin-cli --manifest-path rust/Cargo.toml` 通过。

## 2026-04-19 推理核心收口（第二批：pause/resume + pending input 最小状态机）

- 新增执行状态真源：`ExecutionStateRecord`、`PendingInputRecord`、`PauseCheckpointRecord`。
- CLI / web-debug 现已支持最小状态机：
  - `/pause [reason]`：写 `execution_state=paused` + `pause_checkpoint`
  - `/resume-run`：恢复到 `idle`
  - paused / running 时新输入不直接推理，进入 `queue/pending_inputs.json`
- 新增当前态落盘：
  - `runtime/current/current_execution_state.json`
  - `runtime/current/current_pause_checkpoint.json`
  - `runtime/current/current_pending_inputs.json`
- session 级落盘：
  - `sessions/.../control/execution_state.json`
  - `sessions/.../control/pause_checkpoint.json`
  - `sessions/.../queue/pending_inputs.json`
- `status_probe` 现在会显示：`phase/status`、`active_step`、`resume_from`、`pending_inputs`。
- `wait.remind` 到期注入 system reminder 后，会把 `waiting_external -> idle`。
- 这仍然不是“真正并行推理”或“真正 provider 中途恢复”；当前只是先把框架状态机、checkpoint 与 queue 真源补齐。
- 回归：`cargo test -p fin-cli -p fin-runtime --manifest-path rust/Cargo.toml` 通过。

## 2026-04-19 推理核心收口（第三批：queue drain + resumed continuation）

- `/resume-run` 不再只是把 `paused -> idle`；现在若 `queue/pending_inputs.json` 中有待处理输入，会自动 dequeue 第一条并继续执行一次正常 closure。
- dequeue 后会更新：
  - `queue/pending_inputs.json`
  - `runtime/current/current_pending_inputs.json`
  - `execution_state.pending_input_count`
- 继续执行时走与正常 chat 相同的推理链，不做旁路拼接；因此 turn/digest/tool/reasoning/turn-record/step-record 都继续保持同一套真源。
- 本轮新增回归：`resume_run_drains_pending_queue_and_executes_next_input`，用静态 provider 验证 `/resume-run` 会真正消费 pending 输入并写入会话消息。
- 这仍然还没做 interrupted segment merge；当前 resumed continuation 是“队列驱动的新 closure 继续同一 session/task”，不是 provider 中途恢复。
- 回归：`cargo test -p fin-cli -p fin-runtime --manifest-path rust/Cargo.toml` 通过。

## 2026-04-19 推理核心收口（第四批：interrupted segment + merge-back）

- `/pause` 现在除了写 `pause_checkpoint` 与 `execution_state=paused`，还会生成 `InterruptedSegmentRecord`。
- 新增 session/runtime artifacts：
  - `sessions/.../interrupts/recent_segments.json`
  - `sessions/.../interrupts/latest_segment.json`
  - `sessions/.../interrupts/recent_merges.json`
  - `sessions/.../interrupts/latest_merge.json`
  - `runtime/current/current_interrupted_segment.json`
  - `runtime/current/current_segment_merge.json`
- `/resume-run` 在 drain queue 时，如果存在 open interrupted segment，会在 resumed closure 落盘后写 `SegmentMergeRecord`，并把 segment 状态更新为 `merged`，写入 `merged_into_turn_id / merged_into_operation_id / merged_at`。
- 这一步明确了当前语义：
  - interrupted segment **不生成 closure digest**
  - resumed continuation 仍然是 **同一 session/task 下的新 closure**
  - merge-back 通过 `SegmentMergeRecord` 和被更新后的 `InterruptedSegmentRecord` 进行索引，不是假装中断那一轮已经完成
- 新增回归：
  - `/pause` 后 `current_interrupted_segment_path` 存在且 segment 为 `open`
  - `/resume-run` 消费队列后 `recent_merges.json` 出现 `resume_as_new_closure`，`recent_segments.json` 里的 segment 状态变为 `merged`
- 回归：`cargo test -p fin-cli -p fin-runtime --manifest-path rust/Cargo.toml` 通过。

## 2026-04-19 推理核心收口（第五批：interrupt policy + queue auto-drain）

- `chat_policy` 已接入 `web-debug` 发送链，当前请求分为四类：
  - `status_probe`：旁路读取框架当前态，不生成 closure
  - `interrupt_request`：立即执行，不进入 pending queue
  - `queue`：当 execution state 为 `paused | running | waiting_external` 时普通输入入队
  - `run_now`：空闲态直接执行 closure
- `/interrupt <message>` 与 `input_kind=interrupt_request` 现在都会走 immediate path：
  - 若已有 open interrupted segment，则保留它不 merge
  - 若当前是 `running | paused` 且还没有 open segment，会先补 `pause_checkpoint + interrupted segment`
  - 然后直接执行新的 closure
- `/resume-run` 现在不只 drain 一条 pending input，而是会在 execution state 维持 `idle` 时持续 drain，直到：
  - queue 为空
  - 或进入 `waiting_external`
  - 或再次 `paused`
  - 或执行失败
- 当前 merge-back 只发生在 `/resume-run` drain 的第一条 resumed closure；后续自动 drain 的 closure 不再重复 merge 原 interrupted segment。
- 新增回归：
  - `interrupt_request_runs_immediately_and_preserves_open_segment`
  - `resume_run_auto_drains_multiple_pending_inputs_until_queue_empty`
- 回归：`cargo fmt --all --manifest-path rust/Cargo.toml && cargo test -p fin-cli -p fin-runtime --manifest-path rust/Cargo.toml` 通过。

## 2026-04-19 推理核心 audit + 收口（第六批：wait.remind yield + round-level hooks）

本轮先做了 runtime audit，确认当前 durable truth 的分层是：
- channel render 真源：`conversation/messages.json`（只保留 user / assistant）
- 完整 closure 索引真源：`TurnRecord`
- turn 内完整推进真源：`StepRecord`
- 推理可见真源：`ReasoningViewRecord`
- 工具真源：`ToolExecutionRecord`
- provider 往返真源：`ProviderRequestRecord / ProviderResponseRecord`

结论：
- `conversation/messages.json` 不应承载所有工具 / reasoning / provider 明细，否则会破坏当前 Web turn 聚合逻辑；完整历史应继续通过 `turns / steps / reasoning / tools / provider / closures` 这一组 records 追索。
- 之前 runtime 的一个真问题是：`wait.remind` 调度后不会结束本轮，而会继续同 turn 自动 tool loop，直到 hit round limit。这会制造重复 reminder 与错误的“继续推理”语义。
- 另一个缺口是：虽然已经有 `StepRecord`，但 `event_ids` 为空，且每轮 provider/model/control/tool 没有独立 round-level event，导致“每一轮到底发生了什么”不够直观。

已修：
- `wait.remind` 现在会设置 `yield_requested=true`，当前 closure 在调度 reminder 后立即收束，不再继续同 turn tool loop。
- `operation.completed` 对 reminder waiting 现在会产出：
  - `status = waiting_external`
  - `stop_source = wait.remind`
- 新增 round-level event hooks：
  - `provider.round_completed`
  - `model.output_round_parsed`
  - `control.feedback_round_recorded`
  - `tool.dispatch_round_completed`
- `StepRecord.event_ids` 现已回填到对应 step（至少 provider_request / model_parse / control_feedback / tool_dispatch）。

新增回归：
- `runtime_closure_records_wait_reminder_tool_and_event`
  - 验证 `wait.remind` 不再触发多轮 provider loop
  - 验证 `operation.completed = waiting_external / wait.remind`
- `runtime_closure_records_round_level_events_for_multi_round_tool_loop`
  - 验证两轮 tool loop 会产生两组 round-level events
  - 验证相关 `StepRecord.event_ids` 非空

回归：`cargo fmt --all --manifest-path rust/Cargo.toml && cargo test -p fin-runtime -p fin-cli --manifest-path rust/Cargo.toml` 通过。

## 2026-04-19 推理核心收口（第七批：retention config + round durable records）

这轮把“资源受控、自动清理、避免无界增长”一起落进了 runtime：

### 1. retention 已进入 system config
- 新增 `RuntimeRetentionConfig`，挂在 `SystemConfig.runtime.retention` 下。
- 当前可控窗口包括：
  - `recent_context_limit`
  - `recent_digest_limit`
  - `recent_reasoning_limit`
  - `recent_tool_record_limit`
  - `recent_closure_limit`
  - `recent_provider_request_limit`
  - `recent_provider_response_limit`
  - `recent_step_record_limit`
  - `recent_turn_limit`
  - `recent_routing_decision_limit`
  - `recent_round_limit`
  - `session_message_limit`
  - `reminder_pending_limit`
- 这些都是 system/runtime 层配置，不暴露给普通 user config 选择，符合之前“用户配置尽量简单，系统配置集中控制”的原则。

### 2. per-round durable records 已落地
- 新增 `RoundRecord`，表达单个 closure 内每一轮 provider/model/control/tool 的稳定汇总。
- 新增落盘：
  - `runtime/current/current_rounds.json`
  - `sessions/.../rounds/recent_rounds.json`
  - `sessions/.../rounds/latest.json`
- `last_run.json` 新增：
  - `current_rounds_path`
  - `session_recent_rounds_path`

### 3. 自动清理策略
- 所有 recent windows 继续沿用 `latest overwrite + recent bounded window`。
- 现在窗口大小不再硬编码在 runtime crate，而是走 system retention config。
- 每次 persist 时自动 `trim_head`，不会因为 round/step/provider/tool/message 增长而无限放大。
- 新增测试已验证：把 `recent_round_limit=2`、`session_message_limit=4` 后，三轮 transcript 结束只保留最近 2 个 round 和最近 4 条会话消息。

### 4. 当前边界
- 本轮控制的是 current/recent/materialized windows 的资源增长。
- `events/stream.jsonl` 仍是 session raw ledger，不在这轮裁剪；后续若做 archive/rotation，必须走“保留事实真源 + 冷归档”的方案，不能直接丢失语义。

回归：`cargo fmt --all --manifest-path rust/Cargo.toml && cargo test -p fin-config -p fin-runtime -p fin-cli --manifest-path rust/Cargo.toml` 通过。

### 6. debug-server / Web 侧 archive-aware event viewer（本轮）
- 后端新增显式 API：
  - `/api/session_event_archive_index.json`
  - `/api/session_events_segment.json?tier=local|cold&segment=segment-XXXXXX.jsonl`
- 保持 `/api/session_events.json` 只返回 **live hot stream**，不把 archive 混进默认 timeline，避免普通页面无界拉全量 raw events。
- `session_event_archive_index.json` 会在原始 index 基础上补出：
  - `local_segments[]`
  - `cold_segments[]`
  - 每个 segment 的 `relative_path + event_count`
- Web inspector 的 `Operation & Event` 卡已支持：
  - `live / local archive / cold archive` scope 切换
  - 点击 segment 后显式加载该 archive segment 的 raw events
  - 默认仍保持 live timeline，不改变聊天区和 turn 聚合真源

### 7. 这轮验证
- `cargo fmt --all --manifest-path rust/Cargo.toml`
- `(cd rust/crates/debug-server/webui && npx tsc -p tsconfig.json)`
- `cargo test -p fin-debug-server --manifest-path rust/Cargo.toml`
- 结果：`fin-debug-server` 20 tests passed

### 8. archive viewer 第二步：segment 内 operation 级聚焦
- Web inspector 现在不只是能选 `segment`，还能在当前 ledger scope 内看到 `operation list`
- 点击 operation 后：
  - 只过滤当前 ledger timeline
  - 如果该 operation 同时仍存在于 live `focusTurns`，则同步更新左侧主选中 operation
  - 如果它只是 archive 中的旧 operation，则只在 inspector 内局部聚焦，不伪造 live session truth
- 这样 archive drill-down 已从“看 raw event 段”升级到“看 raw event 段里的某个 operation”

### 9. 本轮额外验证
- `(cd rust/crates/debug-server/webui && npx tsc -p tsconfig.json)` 通过
- `cargo test -p fin-debug-server --manifest-path rust/Cargo.toml` 通过
- 结果：`fin-debug-server` 22 tests passed

### 10. archive operation 第三步：从 raw events 重建可读摘要
- 当前 selected archive operation 已不只是“过滤 timeline”：
  - 现在会从该 operation 的 raw events 中重建一份可读摘要
  - 读取来源仅限已有事件 payload，不补第二套业务真相
- 当前重建内容包括：
  - request input
  - provider / model
  - operation status / stop_source / provider finish / http status
  - control origin / topic shift / simple query
  - execution note summary
  - reasoning summary
  - digest summary
  - tool summary
- 这使 archive 中的历史 operation 已经接近“轻量 closure 视图”，而不只是 raw event 列表

### 11. 本轮再次验证
- `(cd rust/crates/debug-server/webui && npx tsc -p tsconfig.json)` 通过
- `cargo test -p fin-debug-server --manifest-path rust/Cargo.toml` 通过
- 结果：`fin-debug-server` 23 tests passed

### 12. archive operation 第四步：live/materialized 关联索引
- 当前 selected archive operation 现在还会额外挂出一块 `Linked live / materialized records`
- 它只使用当前 session 的 recent materialized windows 做关联，不创造新事实：
  - live turn（若该 operation 仍在 live focusTurns）
  - digest
  - reasoning view
  - closure trace
  - tool records
- 当前展示内容包括：
  - availability / recent-window-miss
  - live turn snippet
  - digest summary
  - reasoning summary
  - closure summary
  - tool summary
- 这样 archive operation 已同时具备：
  - raw event timeline
  - event-derived readable summary
  - live/materialized cross-link index

### 13. 本轮再次验证
- `cargo fmt --all --manifest-path rust/Cargo.toml`
- `(cd rust/crates/debug-server/webui && npx tsc -p tsconfig.json)` 通过
- `cargo test -p fin-debug-server --manifest-path rust/Cargo.toml` 通过
- 结果：`fin-debug-server` 25 tests passed

## 2026-04-19 推理核心收口（第八批：raw event ledger rotation + cold archive）

这轮修的是 session raw event ledger 本身：

### 1. 发现的真问题
- 之前 `sessions/.../events/stream.jsonl` 每次 closure persist 都是 `File::create` 重写。
- 这意味着它并不是 session append-only ledger，而只是“当前 closure 的 latest event slice”。
- 这和我们前面反复确认的 raw event ledger 真源定位不一致。

### 2. 当前修正后的语义
- `events/stream.jsonl` 现在变成 **hot live stream**：保留最近一段 session raw events。
- 超出 `runtime.retention.session_event_hot_limit` 后，不再丢弃，而是把溢出 head spill 成 archive segment：
  - session 热归档：`sessions/.../events/archive/segment-XXXXXX.jsonl`
- 若 session 本地 archive 文件数超过 `runtime.retention.session_event_local_archive_file_limit`：
  - 再把更老的 segment 移到冷归档：
  - `~/.fin/archive/sessions/YYYY/MM/<session-id>/events/segment-XXXXXX.jsonl`

### 3. 结果
- raw fact truth 不再因 current/recent 限额被静默删除。
- session 热路径资源受控：
  - hot stream 行数 bounded
  - 本地 archive 文件数 bounded
- 更老 raw truth 进入 `~/.fin/archive/...` 冷数据区，符合 runtime-home 文档中的 archive 设计。

### 4. 新增索引
- `sessions/.../events/archive_index.json`
- `runtime/current/current_event_archive_index.json`
- `last_run.json` 新增：
  - `current_event_archive_index_path`
  - `session_event_archive_index_path`

### 5. retention 配置新增两项
- `session_event_hot_limit`
- `session_event_local_archive_file_limit`

当前验证过：
- hot stream 超限会分段归档
- 本地 archive 超限会移动到冷归档
- live + local archive + cold archive 的总 event 行数仍等于所有 closure 事件总数，没有事实丢失

回归：`cargo fmt --all --manifest-path rust/Cargo.toml && cargo test -p fin-config -p fin-runtime -p fin-cli --manifest-path rust/Cargo.toml` 通过。

## 2026-04-19 runtime-owned control plane slice (state transition downshift)

- 本轮先做最小 ownership 下沉：把 `execution/pending/pause/segment` 的**状态转移逻辑**从 `fin-cli` 下沉到 `fin-runtime::control_plane`。
- 当前 runtime control plane 已拥有的纯逻辑：
  - `running_state`
  - `state_after_run`
  - `failed_state`
  - `paused_state`
  - `resumed_state`
  - `new_pending_input`
  - `dequeue_pending_input`
  - `state_with_pending_count`
  - `clear_waiting_state_if_due`
  - `interrupted_segment`
  - `segment_merge`
  - `apply_segment_merge`
- `fin-cli` 现在只保留：
  - session/runtime current 路径解析
  - json 读写
  - last_run path 更新
  - command/web-debug glue
- 这一步的意义：先把 control plane 的**语义真源**收回 runtime，再在下一轮继续把 scheduler/supervisor / executable routing action 接上。
- 新增 runtime 单测覆盖：
  - pause state 继承 active turn/step
  - pending queue dequeue 语义
  - interrupted segment id 唯一性
  - exact-open-match merge 语义
  - run completion -> execution state 语义
- 回归通过：
  - `cargo fmt --all --manifest-path rust/Cargo.toml`
  - `cargo test -p fin-runtime -p fin-cli -p fin-debug-server --manifest-path rust/Cargo.toml`

## 2026-04-19 routing decision -> executable action (minimal closeout)

- 本轮把 `RoutingDecisionRecord` 从“观察结果”升级成 runtime 产出的 canonical `RoutingActionRecord`。
- 新增 contract：`RoutingActionRecord`
  - `action_kind`
  - `source_disposition`
  - `apply_immediately`
  - `prompt_user`
  - `prompt_text`
  - `suggested_task_id / suggested_topic_thread_id`
  - `confidence`
  - `reason`
- 新增 runtime 规则模块：`runtime::routing_actions`
  - `continue_current_task -> continue_current_task`
  - `tentative_simple_chat -> stay_tentative_session`
  - `candidate_existing_task -> ask_reuse_existing_task`
  - `candidate_topic_switch -> ask_topic_switch`
  - other -> `observe_only`
- `ClosureRun` 现在同时携带：
  - `routing_decision`
  - `routing_action`
- runtime event 新增：
  - `routing.action_derived`
- session/runtime artifacts 新增：
  - `runtime/current/current_routing_action.json`
  - `sessions/.../tasks/routing/recent_actions.json`
  - `sessions/.../tasks/routing/latest_action.json`
- `last_run.json` 现在会写：
  - `current_routing_action_path`
  - `session_recent_routing_actions_path`
- CLI/web-debug 当前最小消费：
  - 普通 assistant response 会把 `routing_action` 带回 `ChatSendResponse`
  - `status_probe` 会读取 latest routing action，并在 answer 中显示 `routing_action=...`
- 这一步仍然是“最小闭环”：
  - action 已是 runtime 真源
  - CLI 只消费，不再自己二次推断
  - 但真正的 scheduler / supervisor / auto-confirm / auto-switch 还没接
- 验证通过：
  - `cargo fmt --all --manifest-path rust/Cargo.toml`
  - `cargo test -p fin-runtime -p fin-cli -p fin-debug-server --manifest-path rust/Cargo.toml`

## 2026-04-19 runtime-owned scheduler skeleton + bounded queue drive

- 本轮新增 `SchedulerDecisionRecord`，并把 queue/supervisor 的第一层判断语义收回 runtime：
  - `wait_paused`
  - `wait_running`
  - `wait_external`
  - `await_user_confirmation`
  - `run_next_pending`
  - `stay_idle`
  - `observe_only`
- 新增 runtime 规则模块：`runtime::scheduler::derive_scheduler_decision`
  - 输入：`ExecutionStateRecord + pending_input_count + latest RoutingActionRecord`
  - 输出：canonical `SchedulerDecisionRecord`
- 新增 CLI 驱动模块：`cli::scheduler_driver`
  - 当前只负责：
    - 读取 state / pending / latest routing action
    - 生成 scheduler decision
    - 持久化 scheduler decision
    - 在 `run_next_pending` 时 bounded auto drive（max=8）
- scheduler decision 落盘：
  - `runtime/current/current_scheduler_decision.json`
  - `sessions/.../control/scheduler/latest.json`
  - `sessions/.../control/scheduler/recent_decisions.json`
- `last_run.json` 新增：
  - `current_scheduler_decision_path`
  - `session_recent_scheduler_decisions_path`
- 当前 web-debug 的 `/resume-run` 已不再直接写死 drain loop，而是改为：
  - `resume -> scheduler decision -> bounded drive`
- `status_probe` 现在会同时展示：
  - `routing_action=...`
  - `scheduler=...`
- 当前边界明确：
  - scheduler 决策语义已 runtime-owned
  - bounded drive 已 framework-owned
  - 但它还不是 daemon/clock 驱动的 autonomous tick，也还没有真正 supervisor/lease/heartbeat 管理
- 验证通过：
  - `cargo test -p fin-runtime -p fin-cli -p fin-debug-server --manifest-path rust/Cargo.toml`

## 2026-04-19 autonomous tick v1 (wake-driven)

- 当前已不只是 `/resume-run` 手动驱动：当 `inject_due_reminders` 触发 reminder fired 后，web-debug 入口会：
  1. `clear_waiting_if_due`
  2. 重新读取 binding
  3. 自动调用 scheduler bounded drive
- 新增显式本地入口：`/tick`
  - 作用：强制让 framework 执行一次 scheduler tick
  - 当前路径与 `/resume-run` 一样，都会走 `scheduler decision -> bounded drive`
- 新增测试覆盖：
  - `scheduler_driver::drive_scheduler_runs_pending_until_queue_is_empty`
  - `scheduler_driver::drive_scheduler_blocks_when_prompt_user_is_required`
  - `web_debug::tick_command_drives_pending_queue_when_scheduler_allows`
  - `web_debug::due_reminder_auto_ticks_scheduler_and_drains_pending_queue`
- 到这里第一版 autonomous tick 的边界已明确：
  - wake source：reminder fired / explicit `/tick`
  - drive mode：single-process bounded drive
  - owning truth：runtime scheduler decision + routing action
  - 还没有 daemon/timer loop/lease/heartbeat supervisor
- [2026-04-19] scheduler tick 已升级为独立 framework truth：新增 `SchedulerTickRecord`，`/tick`、`/resume-run` 的 bounded drive 与 `reminder fired` 自动唤醒现在统一走同一条 tick pipeline，而不是只在 wrapper 里直接 drain queue。
- [2026-04-19] tick 现在会把 `scheduler.tick_started / scheduler.tick_decision_recorded / scheduler.tick_drove_pending|blocked / scheduler.tick_completed` 写入 session `events/stream.jsonl`，并把 `latest_tick.json` / `recent_ticks.json` / `runtime/current/current_scheduler_tick.json` 作为控制面真源持久化。
- [2026-04-19] tick pipeline 当前边界已冻结：它只负责 framework control-plane truth（tick record + tick events + bounded scheduler drive），不伪装成 provider/runtime inference event；下一层再把常驻 supervisor/daemon 接到这条统一 tick 真源上。
- [2026-04-19] framework-side tick events 不再由 CLI 直接 append raw stream；现统一复用 `fin-runtime::append_framework_events` 进入 session event archive/rebalance/index 真源，避免 control-plane 事件再分叉出第二套热流/归档语义。
- [2026-04-19] `status_probe` 已纳入 latest tick 摘要：读取 `control/scheduler/latest_tick.json` 或 `runtime/current/current_scheduler_tick.json`，可直接看到 `source / drove_count / final action / blocked_by`，便于非中断并行询问当前 scheduler/tick 状态。
- [2026-04-19] supervisor 第一层真源已落地为 `SupervisorCycleRecord`：它不重写 scheduler/tick 语义，只包装一次 framework-owned cycle，记录 `source / tick_id / drove_count / blocked_by / next_wake_hint / result_summary`，为后续 daemon 常驻接管预留稳定入口。
- [2026-04-19] `web_debug` 中 `/tick`、`/resume-run`、`reminder fired` 现在统一先经过 `run_supervisor_cycle -> run_scheduler_tick -> drive_scheduler`；status probe 也已暴露 `supervisor=` 摘要，可非中断查看最近一次 supervisor cycle 的控制结论。
- [2026-04-19] supervisor cycle 现在具备 canonical blocked taxonomy：`await_user_confirmation / wait_external / wait_running / wait_paused / idle_no_work / auto_step_limit / observe_only`；daemon 以后应直接消费该 taxonomy，而不是再从 scheduler/tick 字段二次猜测阻断原因。
- [2026-04-19] supervisor cycle 已补 `next_check_at / heartbeat_interval_ms / lease_ttl_ms / next_wake_hint`：当前 `wait_running` 与 `auto_step_limit` 会生成下一次 heartbeat 检查时间，`wait_external`/`await_user_confirmation`/`idle_no_work` 则以 wake hint 为主，不做无意义轮询。
- [2026-04-19] supervisor heartbeat 第一版已落地为 `SupervisorHeartbeatRecord`：它负责观察最近一次 supervisor cycle，判断 `due_for_tick / stale_lease`，并在 `next_check_at` 已过期时自动触发 `supervisor_heartbeat_due` cycle；这使 daemon 常驻化前，framework 已具备最小自驱心跳能力。
- [2026-04-19] `web_debug` 入口现在会先记录一次 `supervisor_heartbeat`；`status_probe` 也已暴露 `heartbeat=` 摘要，能直接看到 `source / due_for_tick / stale_lease / blocked_kind / next_check_at`，用于非中断检查常驻控制面的健康度。
- [2026-04-19] daemon state 第一版已落地为 `DaemonStateRecord + DaemonRecoveryActionRecord`：当前以 `web_debug_attached` 作为 attached daemon 真源，记录 `lifecycle_state / supervision_state / pid / health_state / recovery_needed / recovery_action_kind`，为将来的 headless daemon 统一生命周期模型铺路。
- [2026-04-19] attached daemon recovery skeleton 现在直接消费 `SupervisorHeartbeatRecord + SupervisorCycleRecord`：当 heartbeat 标记 `stale_lease=true` 时派生 `recover_stale_cycle`，否则派生 `continue_heartbeat_monitoring / await_external_event / await_user_confirmation / observe_only` 等 canonical recovery actions，避免 daemon 层再次从原始事件猜恢复策略。

## 2026-04-19 M1 closeout freeze

- 当前项目正式切换到 **M1 收口模式**：默认优先 `scope freeze -> regression matrix -> blocker fix -> receipts`，不再继续扩大的 framework 设计。
- 本轮新增 closeout 真源文档：
  - `docs/closeout/m1-scope-and-freeze.md`
  - `docs/closeout/m1-regression-matrix.md`
  - `docs/closeout/m1-known-gaps-and-m2-backlog.md`
- M1 冻结边界已明确：
  - 包含：单 agent 推理闭环、context rebuild、tool loop、stop/wait/reminder、session truth、event archive、status/tick/supervisor/heartbeat/daemon state
  - 不包含：真多 agent 执行面、跨机协作、detached daemon、真正 pause/resume、普通推理并行、完整 session/task/topic 产品化、成熟 memory graph
- 当前 closeout 期间只允许做：
  - 阻塞 M1 的 bug 修复
  - regression matrix 补齐
  - receipts / hand-check 证据整理
  - truth consistency 修复
- 当前 closeout 的主要证据缺口也已单独列出：
  - installed-binary smoke 自动化仍缺
  - provider real smoke receipt 需固化
  - compact rebuild 对照 evidence 需补
  - 4040 web debug 真源验收需保留 receipt

## 2026-04-19 M1 receipts completed

- 本轮 closeout receipts 已在隔离 run `~/.fin/harness/runs/m1-closeout-20260419-163831/runtime-home` 完成，并汇总到 `docs/closeout/m1-receipts-2026-04-19.md`。
- formal `install-dev` 已真实执行，但被 line-limit gate 阻断；当前阻断文件包括：
  - `rust/crates/cli/src/session_commands.rs`
  - `rust/crates/cli/src/web_debug.rs`
  - `rust/crates/runtime/src/lib.rs`
  - `rust/crates/runtime/src/session_materializer.rs`
  - 以及其余超 500 行文件
- installed-binary smoke 已用隔离 manual install layout 完成 receipt：
  - staged binary `config-check/runtime-demo/debug-projection`
  - current bin link `config-check`
  - 对应 receipt：`installed-binary-smoke-manual.json`
- provider real smoke 已完成 receipt：
  - `provider = ali-coding-plan`
  - `protocol = anthropic-wire`
  - `model = qwen3.6-plus`
  - `status = 200`
  - `output_text = OK`
- 4040 web-debug live hand-check 已完成：
  - `/status` 返回 `response_kind=status_probe`
  - `events_count=0`
  - `messages/digests/context-count` 不变
- `/compact` live hand-check 已完成：
  - 返回 `response_kind=system_notice`
  - `events_count=0`
  - `current_context.json` 与 `current_rebuild_index.json` hash 改变
  - `recent_contexts` 计数 `3 -> 4`
  - `conversation/messages.json` 因 notice 追加而变化
- [2026-04-19] M1.1 receipt 第二步开始：从 `receipt-index` 继续推进推理主链 receipts，当前已有真实样本最稳的是 `session-test-real-transcript`，它能证明 history/context continuity（0001 -> 0002 -> 0003 continuity_tail 0 -> 2 -> 4）。
- [2026-04-19] 当前 closeout runtime-home 里还没有真实 multi-round auto-tool-loop artifact；本轮先标准化 mainline receipt schema 和生成脚本，先产出 `history_context`，并把 `auto_tool_roundtrip` 明确标记为 `missing`，避免无证据硬宣称完成。
- [2026-04-19] `control_boundary` receipt 当前先收 session durable control-plane artifacts（heartbeat / daemon state / recovery action 等）；queue/wait/interrupt 的更强样本后续再补到 closeout run，不在本轮伪造事实。
- [2026-04-19] 已新增 `fin mainline-demo <user.toml>`：用 deterministic provider 在隔离 runtime-home 里生成 3 turn history + 第 3 turn 的真实 2-round auto tool loop，session 为 `session-<namespace>-mainline`，用于 closeout 的 `auto_tool_roundtrip` receipt，避免再用单轮 transcript 假装 tool loop 通过。
- [2026-04-19] 已新增 `fin control-boundary-demo <user.toml>`：复用真实 web_debug control path 生成 `/new -> seed -> /pause -> queued x2 -> /resume-run -> /status` 的 stronger control-boundary session，能稳定产出 `execution_state / pause_checkpoint / pending queue / interrupted segment / segment merge / scheduler latest+tick / supervisor latest / heartbeat / daemon state`，用于 mainline `control_boundary` receipt。

## 2026-04-19 finger decommission + fin builtin qqbot peer

- 已显式停掉残留 live 旧链进程：
  - `5839 rust/target/debug/fin-cli web-debug ... 4056`
  - `5856 node ~/code/finger/dist/cli/index.js gateway-bridge start qqbot --stdio`
- 已禁用并移走旧自启动：
  - `com.finger.dual-daemon`
  - `com.finger.daily.project.analysis`
  - `com.finger.daily.user.analysis`
  - `com.finger.email.check`
  - `com.finger.news.digest`
  - `com.finger.weibo.timeline`
  - `com.finger.zombie.cleanup`
  - `ai.openclaw.gateway`
  - 证据日志：`~/.fin/logs/finger-decommission-20260419_212053.log`
- `fin` 已移除 qqbot 对 `finger gateway-bridge` 与 legacy finger config 的运行时依赖：
  - 新增 fin-owned runner 资产：`rust/crates/cli/assets/qqbot_peer_runner.mjs`
  - `web-debug` 当前实际拉起：`node ~/.fin/runtime/peers/qqbot/bin/qqbot-peer-runner.mjs`
  - `channel_peer_connectivity` 不再回退读取 `~/.finger/...`，改为 `env -> user.toml[channels.qqbot]`
- 已把 legacy qqbot 凭据一次性迁入 `~/.fin/config/user.toml`：
  - `[channels.qqbot]`
  - `app_id = "1903323793"`
  - `client_secret = "***"`（本地真实值已写入，笔记中脱敏）
- live 验证：
  - 4040 实例进程：
    - `40097 rust/target/debug/fin-cli web-debug ~/.fin/config/user.toml 4040`
    - `40261 node ~/.fin/runtime/peers/qqbot/bin/qqbot-peer-runner.mjs`
  - `/qqbot connect` 返回：`credential_source=user_toml:/Users/fanzhang/.fin/config/user.toml`
  - `runtime/peers/qqbot/events.jsonl` 已记录：
    - `channel.peer.bridge_spawned`（bridge_impl=`fin_builtin_runner`）
    - `channel.peer.bridge_start_requested`
    - `channel.peer.bridge_ready`
    - `channel.peer.upstream_authenticated`
- 当前剩余事实：
  - built-in peer 启动 / upstream auth / session binding 已通
  - 还没在本轮用真实 QQ 消息再次验证“单条外部输入 -> 单条 fin 回复”
  - 所以本轮结论是：**finger 已移除，fin builtin qqbot peer 已取代旧桥；真实 QQ roundtrip 还需一条外部消息做最终收口**

## 2026-04-19 text channel activity cards freeze

- 纯文字 channel 架构已冻结为双层卡体系：
  - `source-owned progress cards`
  - `system-owned user activity card`
- owning scope：
  - 每个 source（system/project/peer）各自维护一张当前卡
  - 用户前台会话由 `system agent` 维护一张总卡
- 更新规则：
  - 有变化才更新，无变化静默
  - 最长 1 分钟允许一次最小心跳
  - 渠道不支持编辑时走“紧凑重绘”，逻辑上仍视为同一张卡
- 可见性：
  - `hidden / compact / detailed / verbose`
  - 并允许 active / failed / waiting-too-long 自动提升
- verbose：
  - 允许 source card 分片
  - 总卡只引用摘要，不承载 verbose 明细
- 工具渲染：
  - 统一按“用户关心做了什么”语义化解释
  - WebUI 与文字 channel 共用同一套 tool semantic render truth

## 2026-04-19 activity card builder + shared tool semantics landed

- 已新增共享 contract：
  - `fin-contracts::ActivityCardsSnapshot`
  - `ToolSemanticView / SourceActivityCardView / UserActivityCardView`
- 已在 `fin-debug-server` 落地统一 builder：
  - `build_activity_cards(runtime_home)` 从 `last_run + session artifacts + current_execution_state + runtime/peers/registry.json` 聚合 source/user cards
  - 当前最小 source 已覆盖 `system-agent` 与 peer registry（含 qqbot peer）
- 已在 `fin-debug-server` 落地统一工具语义层：
  - `tool_semantics::semantic_views(...)`
  - 把 `ToolExecutionRecord` 规范化为 `category / verb / object / summary / detail`
- 已新增 API：
  - `GET /api/activity_cards.json`
- Web 状态层已接入 `activityCards` 读取，但本轮**不改你正在调整的具体展示**；先保证 Web / text channel 后续都能消费同一份后端真源
- 验证：
  - `cargo test -p fin-contracts -p fin-debug-server -p fin-cli --quiet`
  - `python3 scripts/check-code-line-limit.py`

## 2026-04-19 qqbot text channel delivery policy landed

- 已新增 `rust/crates/cli/src/channel_peer_activity_delivery.rs`
  - 持久化真源：`~/.fin/runtime/peers/qqbot/activity_delivery_state.json`
  - 负责：
    - 绑定当前会话对应的用户 target
    - 基于 `previous delivered card view vs current card view` 做 diff
    - 生成 compact text redraw
    - 在 active 状态下按 60s 规则允许最小 heartbeat delivery
- `qqbot bridge` 现在的最小闭环：
  - 收到用户消息后先绑定 target
  - 正常 assistant reply 会尝试**嵌入一份 compact activity card**
  - 后台 activity loop 每 5s 检查一次，但只有：
    - card diff 非空，或
    - active 状态且距离上次发送 >= 60s
    才会发送新的 compact redraw
- 当前实现边界：
  - 非编辑渠道不做“逐行 delta”，而是发送 compact redraw 文本
  - delivery 只以 `user_card + source_cards` 的 signature 作为比较真源
  - 还没有做更细的 verbose/source 分片投递策略
- 事件：
  - `channel.peer.activity_card_embedded`
  - `channel.peer.activity_card_send_requested`
  - `channel.peer.activity_card_send_failed`
  - `channel.peer.activity_card_prepare_failed`

## 2026-04-19 qqbot channel conversation/session restore + attached keepalive

- 用户纠正后的 blocker 已确认：问题不是 activity card 是否刷新，而是 qqbot 之前只是单次 bridge，没有真正的 `target -> session` 会话恢复、session truth 驱动的自动回复，以及 bridge 异常后的保活。
- 本轮已补 `~/.fin/runtime/channels/qqbot/conversations.json` 作为 channel conversation 真源：记录 `target / session_id / last_inbound_message_id / last_delivered_message_id`，用于 target 级 session restore、重复消息去重、outbound cursor 推进。
- qqbot ingress 现已改成：`message.ingest -> conversation resolve/restore -> session binding -> runtime inference -> session messages delivery`；正常 reply 不再直接依赖 handler 返回文本，而是从 session `conversation/messages.json` 读取新增 assistant/system 消息并发送。
- 后台 activity loop 现先扫描 conversations 做 pending outbound delivery，再做 activity-card heartbeat/diff；因此 reminder / queued follow-up / 后续 system notice 也能通过同一条 session-truth 通道自动外发。
- built-in qqbot bridge 已从“单 child 挂在 web_debug”升级为 attached supervisor：runner 进程退出后会产出 `bridge_process_exited / bridge_restart_scheduled` 并按固定 backoff 自动重启。当前仍是 attached keepalive，不是最终 detached dual-daemon；但已补最小 crash-restart 能力。
- debug 观察新增：`GET /api/qqbot_conversations.json`。定位 qqbot“有对话但无上下文/无自动回复”时，先查 conversations registry，再查 session messages，再查 bridge events。

## 2026-04-19 qqbot pairing default changed to persistent

- 用户确认：channel peer 的默认 pairing 过期没有意义，只会制造“agent 像死了”的假故障。
- 已改为：`/qqbot pair` 默认持久绑定（`session_expires_at=null`, `session_ttl_minutes=null`）；只有显式传 TTL 才会做限时绑定，显式 `/qqbot expire` 仍保留。
- 已实测 live 4040：`/qqbot pair` 返回 `expires_at=persistent`，并且 `~/.fin/runtime/peers/qqbot/state.json` 已落成 `session_expires_at=null`。

## 2026-04-19 qqbot replay cursor + attachment ingress + stable activity signature

- conversation 首次绑定到已有 session 时，delivery cursor 现在会初始化到该 session 当前最后一条可发送的 assistant/system message；这样后续只会发“新产生的回复”，不会把旧历史整段补发。
- qqbot 附件现在走白名单摘要链路：`channel ingress attachments -> ChatSendRequest.attachments -> DemoRequest.attachment_summaries -> ContextAssemblyInput -> current_input.attachments -> model input assembler`。当前是 metadata 进入上下文，不做图片下载/视觉解析。
- 已做真实闭环验证：本地 `/api/chat/send` 传入附件 `demo-proof.png` 后，模型直接回复 `demo-proof.png`，并且 `~/.fin/runtime/current/current_context.json` 可见结构化 `attachments`。
- activity card diff 签名已改成忽略 `updated_at` / `generated_at` 这类易变字段，避免纯文字 channel 每 5 秒把同一张卡重复当成 diff；当前无变化时只剩 60s heartbeat。

## 2026-04-20 pending queue attachment persistence + qqbot runner EPIPE hardening

- pending input 现在不再只存 `message`，而是持久化 `source + attachments`；因此当 channel 消息在 `running/paused/waiting_external` 阶段被排队后，后续 `/tick` / `/resume-run` 驱动时，可以把原始 channel 来源与附件 metadata 一起恢复回 `current_input`。
- 已补回归：scheduler driver 会把 queued input 的 `source/attachments` 传给下一轮推理；`paused session` 场景会把 `channel_ingress` 的附件写入 `queue/pending_inputs.json`。
- qqbot builtin runner 已加 `stdout EPIPE / ERR_STREAM_DESTROYED` 防护：检测到 stdio 断裂时直接置 `stopping=true`、清理连接并 `exit(0)`，避免 Node 因未处理 `process.stdout` error 崩成 noisy crash。
- 当前证据层级：队列恢复已由 Rust 测试覆盖；runner 防护已确认写入生成的 `~/.fin/runtime/peers/qqbot/bin/qqbot-peer-runner.mjs`，但尚未做一次专门的 pipe-break live 注入验证。

## 2026-04-20 qqbot activity heartbeat spam closeout

- 真源确认：重复刷屏不是多进程，也不是 signature 抖动；是 `channel.peer.activity_card_send_requested(reason=heartbeat)` 在 `ready/idle` 无变化时仍每 60s 投递。
- 修复口径：
  - heartbeat 只允许 `running/paused`；
  - `pairing_required / binding_mismatch / no active session` 时 `prepare_periodic_delivery` 直接静默，并清空 `activity_delivery_state.target/session_id`；
  - activity card 的 peer stage 不再把 `pairing_required` 渲染成旧的 `bound to session ...`。
- 验证：
  - Rust tests：`cargo test -p fin-cli channel_peer --manifest-path rust/Cargo.toml --quiet`，`cargo test -p fin-runtime activity_cards --manifest-path rust/Cargo.toml --quiet`
  - live：重启 4040 后，`~/.fin/runtime/peers/qqbot/activity_delivery_state.json` 已变为 `target=null, session_id=null`，并且 events tail 不再新增 `reason=heartbeat`。

## 2026-04-20 qqbot text card attention pass

- 用户反馈：qqbot 文字卡“没有注意力”，不利于扫读当前焦点。
- 本轮调整只改 compact text render，不改 runtime truth：
  - 顶部改为 `🌐 Global status`
  - 第二行直接显示当前 focus source 标题
  - 第三行显示高注意力状态摘要（`🔄/⏳/✅/❌`）
  - `sources:` / `stage:` / `detail:` / `source:` 改为 `👥 / 📍 / ⏳/❌ / 🧩`
  - 默认不再把 session/task/focus id 这类低价值标识堆到第一屏
- 验证：`cargo test -p fin-cli channel_peer_activity_delivery --manifest-path rust/Cargo.toml --quiet`

## 2026-04-20 qqbot text card semantic action pass

- 继续把文字卡从“内部状态串”往“人类可扫读进度卡”收敛：
  - `phase=inference_completed next_step=...` 映射为自然语义（如“本轮推理完成，正在整理结果 / 准备继续下一步”）
  - `bound to session ... / pairing required / binding invalidated` 映射为中文状态
  - 最近动作按语义渲染：`搜索 / 查看 / 修改 / 计划 / 命令 / 模型 / 推理`
- 作用：qqbot 卡片现在更像 finger 的“当前在做什么”提示，而不是把 provider/tool 内部字段直接甩给用户。
- 验证：
  - `cargo test -p fin-cli channel_peer_activity_delivery --manifest-path rust/Cargo.toml --quiet`
  - `cargo build -p fin-cli --manifest-path rust/Cargo.toml`

## 2026-04-20 qqbot text card checklist pass

- 继续强化“注意力”：
  - focus source 下方新增最近动作 checklist，前缀固定 `✅`
  - source 行只保留“谁在做什么”，不再把动作细节塞进同一行
  - `provider.call` 会优先提取 prompt 前半段，避免把 `输入 → 输出` 整段丢给用户
- 当前卡片结构更接近：
  - 标题 / 焦点 / 状态
  - 活跃源
  - 当前阶段
  - focus source
  - 最近动作 checklist

## 2026-04-20 qqbot attachment-only ingress fix

- 真源：图片消息已进入 `channel.peer.message_ingested`，但因 `content_preview=""` 且 `attachment_count=1`，随后被 `channel.peer.message_rejected(reason=empty_text_payload)` 直接拒绝，所以没有进入推理。
- 修复：qqbot inbound 现在对“空文本 + 有附件”不再 reject，而是框架生成一条附件说明型 fallback message 进入正常推理链；“空文本 + 无附件”仍然拒绝。
- 验证：
  - `cargo test -p fin-cli channel_peer_qqbot_bridge --manifest-path rust/Cargo.toml --quiet`
  - `cargo build -p fin-cli --manifest-path rust/Cargo.toml`

## 2026-04-20 qqbot text-channel sanitize + progress restore

- 用户指出：文字通道回复里仍带 `<fin_user_response>` 标签和 `**markdown**` 噪音，并且没有 progress update。
- 真源：
  - outbound 发送时直接使用 session 原始 assistant content，未做 text-channel sanitize；
  - qqbot peer 处于 `pairing_required/session_valid=false` 时，activity delivery loop 不会继续发 progress cards。
- 修复：
  - `deliver_pending_messages_for_target` 发送前统一做 text-channel sanitize：去掉 `fin_*` 标签块、`**/__/\`` 等 markdown 强调噪音；
  - inbound 恢复到已有 session 后，自动把 qqbot peer pairing 恢复为 `bound`，让 activity delivery 恢复工作。
- 验证：
  - `cargo test -p fin-cli channel_peer_qqbot_bridge --manifest-path rust/Cargo.toml --quiet`
  - `cargo build -p fin-cli --manifest-path rust/Cargo.toml`
## 2026-04-20 qqbot inbound ack + no-silent-failure

- 框架规则补齐：qqbot ingress 一旦完成去重判定，就先发一条用户可见回执“已收到，正在处理。”，不能等模型跑完才首条可见反馈。
- 所有已进入 ingress 的异常/拒绝路径必须用户可见：未绑定会话、空 payload、处理异常、以及“本轮没有新可发送回复”都要显式回复，不能只记 event。
- 诊断增强：runner 现在会把每个 gateway dispatch 的 `eventType/messageId/timestamp` 打到 stderr，并对未处理事件名显式记录，便于定位“connected 但没 ingress”的真源。

## 2026-04-20 qqbot pairing semantic correction

- 用户指出真问题：当前实现把“上游 bot 已登录/已鉴权”和“当前 session 绑定”混成一个 `pairing_required` 状态，导致 session mismatch/expire 后看起来像要重新配对/重新鉴权。
- 修正后口径：
  - `connectivity_state + upstream_authenticated_at` 表示 bot 是否已登录服务器；
  - `binding_state + session_valid` 只表示当前是否绑定到活动 session。
- 当前实现中，session mismatch/expire 只会释放到 `binding_state=unbound`，不会再打回 `pairing_required`；已登录 peer 等下一条真实 inbound 时可基于 conversations 自动恢复 session 绑定。

## 2026-04-20 progress semantic cleanup

- 用户纠正：文字卡 / progress 不应把用户上两轮提示词、provider base URL 这种内部输入细节当成“模型进度”展示。
- 修正后规则：
  - `provider.call` 的语义展示只保留模型标识（例如 `ali-coding-plan.qwen3.6-plus`），不显示 endpoint/base URL；
  - progress recent actions 优先显示真实工具调用；如果存在非 provider 工具，不再让 `provider.call` 占据 recent items；
  - provider 类动作只作为“模型已调用/已返回”的弱提示，不再回显 prompt 文本。

## 2026-04-20 tool catalog parity fix

- 用户指出模型报告的可用工具列表不完整；真源确认是 `runtime::tool_catalog` 漏掉了若干工具，而不是模型自己漏报。
- 当前已补回的可调用 model tools：
  - `update_plan`
  - `session.list`
- 当前已显式暴露但标为 disabled/planned 的工具族：
  - `apply_patch`
  - `view_image`
  - `context_history.rebuild`
  - `project.task.status / project.task.list`
- 固定规则：
  - tool catalog 必须与 runtime dispatcher 保持一致；
  - 不能让“文档/记忆里存在但 runtime catalog 不可见”的工具静默消失；
  - 未接线工具应进入 disabled/planned 认知面，而不是伪装成不存在。

## 2026-04-20 apply_patch tool enabled

- `runtime::tool_catalog` 现在把 `apply_patch` 提升为可调用 model tool，不再只停留在 disabled/planned；tool catalog 与 dispatcher 真源重新对齐。
- `apply_patch` 当前按 Hermes 思路支持两种模式：
  - `mode=replace`：`path + old_string + new_string + replace_all?`
  - `mode=patch`：V4A patch 文本（`*** Begin Patch` ...）
- runtime 会把 patch 成功结果写成 `ToolExecutionRecord + tool.apply_patch_completed`，并在有 `runtime_home` 时落 `runtime/tools/patch_receipts/*.json`，供 Web/QQ/debug 统一消费。
- patch 写入被限制在当前 `project.cwd / project_root` scope 内；相对路径没有 workspace scope 时直接失败，避免模型越界写盘。
- 验证：
  - `cargo test -p fin-runtime tool_dispatch --manifest-path rust/Cargo.toml --quiet`
  - `cargo test -p fin-runtime context_view --manifest-path rust/Cargo.toml --quiet`
  - `cargo build -p fin-cli --manifest-path rust/Cargo.toml --quiet`

## 2026-04-20 tool prompt + query tools closure

- prompt 层已强化 `apply_patch` 使用规则：模型现在明确被告知“有界单点编辑优先用 replace 模式，只有多文件/增删改移动才用 patch 模式”，并且工具列表渲染不再只显示工具名摘要，而是带 `use/avoid/input/output/example`。
- runtime 现已补齐并接线的查询/辅助 model tools：
  - `view_image`
  - `context_history.rebuild`
  - `project.task.status`
  - `project.task.list`
- `view_image` 当前是真实可调用但边界诚实：只返回附件/本地图片的引用元数据（path/url/size/dimensions），不伪装成像素级 vision 推理。
- `context_history.rebuild` 当前做的是 framework-owned rebuild bookkeeping：基于 `current_context.json + recent_contexts/digests/reasoning/tools` 刷新 session/runtime 的 rebuild-index，而不是让模型手工压缩历史。
- `project.task.status/list` 现在直接读 session truth（routing/execution_state/plan/messages）给任务列表与状态，不再让模型靠记忆猜 task 状态。

## 2026-04-20 prompt contract closure rule

- tool prompt contract 现在明确要求：`when_to_use / when_not_to_use / input / output / example`
  必须进入最终 `rendered_model_input`，不能只保留在结构化 context 里给 Web 看。
- `apply_patch` 的调用策略也固定为 prompt contract 的一部分，而不是松散经验：
  - 单点精确编辑默认 `mode=replace`
  - 多文件 / add / delete / move 才 `mode=patch`
- 这条规则用 runtime tests 固定，避免后续又退回“工具只有名字和一句简介，模型不会用”的状态。

## 2026-04-20 runtime multi-round tool loop closure

- 单次 closure 内原先虽然存在 auto tool loop，但每轮 provider 请求没有使用“重建后的 round context + 动态 tool catalog + 已执行工具结果”，本质上只是拿一段 follow-up 字符串继续问模型；现在已修成真正的每轮 round context rebuild。
- 当前 round context 重建规则：
  - 根据本轮前累计 `ToolExecutionRecord` 重建 `history.recent_tool_activity`
  - 把上一轮 assistant 回复与工具 artifact 合入 continuity / knowledge 视图
  - 重新生成 `current_input`
  - 重新生成动态 tool catalog（按 round/runtime_home/project scope/peer scope/exec session 状态调整 `use/avoid/policy`）
- follow-up input 现在只注入**已执行工具结果**，不再把 `provider.call` 混进“工具结果”；并明确告诉模型这些是 authoritative client facts。
- round truth 修正：第 2 轮及之后的 `RoundRecord / tool_dispatch step` 不再吃累计 dispatch state，而是只记录当轮 dispatch 结果；累计 stop/yield/reminder 只留给 closure 级聚合。
- 已补测试证明：
  - 第二轮 provider request 的 `rendered_input` 里能看到执行后的 tool result 注入
  - dynamic tool catalog 会根据 runtime_home / exec session / peer capability 状态变化

## 2026-04-20 system/project agent same-runtime rule

- `system agent` 与 `project agent` 当前正式冻结为：**同一套 runtime / operation-event / session truth / tool dispatch 基础设施**，区别不在基础设施分叉，而在 `role prompt + dynamic tool policy + workflow emphasis`。
- prompt role 真源现在只保留两类：
  - `system`
  - `project`
- 历史 `default` 只保留为向后兼容 alias，并映射到 `project`；`worker/reviewer` 不再是独立 role。
- role-aware dynamic tool policy 已进入 runtime 真源：
  - `system`：优先 orchestration / peer visibility / coordination / health
  - `project`：优先 project-scoped closure / docs-code-test-debug，并在同一 role 内承担 execution / review / handoff 模式
- 已补测试证明：
  - 同一 `SystemConfig` 可直接启动 `system` 与 `project` 两种 `WorkerRuntime`
  - `default` 兼容请求会被解析到 `project`
  - 二者共享 provider/runtime 基础设施，但 role id 与 tool policy 不同
- 2026-04-20 纠偏：只有 `system` 和 `project` 两类角色；`project agent` 需要多人执行时，直接 spawn 多个 worker runtime。worker 是执行体，不是角色。
- 2026-04-20 新增本地 worker skeleton：project 侧可以直接对 `target_worker_id` 做 `agent.assign` 和 `mailbox.send`，worker 侧用 `worker_id` 做 `mailbox.poll`；框架内部统一映射到 `local-<worker_id>` peer id。
- 2026-04-20 `ContextViewBuilder` 已开始消费 `runtime/peers/state/*.json`：ensured local worker peers 会回流到 `context.peer`，所以多 worker 不再只是底层落盘，也进入后续推理/观察视图。
- 2026-04-20 agent naming 最小真源已接入：
  - `agent_id = <device_name>.<agent_name>`
  - `device_name` 优先取 `user.toml -> runtime.device_name`，否则退回系统默认名
  - 本地自动命名走 `~/.fin/runtime/agents/name_pool.json`
  - 分配结果写入 `runtime/agents/registry.json` 与 `runtime/current/current_agent_registry.json`
  - 当前 `create_named_local_worker(...)` 已接到 CLI demo 和 `/compact` 的本地 worker 创建路径
- 2026-04-20 status probe 已开始直读 agent registry truth：
  - `status` 现在会优先读 `runtime/current/current_agent_registry.json`
  - 其次回退 `runtime/agents/registry.json`
  - 目的是让 Web / QQ / CLI 看到统一的 `device_name.agent_name`，不再只暴露匿名 worker_id


## 2026-04-20 system agent identity / boundary discussion snapshot

### A. Agent vs model vs user boundary correction
- 之前把 `system agent` review 错误地往 `model overlay / provider family` 方向拉了，这条路已经判定为错误。
- 当前冻结的新边界：
  - 对用户：用户面对的永远是 `Agent`，不是模型，不应该感知到底层 provider/model。
  - 对模型：模型只接收 framework 赋予的 `role + request + context + tools`，不应认为自己是某个模型，也不应认为自己正在“直接和用户聊天”。
  - 对系统：`provider/model` 只属于 backend/runtime adapter/debug truth，不属于 agent identity truth。
- 因此 prompt system 后续必须改成 `Agent-first`，而不是 `Model-first`。
- `system/project role` 是 agent 身份真源；provider/model 名称、family overlay、transport quirks 不得进入 agent 自我认知层。

### B. System agent role re-clarification
- `system agent` 是整个 fin 系统的大脑、指挥家、协调者、leader。
- 它是：
  - 唯一用户入口 frontstage
  - 用户与任务网络之间的协调层
  - 多 task / 多 agent / 多 peer 的统一编排者
  - 任务目标整理者、owner 分配者、计划维护者、状态汇总者、统一汇报者
- 它不是：
  - 长时间做具体执行的 worker
  - 长时间沉入单个 project 细节的 executor
  - 直接和用户裸聊的“模型”
  - 直接替代 project agent 的实现者

### C. System agent responsibilities (current discussion draft)
- 接收用户指令、变更、优先级调整、状态询问、中断/恢复请求。
- 整理用户目标，判断：
  - 是否延续当前 task/topic/session
  - 是否需要新 task / revive 旧 task
  - 是否影响其他并行任务
- 把用户目标编译成系统内 task language：objective / owner / priority / next action。
- 把工作分派给：
  - 本地或远端 project agent
  - capability peer
  - 其他 peer/worker
- 统一收集异步反馈：progress / update_plan / note / result / failure / timeout / waiting / health。
- 在任务之间穿梭协调，最终统一向用户汇报。

### D. System vs project role split (discussion consensus)
- `system agent` 负责：
  - why / what / who / when
  - task ownership
  - routing / delegation / recovery / coordination
  - overall user-facing reporting
- `project agent` 负责：
  - how
  - project-scoped exploration / implementation / verification / delivery
- 统一原则：
  - `system = control plane first`
  - `project = execution plane first`

### E. System agent direct-execution budget
- `system agent` 允许做小范围直接执行。
- 允许条件：
  - 简单任务
  - 一个 closure（一次完整推理闭环）大概率就能完成
  - 不需要长等待 / 长探索 / 项目级持续执行
- 允许的中间态：
  - 可以做 1~2 次 bounded probing/self-execution 作为快速探测
- 当前讨论冻结的硬预算：
  - 若连续 2~3 个 closure 之后仍然看不到明显收口，就不应继续自己做
  - 必须升级为：`形成目标 -> 建计划 -> 指定 owner -> 委派`
- 若当前没有明确执行路径：
  - `system agent` 可以 spawn 一个 `project role worker/runtime` 去探索或执行
  - 不新增新的 prompt role；仍然只有 `system` 与 `project` 两类 role

### F. Important modeling correction: closure != task
- Jason 的意图是要把 `system agent` 的直接执行控制得很紧，这一点保留。
- 但系统建模上当前倾向保留区分：
  - `closure` = 一次完整推理闭环（输入 -> 推理/工具 -> 停止）
  - `task` = 更高层的目标线程，可跨多个 closure，并且后续可转 delegated path
- 因此：
  - `system agent` 的直接执行预算按 `closure` 控制
  - `task` 不等于一次 closure

### G. Current non-final but important wording direction
- prompt / runtime 语义里不应把模型表述成“你在和用户直接聊天”。
- 更接近的框架语义应是：
  - current request
  - frontstage request
  - routed work item
  - interaction ledger
- `system agent` 即使自己处理简单任务，也必须保持控制面身份：
  - 这是“为了减少调度成本而亲自处理一个低复杂度小任务”
  - 不是退化成长期 executor

### H. Next discussion items after this note snapshot
- 继续讨论：`system agent` 如何判断“继续自做 / 升级委派”的具体触发信号。
- 候选信号包括：
  - closure 次数
  - tool loop 深度
  - wait/reminder
  - plan emergence
  - owner clarity
  - need for project-scoped context
  - need for parallel subtask split
  - health/risk escalation
- 等这些讨论完成后，再统一提炼成正式 architecture / prompt working doc，一次性修改真源与实现。


## 2026-04-20 startup topology / project registry / presence 落盘

### A. Startup topology 进入 system-only config
- 当前已把 startup topology 落到 `runtime.startup`：
  - `system_agent.local_worker_budget`
  - `system_agent.auto_resume`
  - `project_agents[]`
- `project_agents[]` 当前最小字段：
  - `project_id`
  - `mode=local|remote`
  - `project_root?`
  - `endpoint?`
  - `agent_name?`
  - `worker_budget`
  - `always_on`
  - `auto_resume`
  - `auto_connect`
- 这是 system-only 配置，不属于 user.toml。

### B. Effective system config 规则补齐
- 之前 CLI 一直只从 user.toml 动态 map system config，导致 `~/.fin/config/system.toml` 就算生成了也不会真的生效。
- 当前已补 effective system config 读取：
  - 先用 `user.toml` 映射 baseline
  - 再读取 `~/.fin/config/system.toml`
  - 保留 user-owned 字段（provider/default_provider/runtime.device_name）
  - 其余 system-only 字段继续从 system.toml 生效
- 这样 startup topology 才不是“写得出来但永远不生效”的假配置。

### C. Project registry / wake queue 真源
- 当前 framework 已落以下路径：
  - `~/.fin/runtime/projects/registry.json`
  - `~/.fin/runtime/projects/state/<project_id>.json`
  - `~/.fin/runtime/projects/wake_queue.json`
  - `~/.fin/runtime/current/current_startup_topology.json`
- 语义：
  - registry：当前已注册 project agent 列表与派生状态
  - state：单 project 的最新摘要
  - wake_queue：framework 生成的唤醒 intent

### D. Wake policy（当前最小版）
- `always_on=true` 且当前 presence 不是 `busy/idle/waiting`：
  - framework 直接生成 `always_on_startup`
- 若某 project 存在 unfinished work 且 agent 当前不在线：
  - framework 生成 `unfinished_work_detected`
- 这对应 Jason 已确认的“recovery-first，不要立刻重置任务”。

### E. Agent presence 真源
- 当前 framework 已落：
  - `~/.fin/runtime/agents/state/<agent_id>.json`
  - `~/.fin/runtime/current/current_agent_presence.json`
- system entry agent：
  - Web/debug 启动时先 seed 为 ready/idle
  - 收到请求时标记 `busy`
  - 成功/失败后写回 `idle`
- startup config 中声明的 project agent：
  - 当前先 seed 为 `offline + await_startup_wake`
  - 后续等 daemon/supervisor 真连接后，在同一 truth 上更新，不再造第二套 presence

### F. 当前阶段边界
- 已完成的是 framework-owned skeleton：
  - startup config
  - project registry
  - wake queue
  - agent presence
- 继续推进后，当前还多了一步真实执行：
  - framework 会执行 wake queue
  - local project agent 会被推到 `idle/project_ready`
  - remote project agent 会被推到 `waiting/await_remote_connect`
  - 同时把 managed project peer 写入 `runtime/peers/state + runtime/peers/registry`
- 还没完成的：
  - detached daemon 真正拉起 project agent
  - remote reconnect / lease / supervisor takeover
- 也就是说，当前阶段先把“应该唤醒谁、谁在线、谁离线、谁在忙”变成可观测事实，再接自治恢复。

## 2026-04-20 project runtime auto-pickup / auto-resume 补口

### A. 已补 framework-owned auto-resume seed
- 之前 local project runtime 在 handoff=`prepared|noop` 且 task 已 claimed 时，如果 queue 为空，会停在：
  - `pickup_state=claimed_idle`
  - `next_action=await_manual_work`
- 当前已在 `project_runtime_resume` 补 framework-owned seed：
  - 对 `claimed_idle + await_manual_work + project.auto_resume=true` 的 local project runtime，
  - framework 自动注入一个 synthetic pending input：
    - `input_kind=framework_resume`
    - `source=project.resume`
    - `message=continue work`
    - `enqueue_reason=resume`
- 然后重新 materialize pickup，再由 scheduler/supervisor 正常推进下一轮。

### B. 归属与边界
- 没把 side effect 塞进 `project_runtime_pickup` 的 snapshot materialization。
- 仍保持：
  - `pickup` 负责观测快照
  - `project_runtime_resume` 负责控制动作
- 这样不会把“读状态”变成“隐式推进状态”的双语义函数。

### C. 新增验证
- 新增测试：
  - `project_runtime_resume_tests::drive_ready_project_runtime_resumes_seeds_claimed_idle_project_queue`
- 验证内容：
  - 初始 queue 为空
  - handoff 已 prepared
  - task 已 claimed
  - framework 自动 seed pending input
  - scheduler 成功 drive 一轮
  - queue 最终被 drain 回空

### D. 当前证据
- `cargo test -p fin-cli --manifest-path rust/Cargo.toml project_runtime_resume --quiet` ✅
- `cargo test -p fin-cli --manifest-path rust/Cargo.toml attached_control_plane --quiet` ✅
- `cargo fmt --all --manifest-path rust/Cargo.toml --check` ✅

### E. 后续纠正
- 上面这 2 个失败后来已经定位并修复，不再视为“未知既有失败”：
  - 一部分是真正的 credentials precedence bug
  - 另一部分是 QQ 相关测试并行修改全局 env/HOME 导致的测试污染

## 2026-04-20 qqbot credentials precedence fix

### A. 根因
- `resolve_qqbot_credentials(...)` 之前是：
  - 先 `resolve_from_env()`
  - 再 `resolve_from_user_toml(...)`
- 这会导致：
  - 调用方已经显式传了 `user.toml` 路径，
  - 但只要进程环境里残留 `FIN_QQBOT_CLIENT_SECRET/QQBOT_CLIENT_SECRET`，
  - 解析就会被全局 env 抢走。
- 结果：
  - 显式配置文件不是唯一真源，
  - 测试与真实 bridge/connectivity 行为都会受外部环境污染。

### B. 修正后的规则
- 若调用方显式传入存在的 `user.toml` 路径：
  - **优先 user.toml**
  - user.toml 内若使用 `*_env` 字段，再按文件里的 env 引用读取
  - 若文件里没有 qqbot credentials，再回退到全局 env
- 若调用方没有显式传路径：
  - 仍保持 env first，再 fallback 到默认 `~/.fin/config/user.toml`

### C. 新增测试
- `explicit_user_toml_wins_over_global_env_credentials`
- 固定住：
  - 全局 env 存在时
  - 显式 `user.toml` 仍必须赢

### D. 当前证据（已更新）
- `cargo test -p fin-cli --manifest-path rust/Cargo.toml channel_peer_connectivity --quiet` ✅
- `cargo test -p fin-cli --manifest-path rust/Cargo.toml --quiet` ✅ `97 passed`
- `cargo test -p fin-runtime --manifest-path rust/Cargo.toml --quiet` ✅
- `cargo test -p fin-config --manifest-path rust/Cargo.toml --quiet` ✅
- `cargo fmt --all --manifest-path rust/Cargo.toml --check` ✅

### E. 额外修正：env test pollution
- 除了 precedence bug，本轮还发现 QQ 相关测试在并行修改：
  - `HOME`
  - `FIN_QQBOT_*`
  - `QQBOT_*`
- 原先 `channel_peer_tests` 与 `channel_peer_connectivity` 各自持有不同的 env lock，无法跨模块串行。
- 当前已补统一 `test_env::env_lock()` 真源，两个测试模块共享同一把锁，避免：
  - 默认 `~/.fin/config/user.toml` 抢进来
  - 某个测试的 env 残留影响另一个测试
  - poison 连锁导致误判

## 2026-04-20 startup -> supervision -> handoff -> pickup -> auto-resume E2E evidence

### A. 新增端到端测试
- 新增测试：
  - `startup_wakeup::tests::refresh_builds_resume_chain_and_auto_resume_can_drive_claimed_idle_project`
- 这条测试不再手写 handoff/pickup 快照，而是从 framework 真链路生成：
  - session truth
  - startup refresh
  - project supervision
  - execution handoff
  - runtime pickup
  - project auto-resume drive

### B. 当前验证的链路
- 预置：
  - project session 存在
  - `context.current_context.project.primary_project.project_id = fin`
  - execution state 标明 task 未完成（`pending_input_count=1`）
  - queue 为空
  - task registry 中 task=`ready`
- framework 执行后验证：
  - `current_project_supervision.json` => `resume_ready`
  - `current_project_execution_handoffs.json` => `prepared`
  - `current_project_runtime_pickups.json` => `claimed_idle`
  - `drive_ready_project_runtime_resumes(...)` 自动注入 synthetic resume input
  - scheduler 成功 drive 一轮并 drain queue

### C. 这条测试修正了一个真源陷阱
- 之前失败的根因不是 runtime 逻辑，而是测试数据错误：
  - `ExecutionStateRecord` / `PendingInputRecord` 的 `refs` 是 `flatten`
  - 测试若写成嵌套 `"refs": {...}`，`task_id/session_id` 实际不会进入真源
- 已按真实 contract 改成顶层字段书写。

### D. 当前证据（最新）
- `cargo test -p fin-cli --manifest-path rust/Cargo.toml refresh_builds_resume_chain_and_auto_resume_can_drive_claimed_idle_project --quiet` ✅
- `cargo test -p fin-cli --manifest-path rust/Cargo.toml --quiet` ✅ `98 passed`
- `cargo test -p fin-runtime --manifest-path rust/Cargo.toml --quiet` ✅
- `cargo test -p fin-config --manifest-path rust/Cargo.toml --quiet` ✅
- `cargo fmt --all --manifest-path rust/Cargo.toml --check` ✅

## 2026-04-20 collaboration context truth expansion

### A. 当前识别出的缺口
- system/project 的 `ProjectContextBlock` 之前已经有：
  - `task_board_summary`
  - `agent_presence_summary`
  - `project_supervision_summary`
- 但 owner-loop 真协调还缺两类 framework truth：
  - assignment queue
  - mailbox backlog
- 没有这两类摘要，system/project 在做 dispatch / follow-up / review / unblock 时只能看到 task board，却看不到：
  - 已派出去但未被消费的 assignment
  - 已投递但未被 worker 消费的 mailbox 消息

### B. 本轮补齐
- `ProjectContextBlock` 新增：
  - `assignment_queue_summary`
  - `mailbox_summary`
- `ContextViewBuilder -> build_project_block(...)` 现在会从 runtime_home 读取：
  - `runtime/assignments/pending.json`
  - `runtime/mailbox/*/inbox.json`
- 并把它们作为 project/system 推理前可见的 framework truth 注入 context。

### C. 当前语义
- `assignment_queue_summary`
  - 例如：`pending_assignments=2 [worker-b<-worker-system:pending, worker-c<-worker-system:pending]`
- `mailbox_summary`
  - 例如：`mailbox_messages=2 [local-worker-b:2]`
- 这让 system/project 的 owner-loop 在不额外调用工具前，就能先看到协作积压面。

### D. 新增验证
- 扩展测试：
  - `context_view_registry_tests::system_context_view_loads_active_and_registered_projects_from_runtime_registry`
- 当前固定验证：
  - active projects / registered projects
  - presence summary
  - supervision summary
  - assignment queue summary
  - mailbox summary

### E. 当前证据
- `cargo test -p fin-runtime --manifest-path rust/Cargo.toml context_view_registry_tests --quiet` ✅
- `cargo test -p fin-runtime --manifest-path rust/Cargo.toml context_view_tests --quiet` ✅
- `cargo test -p fin-runtime --manifest-path rust/Cargo.toml --quiet` ✅
- `cargo fmt --all --manifest-path rust/Cargo.toml --check` ✅

## 2026-04-20 richer testing + real provider smoke
- Added richer regression around combined `task_board + assignment_queue + mailbox_summary` context assembly to prevent collaboration backlog truth from regressing when active task view is present.
- Added `fin provider-live-smoke <user.toml> [transcript.json]` and `scripts/run-real-provider-smoke.sh` for isolated live provider verification under `~/.fin/harness/runs/<run-id>/...`.
- Live smoke now verifies multi-turn session truth + current projection/current provider artifacts and records `control_feedback_origin` plus `reasoning_stop_present` in receipt.
- Real provider evidence: `~/.fin/harness/runs/test-live-provider-20260420-2155/provider-live-smoke-report.json` with `provider=ali-coding-plan`, `model=qwen3.6-plus`, `control_feedback_origin=model_output_contract_v1`, `reasoning_stop_present=true`, `turn_count=3`.

## 2026-04-20 qqbot ingress E2E truth
- Added real repo-level qqbot E2E around `channel_peer_qqbot_bridge::process_inbound_message(...)` instead of more smoke.
- Verified true chain: `message.ingest -> conversation/session restore -> runtime inference -> session truth -> outbound emit`.
- New coverage proves two critical closures:
  - fresh inbound target gets `ack + final reply`, session truth persists answer, peer events and provider request artifacts are written.
  - existing target binding restores the old session even when built-in qqbot active pairing has moved to a newer session.
- Test files split to respect the `<500 lines` rule:
  - `rust/crates/cli/src/channel_peer_qqbot_bridge_tests.rs`
  - `rust/crates/cli/src/channel_peer_qqbot_bridge_e2e_tests.rs`
- Verification: `cargo test -p fin-cli channel_peer_qqbot_bridge -- --nocapture` ✅

## 2026-04-20 qqbot live receipt command
- Added `fin qqbot-live-receipt <user.toml> <qqbot-target> [run-id]`.
- Purpose: after a real QQ channel message has been processed, collect the current target/session truth into `~/.fin/harness/runs/<run-id>/qqbot-live-receipt.json`.
- Receipt currently verifies and records:
  - bound conversation/session identity
  - latest inbound / latest delivered cursor
  - ack notice presence
  - session-visible reply presence
  - provider request/response artifact presence
  - peer event count and reply preview
- Validation: `cargo test -p fin-cli` ✅

## 2026-04-20 real qqbot live receipt evidence
- Real target discovered from `~/.fin/runtime/channels/qqbot/conversations.json`:
  - `qqbot:c2c:F6A6F19355D0D62EEC06277EB445B51F`
- Executed:
  - `cargo run -p fin-cli -- qqbot-live-receipt ~/.fin/config/user.toml qqbot:c2c:F6A6F19355D0D62EEC06277EB445B51F qqbot-live-receipt-20260420-real`
- Generated receipt:
  - `~/.fin/harness/runs/qqbot-live-receipt-20260420-real/qqbot-live-receipt.json`
- Verified fields:
  - `status=passed`
  - `ack_notice_present=true`
  - `session_reply_present=true`
  - `provider_request_present=true`
  - `provider_response_present=true`
  - `session_id=session-test-install-0-1-0001`
  - `task_id=task-test-install-0-1-0001`
  - `latest_reply_preview=OK`
- Closeout meaning:
  - qqbot minimal real channel closure is now evidenced not only by repo E2E but also by real runtime receipt.

## 2026-04-20 m1 final closeout report
- Added final closeout report:
  - `docs/closeout/m1-final-closeout-report-2026-04-20.md`
- Final judgement now frozen as:
  - M1 complete
  - qqbot no longer a blocker
  - next phase should enter M2 through debt reduction + always-on lifecycle strengthening, not random feature expansion.

## 2026-04-20 m2 step1 line-limit debt closeout
- Completed the first M2 debt-reduction pass for the 500-line gate.
- Split oversized runtime/cli files into owning-layer slices without changing behavior:
  - runtime: `control_plane`, `agent_naming`, `activity_cards`, `context_view_tests`, `prompt_tests`, `tests`, `tool_dispatch_tests`
  - cli: `channel_peer`, `agent_presence`, `channel_peer_activity_delivery`, `provider_live_smoke`, `startup_wakeup`
- New helper slices keep the same truth boundaries: store/render/test/report/state helpers moved out; public entrypoints stayed in original owning modules.
- Verification:
  - `cargo test -p fin-runtime -p fin-cli --manifest-path rust/Cargo.toml` ✅
  - `python3 scripts/check-code-line-limit.py` ✅ (`code line-limit ok`)
- Result:
  - repo-wide non-whitelist code files are now back under the 500-line gate.
- Follow-up cleanup after line-limit split:
  - exported pending runtime naming APIs through `fin_runtime::lib` to remove dead-code warnings while preserving planned control-plane surface
  - removed leftover duplicate imports and dead local warnings from QQ activity delivery slices
- Verification refresh:
  - `cargo test -p fin-runtime -p fin-cli --manifest-path rust/Cargo.toml` ✅ (no warning lines emitted)
  - `python3 scripts/check-code-line-limit.py` ✅

## 2026-04-20 m2 step2 project runtime pickup/control truth hardening
- Closed one M2 Step 2 control-plane gap on the `supervision -> handoff -> pickup -> status probe` chain.
- Root issue:
  - `project_runtime_pickup` previously reused `claimed_idle + await_manual_work` for two different facts:
    1. framework may still seed the first resume for a claimed project task
    2. the task was already handed off to the same worker and runtime is simply idle waiting for new project input
  - This made pickup/status surfaces too weak and created a path for repeated resume interpretation / truth drift.
- Code changes:
  - Added `handoff_idle + await_new_project_input` pickup classification for already-handed-off idle project runtimes (`noop` handoff, or post-handoff idle observed by pickup classifier).
  - Exposed `read_project_runtime_resume_report(...)` and wired `current_project_runtime_resume.json` into `status_probe`, so status now shows both:
    - current pickup surface
    - latest runtime-resume execution summary
  - Split `project_runtime_pickup` tests into `project_runtime_pickup_tests.rs` to keep the 500-line gate green.
- Verification:
  - `cargo fmt --all --manifest-path rust/Cargo.toml` ✅
  - `cargo test -p fin-runtime -p fin-cli --manifest-path rust/Cargo.toml` ✅
  - `python3 scripts/check-code-line-limit.py` ✅
- Frozen boundary after this pass:
  - `pickup` is the current resumability surface
  - `runtime_resume_report` is the latest executed resume action surface
  - `status_probe` must show both rather than forcing one surface to speak for the other

## 2026-04-20 m2 step2 supervisor heartbeat effective-stale truth hardening
- Closed another M2 Step 2 control-plane gap on the `supervisor_cycle -> supervisor_heartbeat -> daemon_state/status` chain.
- Root issue:
  - `SupervisorHeartbeatRecord.stale_lease` was previously computed from the observed pre-refresh cycle.
  - If heartbeat then triggered a fresh supervisor cycle successfully, the heartbeat record could still say `stale_detected / stale_lease=true`.
  - That leaked stale pre-refresh observation into daemon/status/web as if it were still the final effective state.
- Code changes:
  - `due_for_tick` remains an observation on the pre-refresh cycle.
  - `stale_lease` is now recomputed from the final effective cycle after any triggered refresh.
  - `status=triggered_cycle` is preserved when heartbeat successfully drove a refresh and the final cycle is no longer stale.
  - The old-cycle stale fact is still preserved via `supervisor.stale_cycle_detected` event and `result_summary` diagnostic text.
- Verification:
  - `cargo fmt --all --manifest-path rust/Cargo.toml` ✅
  - `cargo test -p fin-runtime -p fin-cli --manifest-path rust/Cargo.toml` ✅
  - `python3 scripts/check-code-line-limit.py` ✅
- Frozen boundary after this pass:
  - `heartbeat.due_for_tick` = observed control need before refresh
  - `heartbeat.stale_lease` = final effective stale state after refresh
  - stale pre-refresh evidence belongs to event stream, not daemon/status final truth

## 2026-04-20 fin-5.1 resumable pause/resume checkpoint closeout
- Closed the current `fin-5.1` implementation pass around true resumable pause/resume by replacing the old `resume_as_new_closure` style continuation with a runtime-owned execution checkpoint chain.
- Frozen supported resume boundary for this pass:
  - checkpoint is recorded at framework-owned resume anchors (`wait.remind` waiting_external and tool-followup resume boundary)
  - scheduler prefers `resume_checkpoint` before generic pending queue replay
  - consumed checkpoint emits durable `execution.checkpoint_consumed`
  - synthetic framework resume input stays in step/provider/debug truth but does **not** pollute user-facing conversation/messages truth
- New durable truth added:
  - `ExecutionCheckpointRecord`
  - `ExecutionStateRecord.resume_checkpoint_ready/resume_checkpoint_id`
  - latest checkpoint artifacts under runtime current/session control paths
- Verification completed:
  - `cargo fmt --all --manifest-path rust/Cargo.toml` ✅
  - `python3 scripts/check-code-line-limit.py` ✅
  - `cargo test -p fin-runtime -p fin-cli --manifest-path rust/Cargo.toml` ✅
  - exact E2E: `web_debug::web_debug_tests::web_debug_tests_runtime::web_debug_tests_runtime_followups::due_reminder_prefers_execution_checkpoint_resume_without_fake_user_message` ✅
  - exact E2E: `channel_peer_qqbot_bridge::e2e_tests::qqbot_inbound_message_runs_end_to_end_and_emits_reply_from_session_truth` ✅
- Gate result:
  - `rust/crates/runtime/src/closure_runtime.rs` is now exactly 500 lines and passes the line-limit gate.
- Scope note:
  - this pass freezes checkpoint-based precise recovery at framework-owned boundaries; provider mid-flight stack restore is still out of scope and should not be implied.

## 2026-04-21 fin-5.2 headless daemon closeout
- Closed the current `fin-5.2` pass by adding a framework-owned headless daemon entry for single-agent always-on supervision.
- New CLI entrypoints:
  - `fin start <user.toml>`
  - `fin stop <user.toml>`
  - internal `fin daemon-run <user.toml>`
- Frozen minimal lifecycle truth for this pass:
  - pid file: `runtime/pids/headless-daemon.pid`
  - lease file: `runtime/leases/headless-daemon.json`
  - daemon state: `runtime/current/current_daemon_state.json`
  - daemon recovery action: `runtime/current/current_daemon_recovery_action.json`
  - stop request file: `runtime/locks/headless-daemon.stop`
- Execution boundary frozen for this pass:
  - headless daemon discovers sessions-with-work from session truth
  - it directly drives the framework supervisor cycle (not UI handlers pretending to tick)
  - due reminders + execution checkpoints can continue without frontstage/web request
  - role dispatch still reuses the same runtime: system entry sessions use entry role, project sessions use project role inferred from context truth
- Verification completed:
  - `cargo fmt --all --manifest-path rust/Cargo.toml` ✅
  - `python3 scripts/check-code-line-limit.py` ✅
  - `cargo test -p fin-cli -p fin-runtime --manifest-path rust/Cargo.toml` ✅
  - exact E2E: `headless_daemon_tests::headless_daemon_cycle_resumes_checkpoint_without_frontstage` ✅
  - exact E2E: `channel_peer_qqbot_bridge::e2e_tests::qqbot_inbound_message_runs_end_to_end_and_emits_reply_from_session_truth` ✅
- Scope note:
  - this pass delivers minimal detached/headless single-agent continuity with framework-owned lease/state/recovery truth; it does not yet implement multi-process supervisor election or external service manager integration.
