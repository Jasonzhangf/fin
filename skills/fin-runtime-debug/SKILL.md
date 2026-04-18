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
