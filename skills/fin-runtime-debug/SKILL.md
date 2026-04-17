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
