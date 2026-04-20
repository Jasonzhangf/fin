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
- WebUI 新字段若“后端有文件但页面没显示”，先查 `/app.js` 是否已经包含对应消费路径关键字，再判前端逻辑问题；include_str/bundled JS 不刷新时，页面会继续跑旧 bundle
- 推理核心若怀疑“没走工具 / 没多步闭环”，先看 `model.output_parsed.stop_kind + tool.dispatch_* + operation.completed/failed`；`fin_tool_calls` 被解析但没有 `tool.dispatch_completed`，问题一定在 runtime dispatch，不在 Web
- 未实现工具必须走 failed closure（`tool.dispatch_failed` + `operation.failed`），禁止把模型的工具请求静默降级成直接答复，否则 session truth 会丢真正的失败因果
- peer-aware 阶段先看 `peer.discovered / binding.opened / daemon.state_observed / peer.routing_feedback_recorded`；若只有 placeholder 事件，说明当前仍在 local-only skeleton，不要误判成 remote peer 真连接
- channel gateway（qqbot）生命周期问题优先看 `~/.fin/runtime/peers/qqbot/state.json + events.jsonl`：若出现 `channel.peer.session_expired`，下一步必须看到 `channel.peer.pairing_required`，否则说明重配闭环断裂
- 若当前 active session 已切换（如 `/new` / `/resume`）但 qqbot 仍绑定旧 session，框架必须产出 `channel.peer.session_invalidated` + `channel.peer.pairing_required`；这类 mismatch 不应继续伪装成有效 paired 状态
- 若怀疑“project agent 没被叫醒/always_on 没生效”，先查 `~/.fin/runtime/projects/registry.json + wake_queue.json + state/<project_id>.json`；先确认 framework 是否已经生成 wake intent，再怀疑 daemon/spawn
- 若需要判断 project agent 当前应“恢复 / 接续 / 观望 / 等远端”，优先查 `~/.fin/runtime/current/current_project_supervision.json`；presence 只说明当前忙闲，supervision 才说明 framework 的下一步控制意图
- 若怀疑 system agent 的 context 里为什么只看到当前 cwd 一个项目，先查 `~/.fin/runtime/projects/registry.json`；当前 `ProjectContextBlock.active_projects/projects` 对 system role 应优先来自 registry，而不是只靠 cwd / project_label 推一个假单项目视角
- 若怀疑 system agent 的 context 没带上“谁在忙 / 当前 supervision 要做什么”，先查 `~/.fin/runtime/current/current_agent_presence_registry.json` 与 `~/.fin/runtime/current/current_project_supervision.json`；当前 `ProjectContextBlock` 应直接带 `active_agent_ids / agent_presence_summary / supervision_actions / project_supervision_summary`
- 若 supervision 已显示 `resume_project_task`，再查 `~/.fin/runtime/current/current_project_execution_handoffs.json`；若没有 `prepared/noop/missing_task` handoff 记录，就不要误报“框架已把 task 接续下去”
- 若 handoff 已是 `prepared/noop`，再查 `~/.fin/runtime/current/current_project_runtime_pickups.json`；它才说明 local project runtime 当前是 `ready_to_resume / claimed_idle / running / waiting_external / paused / missing_binding`，不要只凭 handoff 就断言已经在跑
- 若 pickup 已是 `ready_to_resume + scheduler_tick_needed`，再查 `~/.fin/runtime/projects/runtime_resume_reports.json`；这里能确认 framework 是否真的触发了 `resume seed -> supervisor cycle -> scheduler tick`，不要把“pickup 可继续”误报成“已经实际继续执行”
- `claimed_idle/await_manual_work` 只允许表示“尚未消化 handoff、允许 framework seed resume”；若 task 已被同一 worker handoff 过且当前只是空闲等待新输入，必须用独立 pickup 状态（如 `handoff_idle/await_new_project_input`），否则会误触发重复 resume 与状态漂移
- 当 `current_project_runtime_pickups.json` 与实时推进感受不一致时，必须同时对照 `current_project_runtime_resume.json`；pickup 代表当前待处理面，resume report 代表最近一次 framework 已执行过的恢复动作，二者不能互相冒充
- 若怀疑这些推进动作是不是又被写回某个 UI handler，先查入口是否统一走 `attached_control_plane::run_attached_control_plane_cycle(...)`；heartbeat / reminder / startup / project_resume / daemon_state 若重新散落在入口内联，说明 control-plane 真相退化回了 handler 耦合
- 重启/启动后的“谁已启动、谁 busy、当前资源预算”必须先查 `~/.fin/runtime/current/current_startup_control_summary.json`；不要让 QQ/Web/status 各自从 presence/project files 临时拼第二套重启摘要
- 若 project runtime 实际继续执行后前台 session/context 看起来被串台，先查 `runtime/current/last_run.json` 是否被 project run 偷改；当前设计要求 project continuation 保护 frontstage current 视图，真实 project 结果只看对应 session artifacts
- 若怀疑“daemon 说要 recover_project_agents 但没真正执行”，再查 `~/.fin/runtime/projects/recovery_reports.json + runtime/current/current_project_recovery.json`；先确认 recovery skeleton 是否已跑，再查 detached daemon / spawn 缺口
- 若怀疑“system/project agent 当前到底在忙什么”，先查 `~/.fin/runtime/agents/state/<agent_id>.json`；presence 是 framework 真源，不要只盯着 activity card 或 status 文案
- 若需要看“当前所有 agent 的并发 busy/idle/waiting 概览”，优先查 `~/.fin/runtime/agents/presence_registry.json` 或 `runtime/current/current_agent_presence_registry.json`，不要再从 naming registry 反推忙闲
- 若 system agent 在多轮中需要主动复查 framework-owned control state，优先让它调用 `agent.presence.list` 与 `project.supervision.list`；context 里的 summary 只是首轮装配，后续巡检应回到 runtime truth 工具
- 若 qqbot “有收到消息但没有自动继续回复/恢复上下文”，先查 `~/.fin/runtime/channels/qqbot/conversations.json`：确认 `target -> session_id` 是否存在、`last_inbound_message_id` 是否重复、`last_delivered_message_id` 是否推进；然后再查 session `conversation/messages.json`
- 若 qqbot 首次绑定已有 session 后出现“旧回复被整段补发”，先查 conversations 里的 delivery cursor 是否已初始化到当前 session 末尾；正常行为应只发送绑定后的新增 assistant/system 消息
- 文字 channel 卡片问题先看 `source card view -> user card view -> channel delivery record` 三层：若总卡/源卡摘要不一致，先查 framework card builder；若内容一致但发出文本不对，再查 channel adapter 的 diff / 紧凑重绘逻辑
- 若纯文字 channel 每 5 秒重复发送同一张 activity card，先比对 `activity_delivery_state.last_user_signature` 与当前 card signature；优先排查是否把 `updated_at/generated_at` 这类易变字段错误纳入 diff signature
- 若 qqbot 在 `ready/idle` 且无新进展时仍周期刷同一卡片，先查 `channel.peer.activity_card_send_requested.reason`；当前 heartbeat 只应允许 `running/paused` 活动期，`ready/idle` 与 `pairing_required/invalidated` 必须静默
- 若怀疑 Web / text channel 的工具摘要不一致，先查 `/api/activity_cards.json` 与 `tool_semantics`；只要这里一致，问题就不在 runtime truth，而在各自 renderer / delivery policy
- qqbot 文字卡当前 delivery record 真源在 `~/.fin/runtime/peers/qqbot/activity_delivery_state.json`；若出现“重复发 / 不发 / 发到旧目标”，先查这里的 `target/session_id/last_user_signature/last_delivery_at`，再查 bridge send event
- 若 peer 已进入 `pairing_required` / `binding_mismatch`，`activity_delivery_state` 必须被清空（至少 `target/session_id=null`）；否则旧 target 仍可能被错误复用
- 若用户声称“发了图片但 agent 说没看到”，先查 qqbot ingress `attachment_count`，再查 `runtime/current/current_context.json -> context.current_input.attachments`；当前 M1 只保证附件 metadata 进入上下文，不做真实图片下载与视觉解析
- 若怀疑“暂停/等待期间发来的图片消息恢复后丢了附件”，先查 session `queue/pending_inputs.json`：现在 queued input 真源必须包含 `source + attachments`，并在 `/tick` / `/resume-run` 后进入 `current_context.current_input`
- 若 qqbot bridge 自身异常退出，先查 `channel.peer.bridge_process_exited / bridge_restart_scheduled / bridge_supervisor_error`；当前最小保活真源是 attached bridge supervisor，不是 detached daemon
- 若看到 `bridge.stderr.log` 中的 `write EPIPE`，先确认是否为旧日志；当前 runner 已对 `stdout EPIPE / ERR_STREAM_DESTROYED` 加防护，新的 pipe-break 应表现为安静退出 + supervisor 重拉，而不是 Node 未处理异常直接炸栈
- 文字 channel 若用户反馈“发了消息但没反应”，第一检查不是 provider，而是 ingress 用户可见回执链：进入去重后的 inbound 必须先看到一条 ack，其后 reject/error/no-output 也必须有用户可见 notice；只记 event 不算闭环
- 若怀疑“QQ 已 connected 但就是没有 ingress”，先查 `bridge.stderr.log` 里的 `dispatch / dispatch-unhandled` 事件名；先确认 gateway 实际投递了什么 `eventType`，再决定是 runner 漏处理还是上游根本没推送
- qqbot peer 的“上游登录态”和“session 绑定态”必须分开看：`connectivity_state/upstream_authenticated_at` 代表已登录；`binding_state/session_valid` 只代表当前是否绑定到活动 session。session mismatch/expire 只能释放绑定到 `unbound`，不能把已登录 peer 打回 `pairing_required`

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
