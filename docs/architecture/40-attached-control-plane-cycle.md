# 40 Attached Control Plane Cycle

本文档冻结 `fin` 当前前台模式下的 **attached control-plane cycle**。

目标：

1. 把前台请求到达前会发生的 framework control-plane 推进顺序写成真源
2. 防止 heartbeat / reminder / startup / project continuation 再次散落在 handler 内联
3. 明确当前 attached 模式的边界：已可执行，但还不是 detached autonomous daemon

---

## 1. 核心结论

当前前台模式下，`web_debug` / channel ingress 在真正进入用户 turn 之前，必须先跑一轮：

```text
attached control-plane cycle
```

冻结规则：

1. 这轮 cycle 属于 **framework-owned control plane**
2. handler 只负责调用统一 wrapper，不再手写顺序
3. project continuation 的 pickup / resume 也属于这轮 cycle，不属于 UI 或 prompt 层

当前统一入口：

- `rust/crates/cli/src/attached_control_plane.rs`

---

## 2. 最小执行顺序

当前 attached cycle 的顺序冻结为：

```text
incoming request
  -> supervisor heartbeat
  -> due reminder inject
  -> optional reminder-driven supervisor cycle
  -> startup control-plane refresh
  -> project runtime pickup/resume
  -> startup control-plane refresh
  -> attached daemon state refresh
  -> enter frontstage turn
```

对应当前最小实现：

1. `run_supervisor_heartbeat(...)`
2. `inject_due_reminders(...)`
3. 若有 due reminder：
   - `clear_waiting_if_due(...)`
   - `run_supervisor_cycle(..., "reminder_fired", ...)`
4. `refresh_startup_control_plane(...)`
5. `drive_ready_project_runtime_resumes(...)`
6. 再次 `refresh_startup_control_plane(...)`
7. `refresh_attached_daemon_state(...)`

---

## 3. 为什么要统一成 wrapper

之前这些动作是直接写在 `web_debug.rs` handler 里。

问题：

1. 顺序不稳定，后续任何入口都可能复制一遍
2. control-plane truth 容易退化成“某个 UI 路径才会跑”
3. project continuation 容易被误解成 frontstage turn 的副作用，而不是 framework 的固定责任

因此当前冻结：

```text
frontstage request handler
  -> call run_attached_control_plane_cycle(...)
  -> then classify/run current user request
```

不允许：

- Web handler 内再次散写 heartbeat / reminder / startup / resume 顺序
- channel gateway 自己绕过 wrapper 补一套 control-plane
- project continuation 只靠某个 UI 按钮或局部命令路径触发

---

## 4. 与 startup / supervision / pickup 的关系

attached cycle 本身不是新的真源，它只是把已有 control-plane 子真源按固定顺序串起来：

### 4.1 Startup topology / wakeup

负责：

- 读取 startup config
- 生成 project registry / wake queue
- materialize supervision / handoff / pickup 快照

### 4.2 Project runtime resume

负责把：

```text
ready_to_resume + scheduler_tick_needed
```

推进成真实 continuation：

- seed idle execution state（仅当缺失）
- supervisor cycle
- scheduler tick
- run_next_pending

### 4.3 Attached daemon state

负责把当前前台模式下能观察/执行的 daemon 恢复动作继续向前推进。

冻结结论：

```text
attached control-plane cycle
  = orchestration wrapper
  != second source of truth
```

---

## 5. 当前已完成的能力

当前 attached cycle 已具备：

1. supervisor heartbeat 推进
2. reminder 到期注入与 waiting 清理
3. startup wake / supervision / handoff / pickup materialization
4. local project runtime 的 ready-to-resume continuation
5. attached daemon state refresh

其中 project continuation 的关键要求：

- project turn 必须使用 `project` role
- 不能复用 frontstage `system` role
- 不能污染 `runtime/current` 的前台 current 视图

---

## 6. 当前边界

当前 attached cycle 仍然有明确边界：

1. 它依赖前台请求链提供 provider 执行上下文
2. 它不是 detached autonomous scheduler
3. 它不是真正跨进程 daemon IPC / lease supervisor
4. 它不会在完全无前台流量时自行长期推进

因此当前状态应理解为：

```text
framework-owned attached continuation
!= detached always-on autonomous supervisor
```

---

## 7. 调试与验收

调试 attached cycle 时，优先看：

- `~/.fin/runtime/current/current_supervisor_heartbeat.json`
- `~/.fin/runtime/current/current_startup_topology.json`
- `~/.fin/runtime/current/current_project_supervision.json`
- `~/.fin/runtime/current/current_project_execution_handoffs.json`
- `~/.fin/runtime/current/current_project_runtime_pickups.json`
- `~/.fin/runtime/projects/runtime_resume_reports.json`
- `~/.fin/runtime/current/current_daemon_state.json`

最小验收标准：

1. handler 代码只调用统一 wrapper，不再内联顺序
2. `ready_to_resume` project session 能在 attached cycle 中被真实推进
3. project continuation 不污染前台 `runtime/current` 视图
4. 有单测覆盖：
   - startup -> pickup -> resume 主链
5. `fin-cli` 相关测试集通过
