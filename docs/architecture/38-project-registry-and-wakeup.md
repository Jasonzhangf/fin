# 38 Project Registry And Wakeup

本文档冻结 `fin` 当前的 **project registry / wakeup** 最小真源。

目标：

1. 让 startup topology 不再只存在于口头讨论
2. 让 framework 能在启动时知道有哪些 project agent、哪些需要被唤醒
3. 把 `always_on / unfinished work / wake queue` 固化为可落盘的控制面事实

---

## 1. 核心结论

`system agent` 是编排中心，但 **project 注册、唤醒判断、wake queue** 属于 framework-owned control plane。

冻结规则：

1. system agent 决定“做什么任务、谁来做”
2. framework 决定“哪个 project agent 已注册、当前是否在线、是否该被叫醒”
3. `always_on=true` 的 project agent 不等待 system agent 口头唤醒；framework 启动时直接派生 wake intent

---

## 2. Startup config contract

当前最小配置落点：

```toml
[runtime.startup.system_agent]
local_worker_budget = 4
auto_resume = true

[[runtime.startup.project_agents]]
project_id = "fin"
mode = "local"
project_root = "/Users/fanzhang/github/fin"
agent_name = "builder"
worker_budget = 2
always_on = true
auto_resume = true
auto_connect = true
```

补充说明：

- 以上数值的 baseline default 当前来自 repo 内：
  - `rust/crates/config/defaults/runtime-startup.toml`
- 用户运行时真正生效的 startup 配置来自：
  - `~/.fin/config/system.toml`
- 因此 `local_worker_budget / worker_budget` 的运行时真源是 `system.toml`，不是 Rust 字面量。

冻结字段语义：

- `system_agent.local_worker_budget`：system agent 可派生的本地 worker 预算
- `project_agents[].project_id`：一个 project agent 对应一个 project_id
- `mode=local|remote`
- `project_root`：local project 的权威根路径
- `endpoint`：remote project 的接入地址
- `agent_name`：稳定 agent 命名（最终形成 `<device_name>.<agent_name>`）
- `worker_budget`：该 project agent 之下可并行的 worker 预算
- `always_on`：framework 启动即确保唤醒
- `auto_resume`：存在 unfinished work 时，允许恢复优先
- `auto_connect`：remote peer 后续连接策略预留

---

## 3. Runtime artifacts

framework 当前冻结以下落盘路径：

- `~/.fin/runtime/projects/registry.json`
- `~/.fin/runtime/projects/state/<project_id>.json`
- `~/.fin/runtime/projects/wake_queue.json`
- `~/.fin/runtime/current/current_startup_topology.json`

它们共同回答：

1. 当前注册了哪些 project
2. project 对应哪个 agent_id
3. 当前 unfinished task 数量是多少
4. 当前 presence 是什么
5. 当前 wake_state / wake_reason 是什么

另外 framework 现在还会额外落：

- `~/.fin/runtime/current/current_startup_control_summary.json`

它用于统一回答“重启/启动完成后当前资源状态”：

1. 当前启动配置预算是什么（system workers / project workers / projects）
2. 当前哪些资源已经启动
3. 当前哪些资源处于 busy
4. 当前 waiting / recoverable_offline / wake_actions 概况

Web / QQ / status probe 必须读取这份 startup control summary，而不是各自推断。

---

## 4. Wake policy

### 4.1 Always-on

若：

```text
project.always_on = true
```

且当前 presence 不是 `busy / idle / waiting`，则 framework 直接生成：

```text
wake_reason = "always_on_startup"
```

并写入 wake queue。

### 4.2 Unfinished work

若：

- project agent 当前不在线
- 且该 project 下仍存在 unfinished task

则 framework 生成：

```text
wake_reason = "unfinished_work_detected"
```

这对应 Jason 已确认的恢复优先原则：先尝试叫醒/恢复，不是立刻重置任务。

---

## 5. Unfinished detection（当前最小版）

当前 M1.1 的 unfinished 判断来自 session/runtime truth：

- session `context/current_context.json` 中的 `project.primary_project.project_id`
- session `control/execution_state.json`

当前最小 unfinished 条件：

- `status in {running, paused, waiting_external}`
- 或 `pending_input_count > 0`

后续若 project task system 真正接入 claim/review/blocked board，再升级为 board-first truth。

---

## 6. 边界

当前 registry / wakeup 已进入 **framework executed skeleton**，但还不是最终 detached daemon：

- 已有：
  - 配置 → 注册 → unfinished 扫描 → wake queue 落盘
  - framework 执行 wake queue
  - 本地 project agent presence 从 `offline` 推到 `idle`
  - managed project peer 写入 `runtime/peers/state/*.json + runtime/peers/registry.json`
- 未有：
  - 真正独立进程 spawn
  - reconnect / lease supervisor
  - detached restart

因此当前阶段的 freeze 是：

```text
先让 startup control-plane truth 可执行、可观察，
再把 daemon / remote peer / auto-restart 接到这套真源上。
```

补充冻结（2026-04-20）：

- attached daemon 当前已具备最小 `recover_project_agents` 执行骨架
- 当 daemon state 派生出 `recover_project_agents` 时，framework 不再只记录 action，而会：
  1. 重新 materialize startup topology
  2. 过滤 recoverable offline projects
  3. 复用同一条 wake queue 执行链
  4. 刷新 registry / presence / startup summary
- 当前 recovery report 落盘：
  - `~/.fin/runtime/projects/recovery_reports.json`
  - `~/.fin/runtime/current/current_project_recovery.json`
- 这仍不是 detached supervisor，但已经从“仅观察”推进到“最小可执行恢复”

再补一层（2026-04-20）：

- framework 现在会额外 materialize `project supervision snapshot`
- 落盘：
  - `~/.fin/runtime/projects/supervision.json`
  - `~/.fin/runtime/projects/supervision/<project_id>.json`
  - `~/.fin/runtime/current/current_project_supervision.json`
- supervision 当前最小状态：
  - `ready`
  - `resume_ready`
  - `busy`
  - `waiting`
  - `recover_needed`
- 它的作用不是替代 presence，而是把 framework 当前对每个 project agent 的**下一步控制意图**显式化：
  - `observe_ready`
  - `resume_project_task`
  - `monitor_running_task`
  - `await_remote_connect`
  - `recover_project_agent`

再补一层（2026-04-20，execution handoff skeleton）：

- 对 `mode=local + desired_action=resume_project_task` 的 project，framework 现在会继续 materialize：
  - `~/.fin/runtime/projects/execution_handoffs.json`
  - `~/.fin/runtime/projects/execution_handoffs/<project_id>.json`
  - `~/.fin/runtime/current/current_project_execution_handoffs.json`
- 它不是 detached execution，也不是第二套 task board，而是把 supervision intent 落成最小可执行 handoff：
  1. 找到 `resume_task_id`
  2. 调用 runtime-owned `handoff_project_task(...)`
  3. 若 task 可接续，则把 task status 推到 `claimed`
  4. 记录 handoff state 为 `prepared | noop | missing_task`
- 当前冻结边界：
  - 已完成：framework 可把 “resume project task” 从控制意图推进到 task claim/handoff 真源
  - 未完成：真正 detached/local project runtime 自动捡起该 task 并持续执行

再补一层（2026-04-20，runtime pickup truth）：

- framework 现在会继续 materialize：
  - `~/.fin/runtime/projects/runtime_pickups.json`
  - `~/.fin/runtime/projects/runtime_pickups/<project_id>.json`
  - `~/.fin/runtime/current/current_project_runtime_pickups.json`
- 它读取 execution handoff + session execution state + pending queue，显式回答：
  - 该 project runtime 当前是 `running / waiting_external / paused / ready_to_resume / claimed_idle / missing_binding`
  - 下一步应做什么：`observe_running / await_external_event / resume_or_interrupt / scheduler_tick_needed / await_manual_work / repair_binding`
- 这是 **pickup truth**，不是 detached execution：
  - 已完成：framework 知道 local project runtime 是否已经具备继续推进条件
  - 未完成：framework 自动真正执行 provider/tool closure
