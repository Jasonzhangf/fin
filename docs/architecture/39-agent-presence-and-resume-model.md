# 39 Agent Presence And Resume Model

本文档冻结 `fin` 当前的 **agent presence / resume observation** 最小模型。

目标：

1. 让 framework 可见每个 agent 当前在干嘛
2. 让 system agent / Web / status probe 能读取统一的 presence truth
3. 把“agent 是否忙、在做哪个 task、是否需要恢复”从模型猜测变成框架事实

---

## 1. 核心结论

presence 不是 prompt 文本，也不是 UI 临时状态。

presence 属于 framework-owned control plane truth。

冻结原则：

1. agent 的忙闲、当前 task/session/operation 必须框架落盘
2. system agent 判断 worker/project agent 健康度时，优先读 presence truth
3. UI / channel / status probe 只消费 presence，不维护第二套忙闲状态
4. 不只保留“最后一个 presence”；framework 还要维护 presence registry，才能观察所有 agent 的并发状态

---

## 2. Runtime artifacts

当前最小落点：

- `~/.fin/runtime/agents/state/<agent_id>.json`
- `~/.fin/runtime/agents/presence_registry.json`
- `~/.fin/runtime/current/current_agent_presence.json`
- `~/.fin/runtime/current/current_agent_presence_registry.json`

其中 `<agent_id>` 统一遵循：

```text
<device_name>.<agent_name>
```

---

## 3. Presence fields（当前最小版）

当前 presence 至少包含：

- `agent_id`
- `agent_name`
- `device_name`
- `role_id`
- `agent_kind`
- `project_id?`
- `status`
- `current_task_id?`
- `current_operation_id?`
- `current_session_id?`
- `current_phase?`
- `is_reasoning`
- `updated_at`
- `last_heartbeat_at?`
- `progress_summary`
- `pending_input_count`
- `waiting_reason?`

附加配置面字段：

- `mode`
- `project_root?`
- `endpoint?`
- `always_on?`
- `auto_resume?`
- `worker_budget?`

---

## 4. Status taxonomy

当前最小 presence 状态冻结为：

- `idle`
- `busy`
- `waiting`
- `offline`

说明：

- `busy`：正在推理/执行
- `waiting`：后续可映射 wait/reminder/external wait
- `idle`：当前可接收新工作
- `offline`：配置存在，但当前未被唤起/未有 heartbeat

status summary / probe 若要展示“当前有哪些 agent、分别 busy 还是 idle”，应优先读 `presence_registry`，而不是从 agent naming registry 反推。

---

## 5. Entry agent update rule

system entry agent 现在采用以下最小规则：

1. Web/debug 启动时先确保 entry agent presence 存在
2. 收到真实请求后，进入 `busy`
3. run 失败，写回 `idle + error summary`
4. run 完成，写回 `idle + awaiting next request`

这保证：

- 用户入口 busy/idle 不再靠 UI 猜
- 后续 qqbot / status / dashboard 可直接读同一份事实

---

## 6. Configured project agents

当前对 startup config 中声明的 project agent，framework 会先 seed presence：

- role=`project`
- agent_kind=`project_agent`
- status=`offline`
- current_phase=`await_startup_wake`

初始 seed 之后，当前 wake execution 最小规则是：

- local project wake 成功后：`idle + project_ready`
- remote project wake 记录后：`waiting + await_remote_connect`

这代表：

```text
project agent 已注册，并且 framework 已把“该不该醒、当前醒到了哪一步”写成 presence truth。
```

后续若 detached daemon 接入，应在同一文件上更新，不另造第二套 presence。

补充冻结（2026-04-20）：

- wake request 现在允许携带 `resume_task_id`
- local project wake 若存在 unfinished task，会把 presence 推进到：
  - `status=idle`
  - `current_phase=resume_ready`
  - `current_task_id=<resume_task_id>`
- remote project wake 则保留：
  - `status=waiting`
  - `current_phase=await_remote_connect`
  - 但同样可带 `current_task_id=<resume_task_id>`

这意味着 framework 不只知道“agent 醒了”，还知道它**下一步应接续哪个 task**。

再补一层（2026-04-20，runtime pickup presence）：

- local project agent 的 presence 现在不再只停留在 `resume_ready`。
- framework 会结合 `execution_handoff + session execution_state + pending_inputs`，把 project agent 推进到更接近运行事实的状态：
  - `busy + reasoning`
  - `waiting + waiting_external`
  - `waiting + paused`
  - `idle + project_ready`
- 当前这一步仍是 framework observation/materialization，不等同于 detached runtime 已真实执行下一轮 closure。

再补一层（2026-04-20，ready_to_resume continuation bridge）：

- 当前前台请求链已具备最小的 **framework-owned project resume bridge**：
  - `current_project_runtime_pickups.json` 中若出现 `ready_to_resume + scheduler_tick_needed`
  - 前台框架会先为该 session seed `idle` execution state（仅当 state 缺失）
  - 然后驱动 `supervisor cycle -> scheduler tick -> run_next_pending`
- 对 project runtime，这条链必须使用 `project` role，而不是复用 frontstage `system` role。
- 为防止 project runtime 执行污染前台真源，当前实现会在 project run 前后保护 `runtime/current` 的 frontstage 视图；真正的 project truth 仍落在各自 session artifacts 与 project control-plane snapshots。
- 当前边界：
  - 已能把 `ready_to_resume` 从“观察态”推进到真实 continuation
  - 仍然依赖前台请求链提供 provider 执行上下文，尚未升级为 detached autonomous scheduler

再补一层（2026-04-20，attached control-plane wrapper）：

- 上述前台 continuation 现在不再散落在 `web_debug.rs` handler 中逐段手写。
- 当前冻结为统一 wrapper：
  - `attached_control_plane::run_attached_control_plane_cycle(...)`
- 它会在真正处理当前用户请求前，固定按顺序推进：
  1. `supervisor heartbeat`
  2. `due reminder inject`
  3. `startup control-plane refresh`
  4. `project runtime resume`
  5. `attached daemon state refresh`
- 这意味着：
  - attached 模式下的 continuation 已经从“某个 handler 顺手做一下”
  - 收口为 **framework-owned attached control-plane cycle**
  - 但依然不是 detached autonomous daemon

---

## 7. Resume / recovery relation

Jason 已冻结的恢复原则是：

> unfinished task + agent offline = recovery-first

presence 在这里的用途是：

1. 判断 agent 是否真的在线
2. 给 wake queue / supervisor 提供“需不需要恢复”的依据
3. 让 system agent 先读框架状态，再决定是否要发询问/巡检

当前最小实现边界：

- 已做：presence 文件 + startup wake intent
- 未做：真正 detached restart / remote reconnect / lease-based takeover

---

## 8. 边界

presence 当前是 **观察与控制输入**，不是最终自治调度器。

冻结顺序：

```text
先有 presence truth
-> 再接 wake queue
-> 再接 supervisor/daemon
-> 最后才做 autonomous recovery loop
```
