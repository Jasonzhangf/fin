---
name: fin-runtime-debug
description: Runtime/event debugging workflow for fin. Use for multi-agent, cross-process, and cross-network diagnosis.
---

# fin Runtime Debug Skill

## 1) Intent

用于定位多 agent 运行时问题，尤其是：
- 任务卡住
- worker 掉线
- heartbeat 丢失
- transport 中断
- checkpoint / resume 失效

## 2) Canonical sources

1. `docs/architecture/05-event-model-and-observability.md`
2. `docs/architecture/06-web-debug-console.md`
3. `docs/architecture/07-harness-replay-fault-injection.md`
4. `docs/architecture/14-runtime-home-layout.md`
5. `docs/architecture/15-install-build-regression-flow.md`
6. `docs/architecture/16-operation-and-event-model.md`
7. `docs/architecture/17-debug-method-and-observability-workflow.md`

## 3) Debug order

1. 先定位 `~/.fin` 下的 owning 证据目录（`sessions/`、`workdirs/`、`runtime/`、`diagnostics/`）
2. 再看 raw event
3. 再看 timeline / trace
4. 再看 session / task / dispatch 关联
5. 再看 transport 层
6. 最后才怀疑 Web 投影层


## 3.1) Module-level debug design baseline（路由到全局 skill）

模块级 debug 通用方法统一遵循全局 `coding-principals` 中的：

- Operation + Event + Projection
- Module Debug Baseline
- Evidence-first Delivery

本地只保留 `fin` 的调试补充：

- 先定位 `~/.fin` 下的 owning 证据目录
- 优先检查 session / workdir / runtime 三层证据是否一致
- 修复 runtime 问题默认要补 replay 或等价回放
- 先检查 snapshot 写入是否 bounded（latest overwrite + recent window），避免 debug 本身制造资源问题
- debug 服务默认以前台命令运行；定位问题时不要通过后台悬挂进程维持状态
- live `4040` 若行为与源码不一致，先判 stale binary / stale process；重编译后精确重启当前 PID，再继续怀疑 HTTP/provider 路径
- provider 请求失败时，先看结构化 reqwest 诊断字段（stage/attempt/endpoint/timeout/connect/request/body/decode/source-chain），不要只凭一句 `request failed` 下结论
- live provider run 若被外层短 wall-clock timeout 截断，先判定为 harness timeout 设计错误；不要把“wrapper 杀进程”误诊成 provider/tool/runtime 失败
- 若 live provider E2E 中模型确实调了工具、tool records 也完整，但综合内容仍泛化，优先查 `sessions/.../provider/recent_provider_requests.json` 的 rendered input：先确认 follow-up round 是否真正看到了 executed tool 的 stdout/receipt/artifact refs，而不是只看到了粗粒度 `Recent tool activity`
- Jason 已明确收紧该条：`current history` 不允许用 summary/recent 假真相替代真实工具结果；若 rendered input 里只有 activity 摘要、没有 authoritative receipt/full stdout/full patch arguments，就直接判 runtime context assembly 有缺口
- 若 framework 在正常自然语言输入后自行触发了 formalize/routing/dispatch/prompt_user/system_notice，第一步必须追 trigger source；**合法来源只能是用户刚性命令或模型已解析成功的 control-block 指标**。若源头是 wrapper 预判、置信度阈值、聊天文本关键词或其它 heuristic，直接判为 framework boundary violation
- 若用户看到“框架判断……/请 `/formalize` ……/系统提示你做选择”这类可见插话，先判定为 framework 泄漏；除显式本地命令回执或 transport 层必要 notice 外，framework 不应在正常用户↔agent 对话中主动发声
- 若主业务 `qqbot-live-receipt` 失败但证据显示 `latest_activity_card_preview` 来自**daemon 重启前**的最后一次 inbound，而当前 daemon/runner/binding 已在修复后二次拉起，先判为**historical receipt residue**；下一步先做隔离 simulated inbound + receipt gate 验证新链路，再决定是否需要等待新的真实 ingress
- 若 `attached_control_plane` / `headless_daemon` 相关老测试失败，但 `project_runtime_resume.summary` 明确是 `explicit_trigger_required=*` 且 pickup 仍为 `ready_to_resume`，先判为**test expectation drift**：当前真相是 framework 只 materialize resume truth，不自动驱动 project closure
- WebUI 新字段若“后端有文件但页面没显示”，先查 `/app.js` 是否已经包含对应消费路径关键字，再判前端逻辑问题；include_str/bundled JS 不刷新时，页面会继续跑旧 bundle
- 推理核心若怀疑“没走工具 / 没多步闭环”，先看 `model.output_parsed.stop_kind + tool.dispatch_* + operation.completed/failed`；`fin_tool_calls` 被解析但没有 `tool.dispatch_completed`，问题一定在 runtime dispatch，不在 Web
- 未实现工具必须走 failed closure（`tool.dispatch_failed` + `operation.failed`），禁止把模型的工具请求静默改写成直接答复，否则 session truth 会丢真正的失败因果
- peer-aware 阶段先看 `peer.discovered / binding.opened / daemon.state_observed / peer.routing_feedback_recorded`；若只有 placeholder 事件，说明当前仍在 local-only skeleton，不要误判成 remote peer 真连接
- channel gateway（qqbot）生命周期问题优先看 `~/.fin/runtime/peers/qqbot/state.json + events.jsonl`：若出现 `channel.peer.session_expired`，下一步必须看到 `channel.peer.pairing_required`，否则说明重配闭环断裂
- 若当前 active session 已切换（如 `/new` / `/resume`）但 qqbot 仍绑定旧 session，框架必须产出 `channel.peer.session_invalidated` + `channel.peer.pairing_required`；这类 mismatch 不应继续伪装成有效 paired 状态
- 若怀疑“project agent 没被叫醒/always_on 没生效”，先查 `~/.fin/runtime/projects/registry.json + wake_queue.json + state/<project_id>.json`；先确认 framework 是否已经生成 wake intent，再怀疑 daemon/spawn
- 若需要判断 project agent 当前应“恢复 / 接续 / 观望 / 等远端”，优先查 `~/.fin/runtime/current/current_project_supervision.json`；presence 只说明当前忙闲，supervision 才说明 framework 当前已 materialize 的 trigger 面
- 若怀疑 system agent 的 context 里为什么只看到当前 cwd 一个项目，先查 `~/.fin/runtime/projects/registry.json`；当前 `ProjectContextBlock.active_projects/projects` 对 system role 应优先来自 registry，而不是只靠 cwd / project_label 推一个假单项目视角
- 若怀疑 system agent 的 context 没带上“谁在忙 / 当前 supervision 要做什么”，先查 `~/.fin/runtime/current/current_agent_presence_registry.json` 与 `~/.fin/runtime/current/current_project_supervision.json`；当前 `ProjectContextBlock` 应直接带 `active_agent_ids / agent_presence_summary / supervision_actions / project_supervision_summary`
- 若 supervision 已显示 `resume_candidate_present`，再查 `~/.fin/runtime/current/current_project_execution_handoffs.json`；若没有 `prepared/noop/missing_task` handoff 记录，就不要误报“框架已把 task 接续下去”
- 若 handoff 已是 `prepared/noop`，再查 `~/.fin/runtime/current/current_project_runtime_pickups.json`；它才说明 local project runtime 当前是 `ready_to_resume / prepared_idle / running / waiting_external / paused / missing_binding`，不要只凭 handoff 就断言已经在跑
- 若 pickup 已是 `ready_to_resume + scheduler_tick_needed`，再查 `~/.fin/runtime/projects/runtime_resume_reports.json`；这里能确认 framework 是否真的触发了 `resume seed -> supervisor cycle -> scheduler tick`，不要把“pickup 可继续”误报成“已经实际继续执行”
- `prepared_idle/await_manual_work` 只允许表示“handoff 只是预演/准备好了，但还没有显式 resume trigger”；若这里已经把 task 真改成 `claimed/submitted`，优先检查是不是 framework 在 startup/headless 路径偷做了执行性变更
- 当 `current_project_runtime_pickups.json` 与实时推进感受不一致时，必须同时对照 `current_project_runtime_resume.json`；pickup 代表当前待处理面，resume report 代表最近一次 framework 已执行过的恢复动作，二者不能互相冒充
- `latest_heartbeat.stale_lease` 必须解释为“heartbeat 完成后当前有效 cycle 是否仍 stale”，不能拿它冒充“本次 heartbeat 观察前曾见过 stale”；如果旧 cycle 确实 stale 但已被新 cycle 刷新，这个事实只留在 `supervisor.stale_cycle_detected` 事件里
- 若怀疑这些推进动作是不是又被写回某个 UI handler，先查入口是否统一走 `attached_control_plane::run_attached_control_plane_cycle(...)`；heartbeat / reminder / startup / project_resume / daemon_state 若重新散落在入口内联，说明 control-plane 真相退化回了 handler 耦合
- 重启/启动后的“谁已启动、谁 busy、当前资源预算”必须先查 `~/.fin/runtime/current/current_startup_control_summary.json`；不要让 QQ/Web/status 各自从 presence/project files 临时拼第二套重启摘要
- 若 project runtime 实际继续执行后前台 session/context 看起来被串台，先查 `runtime/current/last_run.json` 是否被 project run 偷改；当前设计要求 project continuation 保护 frontstage current 视图，真实 project 结果只看对应 session artifacts
- 若怀疑“daemon 说要 project_recovery_trigger_present 但没真正执行”，再查 `~/.fin/runtime/projects/recovery_reports.json + runtime/current/current_project_recovery.json`；先确认 recovery skeleton 是否已跑，再查 detached daemon / spawn 缺口
- 若怀疑“system/project agent 当前到底在忙什么”，先查 `~/.fin/runtime/agents/state/<agent_id>.json`；presence 是 framework 真源，不要只盯着 activity card 或 status 文案
- 若需要看“当前所有 agent 的并发 busy/idle/waiting 概览”，优先查 `~/.fin/runtime/agents/presence_registry.json` 或 `runtime/current/current_agent_presence_registry.json`，不要再从 naming registry 反推忙闲
- 若 system agent 在多轮中需要主动复查 framework-owned control state，优先让它调用 `agent.presence.list` 与 `project.supervision.list`；context 里的 summary 只是首轮装配，后续巡检应回到 runtime truth 工具
- 若 qqbot “有收到消息但没有自动继续回复/恢复上下文”，先查 `~/.fin/runtime/channels/qqbot/conversations.json`：确认 `target -> session_id` 是否存在、`last_inbound_message_id` 是否重复、`last_delivered_message_id` 是否推进；然后再查 session `conversation/messages.json`
- 若 qqbot 首次绑定已有 session 后出现“旧回复被整段补发”，先查 conversations 里的 delivery cursor 是否已初始化到当前 session 末尾；正常行为应只发送绑定后的新增 assistant/system 消息
- 文字 channel 卡片问题先看 `source card view -> user card view -> channel delivery record` 三层：若总卡/源卡摘要不一致，先查 framework card builder；若内容一致但发出文本不对，再查 channel adapter 的 diff / 紧凑重绘逻辑
- 若纯文字 channel 每 5 秒重复发送同一张 activity card，先比对 `activity_delivery_state.last_user_signature` 与当前 card signature；优先排查是否把 `updated_at/generated_at` 这类易变字段错误纳入 diff signature
- progress / 状态卡若用户感觉“没意义”，先判是不是把**无变化 snapshot**当成了**进度更新**；文字 channel 必须优先做 `meaningful delta gate`，heartbeat 只能说明 `phase + duration + waiting_reason + last_meaningful_change`，不能重复刷“已收到，正在处理”
- 若 daemon 重启后 session 仍卡 `status=running`，先查 `control/execution_lease.json`：`running` 只有在 lease pid 仍活着时才算真运行；lease 缺失或 pid 已死就是 **orphaned running**，必须先恢复成 `idle + resume_checkpoint_ready`（若有 open checkpoint）或清成 idle failed，不能继续 `wait_running` 空转
- 若 qqbot 在 `ready/idle` 且无新进展时仍周期刷同一卡片，先查 `channel.peer.activity_card_send_requested.reason`；当前 heartbeat 只应允许 `running/paused` 活动期，`ready/idle` 与 `pairing_required/invalidated` 必须静默
- 若怀疑 Web / text channel 的工具摘要不一致，先查 `/api/activity_cards.json` 与 `tool_semantics`；只要这里一致，问题就不在 runtime truth，而在各自 renderer / delivery policy
- qqbot 文字卡当前 delivery record 真源在 `~/.fin/runtime/peers/qqbot/activity_delivery_state.json`；若出现“重复发 / 不发 / 发到旧目标”，先查这里的 `target/session_id/last_user_signature/last_delivery_at`，再查 bridge send event
- 若 renderer/unit 测试已绿，但 QQ 真机仍看到旧文案/旧命名，先确认 **live daemon PID 的启动时间与二进制路径**，再看 `activity_delivery_state.last_delivered_text` 与 `channel.peer.activity_card_send_requested.text_preview`；这类现象默认先判 `stale daemon / stale binary`，重编译并 scoped 重启当前 daemon 后再怀疑源码没生效
- 若文字 channel 出现 `heartbeat -> 5s 后又补一张几乎相同 diff 卡`，先查 delivery state 比较的是不是 **last delivered text** 而不是 **last compact text**；heartbeat 文本和 compact 文本不同，若拿前者做 diff 基准就会形成来回刷屏
- frontstage 对派发型任务必须消费 **delegated agent/session truth**：system 在 owner dispatch 之后即使自己 idle，也应转成“等待 worker 回报”；一旦 worker/project agent 有 running/waiting 进展，主 frontstage 必须把它们聚合成用户可见 recent/progress
- 若 peer 已进入 `pairing_required` / `binding_mismatch`，`activity_delivery_state` 必须被清空（至少 `target/session_id=null`）；否则旧 target 仍可能被错误复用
- 若用户声称“发了图片但 agent 说没看到”，先查 qqbot ingress `attachment_count`，再查 `runtime/current/current_context.json -> context.current_input.attachments`；当前 M1 只保证附件 metadata 进入上下文，不做真实图片下载与视觉解析
- 若怀疑“暂停/等待期间发来的图片消息恢复后丢了附件”，先查 session `queue/pending_inputs.json`：现在 queued input 真源必须包含 `source + attachments`，并在 `/tick` / `/resume-run` 后进入 `current_context.current_input`
- 若 qqbot bridge 自身异常退出，先查 `channel.peer.bridge_process_exited / bridge_restart_scheduled / bridge_supervisor_error`；当前最小保活真源是 attached bridge supervisor，不是 detached daemon
- 若怀疑“测试把主业务入口搞坏了”，先同时检查三处是否被测试 session 污染：`runtime/current/last_run.json`、`runtime/channels/qqbot/conversations.json`、`runtime/peers/qqbot/state.json`。只要它们落到 `test-*` / install / smoke session，就先判定为**test contamination**，不要先怪 provider/模型。
- 若用户反馈“system agent 根本起不来 / QQ 进不去主入口”，第一步不是看 presence 文件，而是确认四个真相是否同时成立：**live daemon PID 存活、canonical system session 已 materialize、QQ 默认 binding 指向该 system session、target conversation 没被旧测试 session 抢回**。
- `presence/worker_pool/startup_summary` 只能证明启动骨架存在，**不能**证明业务入口可用；没有 canonical system session + QQ 可达 binding，就不算“system agent 已启动可测”。
- 当前标准业务启动真相已固定：`start` / `web-debug` / `daemon-run` 必须 materialize canonical `session-system-entry`（或 `session-<entry_role>-entry`），并把 `runtime/current/current_startup_entry_session.json + last_run.json` 对齐到该 session；若 QQ 当前无有效绑定或仍绑测试 session，启动流程必须自动修回这个 canonical session。
- 若看到 `bridge.stderr.log` 中的 `write EPIPE`，先确认是否为旧日志；当前 runner 已对 `stdout EPIPE / ERR_STREAM_DESTROYED` 加防护，新的 pipe-break 应表现为安静退出 + supervisor 重拉，而不是 Node 未处理异常直接炸栈
- 文字 channel 若用户反馈“发了消息但没反应”，第一检查不是 provider，而是 ingress 用户可见回执链：进入去重后的 inbound 必须先看到一条 ack，其后 reject/error/no-output 也必须有用户可见 notice；只记 event 不算闭环
- 若 live run 失败后 run-root 只有 `provider-live-smoke.log` 起始行和极少数初始化文件，先把它归类为“partial truth 缺口”；下一步优先补 started / round / waiting-phase truth，而不是直接继续拍脑袋改 prompt
- 若怀疑“QQ 已 connected 但就是没有 ingress”，先查 `bridge.stderr.log` 里的 `dispatch / dispatch-unhandled` 事件名；先确认 gateway 实际投递了什么 `eventType`，再决定是 runner 漏处理还是上游根本没推送
- qqbot peer 的“上游登录态”和“session 绑定态”必须分开看：`connectivity_state/upstream_authenticated_at` 代表已登录；`binding_state/session_valid` 只代表当前是否绑定到活动 session。session mismatch/expire 只能释放绑定到 `unbound`，不能把已登录 peer 打回 `pairing_required`
- **标准业务启动验收不能只看 `runtime/peers/qqbot/state.json`**：这个文件可能残留旧的 `bridge_ready/connected`。必须同时看到**本轮启动后的 `channel.peer.bridge_spawned + bridge_start_requested` 事件**以及**当前存活的 `node ... qqbot-peer-runner.mjs` 进程**，才算 QQ ingress 真正被拉起。
- 若 QQ 文本上看起来“重复了一次”，先对照 `channel.peer.notice_send_requested(notice_kind=received_ack)` 与 `channel.peer.activity_card_send_requested`：前者是显式入站回执，后者是活动卡。若两者都只表达“已收到，正在处理”，应判为 **ack/activity 语义撞车**，修复策略是抑制首个 ack 等价 activity card，而不是误判为 bridge 重复发送同一包。
- 若 session `conversation/messages.json` 出现 `assistant.content=""`，第一步先查 `provider/recent_provider_responses.json`：若 `output_text=""` 但 `tool_calls!=[]`，这是 provider tool-only round，不是 parser 吃字；修复方向应是**禁止空 assistant 落盘/出站**，并在 prompt 中强制工具轮也给一条可见进度句。
- 若 QQ 侧出现 `no_new_messages` 或“像空转但没回复”，同时查 `pending_outbound_messages` 与 channel sanitize 后的文本：若消息原文只剩 control/tool tags，被清洗后为空，应推进 delivery cursor 但不要出站，更不要再补发 `no_new_messages` 噪声提示。
- 若用户在新的 inbound ack/waiting 卡片里仍看到上一轮的失败行，先查 `build_activity_cards()`：`pending_inbound_notice` 不仅要清 `system_card.failure_detail`，还要清 `user_card.failure_detail`；否则用户卡会继续把旧失败渲染到“已收到，正在处理”状态卡上。
- hidden framework/project 输入若意外出现在 `conversation/messages.json`，不要只查 runtime materializer；`fin` 当前还要同时检查 CLI demo/web wrapper 对 `run.conversation_user_input` 的二次覆盖，避免 runtime 已过滤、wrapper 又写回去形成双真源泄漏
- 若 heartbeat / checkpoint-resume / assignment-resume 明明是 hidden turn，却污染了 `conversation/messages.json` 或 recent digests/reasoning/tools/turn history，先查 `SessionMaterializer` 的 **hidden-turn persistence gate** 与 `last_run` 写入边界；当前规则是 hidden control-plane turn 只允许更新 current/control/checkpoint truth，不得冒充普通 session closure
- 若 framework event 明明已从 `append_framework_events(...)` 关闭，但 archive 仍持续增长，下一步不要只盯 framework event append；先查 `SessionMaterializer.persist()` 是否仍对 hidden/control-plane turn 无条件执行 `persist_event_stream(...)`，再核对 `source_visibility.rs` 是否把 `daemon_headless / daemon_headless_* / project.resume / framework.assignment_runtime` 这类 source 正确归类为 ephemeral
- 若 macOS 再次弹出“`fin-cli` 想访问文稿文件夹”，先查 live daemon 二进制路径；**不要再从 `~/Documents/.../rust/target/debug/fin-cli` 直接挂 daemon**，应先切到 `~/.fin/install/current/bin/fin` 再启动，否则会重复触发 TCC 文稿授权框
- 若想收口 `.fin` 资源占用，先按三类分：**(1) 必须持久化的 latest/current truth；(2) 只该保留 bounded recent 的 recent/debug 文件；(3) 不该进入 session history 的 hidden framework/control-plane turn**。不要把所有大文件一刀切删掉。
- 若代码已调小 retention 默认值，但 live 目录仍旧持续按旧窗口增长，先查 `load_effective_system_config(...)`：当前会保留已有 `~/.fin/config/system.toml` 覆盖；**只改 `RuntimeRetentionConfig::default()` 不会自动影响现网**。
- 若 `headless-daemon.log` 仍在增长，不要只看 daemon heartbeat；launchd `stdout/stderr` 也会打到同一路径。当前正确修法是让同一路径在后续 `append_log(...)` 时做 tail trim，这样下一次 heartbeat 就会把旧日志收回 bounded size。

## 4) Minimal validation

- 关键事件链必须包含 `trace_id`
- 关键运行链必须能定位到 `task_id` / `dispatch_id`
- 修复后至少做一次 replay 或等价回放验证
- 关键 side effect 必须能看到 started / completed / failed 三段事件

## 5) Anti-patterns

- 只看 Web 页面不看事件
- 看到 symptom 就直接改 UI
- 没有 replay 证据就宣称 runtime 已修复
- 只有日志，没有 operation/event/projection 对应关系

## 6) New distilled lessons

- 若 hidden checkpoint 已被 consume，但 `consumed_by_operation_id` 看起来还是前台旧 run，先查 `runtime/current/current_turn.json`；checkpoint attribution 真源必须优先读当前 hidden turn 的 `TurnRecord.operation_id`，不能继续只靠 `last_run.operation_id`。
- 验 hidden worker/assignment 路径时，至少同时看四处：`tasks/registry/*.json`、worker session `conversation/messages.json`、worker session `tools/recent_tool_records.json`、frontstage `runtime/current/last_run.json`；只有“task 推进了，但 worker history 仍空且 frontstage last_run 不变”才说明 hidden-turn 边界真闭合。
- 若 QQ/debug 卡片里工具摘要看起来像“没有更新”或“旧动作排在前面”，先查 renderer 是否按 `started_at/tool_call_id` 做了**最新优先排序**；不能假设 `tool_semantics` / `recent_actions` 输入顺序永远就是用户想看的顺序。
- 若 idle/worker_ready 的 agent presence 里仍挂着旧 `recent_actions`，先查 `agent_presence_store` 是否在空 `recent_actions` 写回时错误继承旧值；startup/worker-pool rewrite 必须以**当前记录为准**，不能把历史 tool failure 混回当前 ready 卡。
- 启动链若出现 framework 直接把 project/worker 从 `offline` 推到 `idle/resume_ready`，先查 `startup_wakeup` / `project_recovery` 是否越界执行了 wake queue；Jason 当前已冻结：**startup 只能 materialize trigger truth，真正是否拉起 worker 必须由 system agent 决定**。
- 若 attached/headless cycle 一看到 `assignment queue` 就直接跑 worker hidden turn，先判为 **framework 越权执行 assignment**；当前正确边界是：cycle 只落 `current_assignment_runtime_resume` 这类 observation/trigger truth，真正恢复必须由 system/owner agent 决定，worker 启动后先 self-check 再 report。
- `scheduler_tick / supervisor_cycle` 当前只允许做**同一 bound session**的刚性 continuation：`resume_checkpoint`、`pending_inputs`、`parallel_pending`，以及把 owner-loop 动作记录成 truth；若它开始跨 project 消费 wake/recovery/assignment truth 并直接替 system/worker 执行，就属于新的 boundary violation。
- 若 QQ/文字状态卡里的 `session_id / task_id / worker name` 被渲染成 `…` 或资源列表退化成 `+N`，先查 `channel_peer_activity_delivery_render*`；这属于 **renderer 信息损失**，不是 runtime truth 缺失。关键标识默认宁可多占一行，也不要裁到不可辨识。
- 若 renderer 已放宽但 live `stage` 仍然带 `…`，继续往上查 `runtime/src/activity_cards.rs`；`system_card.current_activity` / `user_card.stage` 可能在落盘前就被 `shorten(...)` 预裁了，不能只盯着文本 channel 层。
- 若 live WebDebug 已切到新 provider，但 continuation 仍落回旧 endpoint，先对照 `runtime/current/current_provider_requests.json` 与 `runtime/current/current_execution_lease.json`：这通常说明**旧 headless daemon 仍在接管 continuation**。必须把 daemon 用**同一份 `runtime_home/config/user.toml`** 重启到相同 provider，再继续验真正闭环；否则前台首轮成功、后台续轮失败会形成假阴性。
- 若真实 managed-project 验证里突然冒出多条 `task-untitled-task-call_*.json`，先查 `runtime/tools/tool_receipts/call_*.json`：这说明 `project.task.create` 已真实落盘，但 **title/summary 参数在 agent/prompt 链路丢失**。优先修 prompt/tool 调用约束，不要误判成 task registry 或 owner-loop 自己凭空造任务。
- 若 live provider 响应里连续出现 `tool_calls=[{\"arguments\":{}}]`（例如 `project.task.create` / `update_plan` / `write_file` 都变成空对象），先查 `closure_runtime_rounds_tools::build_provider_tool_specs`：仅有 `type=object + description` 的弱 schema 会让模型持续空参；必须补 `properties/required/anyOf` 的结构化输入 schema，再继续怪 prompt。
