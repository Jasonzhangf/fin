# Agent Taxonomy Contract

本文档冻结 `fin` 当前的最小 agent taxonomy 真源。

目标：

1. 明确 **role** 与 **runtime 执行体** 的边界
2. 防止后续又把 `worker/reviewer` 扩成新的 prompt role
3. 给 runtime / supervisor / mailbox / assign 的后续实现提供稳定前提

---

## 1. 唯一 role 真源

`fin` 当前只允许两类 agent role：

1. `system`
2. `project`

约束：

- 不再新增 `worker` / `reviewer` / `analyzer` 等 prompt role
- `default` 只允许作为历史兼容 alias，解析后必须落到 `project`

## 1.1 Durable agent identity 真源

`role` 不是 `agent identity`。fin 的 agent identity 必须显式分为三类：

1. `system_agent`
   - 用户总入口 / 全局协调者。
   - 是 durable primary agent，可跨设备恢复会话。
   - 可发现、鉴权、绑定多个 `project_agent`。
   - 可 spawn 自己的 `subagent` 执行全局局部任务。
2. `project_agent`
   - 项目级 durable primary agent。
   - 不是 `system_agent` 的普通 subagent。
   - 可独立常驻、跨设备通信、鉴权后接收任务。
   - 拥有项目上下文、项目任务队列、项目记忆、项目工具权限。
   - 可 spawn 自己的 `subagent`。
3. `subagent`
   - 从 `system_agent` 或 `project_agent` 派生的临时/半持久执行单元。
   - 继承父 agent 的授权边界和上下文策略。
   - 用于并行探索、局部实现、review、测试、长任务分片。
   - 生命周期由父 agent 管理：spawn → running → waiting/completed/failed → closed/resumed。

关键边界：Codex 只可作为 `spawn/send/wait/close/resume/fork_context/mailbox seq` 等局部机制参考，不能作为 fin 的整体身份拓扑参考。fin 中 `system_agent` 与 `project_agent` 都是一等 durable primary identity。

### 1.1.1 Startup simplification

当前启动模型冻结为：

1. 默认总是启动本机 `system_agent`。
2. `system_agent` 使用标准 primary path：`system:<machine.system-agent-id>`，并在启动时注册到 runtime agent control truth。
3. `project_agent` 不再要求写死在 system config；system agent 可维护动态配置文件 `runtime/agents/project_agents.json`，支持增删 project agent。
4. `project_agent` 与 `system_agent` 的 primary 身份语义一致，区别只在 project binding：`cwd/project_root` 与 agent RPC `endpoint/port`。
5. 本地 project agent 端口可自动选择，但一旦选择必须持久化到动态配置文件，后续启动按配置恢复。
6. system agent 根据静态配置 + 动态配置 materialize 可用 project agent 列表，也可跨设备通过 Agent RPC 发现远端 primary agents。
7. 单个 primary agent 自己 spawn 的 `subagent` 只属于本 agent 内部执行树，对其他 primary agent 不可见、不可寻址、不可作为跨设备协作对象。
8. 动态 project agent 配置控制面由 `fin project-agent add|remove|list <user.toml> ...` 写入/读取同一个 `runtime/agents/project_agents.json`，禁止 UI/channel 维护第二套 project agent 列表。

## 1.2 AgentControl 数据模型

runtime 是唯一真源，UI / Android / Web 只消费 runtime agent events、状态、消息、工具事件和结果。

最小记录类型：

1. `AgentIdentity`
   - `agent_id`
   - `kind: system_agent | project_agent | subagent`
   - `parent_agent_id`（primary agent 必须为空）
   - `project_id`
   - `device_binding`
   - `auth_subject`
   - `capability_descriptor`
2. `AgentRunRecord`
   - `agent_run_id`
   - `agent_id`
   - `parent_run_id`
   - `task_id`
   - `assignment_id`
   - `status`
   - `result_refs`
   - `last_heartbeat_at`
   - `closed_at`
3. `AgentMailboxMessage`
   - `message_id`
   - `seq`（目标 mailbox 内单调递增）
   - `from_agent_id`
   - `to_agent_id`
   - `thread_id` / `task_id`
   - `trigger_turn`
   - `payload`
   - `consumed_at`

路径规则：

- primary path:
  - `system:<id>`
  - `project:<project_id>:<agent_id>`
- subagent path:
  - `system:<id>/subagent:<run_id>`
  - `project:<project_id>:<agent_id>/subagent:<run_id>`

## 1.3 AgentControl 操作语义

1. `register_primary_agent`
   - 只注册 `system_agent` / `project_agent`。
   - 建立 durable identity、device binding、auth lease、capability descriptor。
2. `spawn_subagent`
   - 父 agent 可以是 `system_agent` 或 `project_agent`。
   - 生成 child agent run，记录 parent path、context mode、task binding、权限边界。
3. `send_agent_input`
   - 支持 primary ↔ primary、primary ↔ subagent。
   - 消息进入 durable mailbox，不走 UI 临时状态。
4. `wait_agent`
   - 等待 subagent 或 project agent 的明确状态变化。
   - 返回 completed / failed / timeout / closed，并附 result refs。
5. `close_agent`
   - 对 subagent：关闭自身及 descendants。
   - 对 primary agent：只允许 release lease / detach / stop service，不允许误当普通 child kill。
6. `resume_agent`
   - primary agent：按 durable identity + auth lease 恢复。
   - subagent：按 parent + `agent_run_id` 恢复。

## 1.4 Context / permission policy

每次 `spawn_subagent` 必须声明上下文策略：

1. `task_summary_only`
   - 默认推荐。
   - 只给任务摘要、必要 artifact refs、权限边界。
2. `last_n_turns`
   - 用于需要最近交互上下文的 worker/reviewer。
3. `full_session_context`
   - 仅限明确需要完整上下文的高级任务。
   - 必须记录 reason。

权限继承规则：

- `subagent` 不拥有独立跨设备身份。
- `subagent` 的 tool 权限不能超过父 agent。
- `project_agent` 的权限由 project binding 决定。
- `system_agent` 不能静默绕过 `project_agent` 的项目权限边界。

---

## 2. role 职责边界

### 2.1 system

负责：

- 用户入口
- entry startup role
- orchestration / routing / dispatch / review / health / recovery
- backlog / task portfolio 的优先级与 owner 管理
- 感知 project agent / peer / channel 的状态

不负责：

- 默认沉入长时间的项目内实现切片
- 退化成长期 executor

### 2.2 project

负责：

- 单项目内的 docs / code / test / debug 闭环
- 项目范围内的 epic/task dispatch、review、delivery closure
- 根据任务需要切换 execution / review / diagnosis / handoff 的工作方式
- 在需要并行执行时派生多个 worker runtime

不负责：

- 把这些工作方式升级成新的 role family

---

## 3. worker 的语义

`worker` 不是 role。

`worker` 是：

- `project agent` 派生或复用的 runtime 执行体
- 拥有独立 `worker_id`
- 可以并行执行不同 project slices
- 可以分别回传 progress / note / result / blocker

`worker` 不是：

- 新的 prompt role
- 新的长期 prompt baseline
- 新的 config role profile

---

## 4. 最小实现约束

当前实现最少要满足：

1. 同一个 `project agent` 可以实例化多个 `WorkerRuntime`
2. 多个 worker runtime 共享 `project` role 真相
3. 多个 worker runtime 必须有不同 `worker_id`
4. 后续扩展应优先进入：
   - supervision
   - mailbox / assign
   - progress aggregation
   - result merge / handoff

而不是扩 role taxonomy。

---

## 5. 可验证条件

至少要有测试覆盖：

1. `system` 启动
2. `project` 启动
3. `default -> project` 兼容
4. `entry_role -> system` 用户入口启动
4. 同一 project agent 派生多个 worker runtime 时：
   - role 仍是 `project`
   - worker_id 不同
   - local peer / context 能区分不同执行体

## 5.1 Startup role split

当前 runtime 启动 contract 冻结为：

- `default_role = project`
- `entry_role = system`

语义：

- `default_role` 供 project/runtime/worker 默认执行体使用
- `entry_role` 供 Web / QQ / CLI 对话入口使用

这两个字段不能再混成一个“默认 role”。

---

## 6. naming contract

所有 agent 都必须有稳定名字。

冻结规则：

1. `agent_id` 采用：
   - `<device_name>.<agent_name>`
2. `device_name` 来源优先级：
   - `user.toml -> runtime.device_name`
   - 本机系统默认名（hostname / computer name）
3. 本地未显式命名的 agent，必须从本地姓名池分配：
   - `~/.fin/runtime/agents/name_pool.json`
4. 本地已分配 agent identity 必须进入 registry：
   - `~/.fin/runtime/agents/registry.json`
   - `~/.fin/runtime/current/current_agent_registry.json`
5. `worker_id` 是 runtime 执行体 id，可由 agent 名派生，但不允许再退化成匿名/纯角色名。

目标：

- 避免多机 / 多 peer / 多 worker 出现重名
- 给后续 daemon / peer routing / status probe / debug UI 提供稳定显示锚点


## 4.1 Owner / reviewer semantics

冻结规则：

- `system agent` 与 `project agent` 都可以是 owner / dispatcher / reviewer
- `worker` 不是最终 review owner，只负责 claim、执行、提交结果
- 谁发布任务，谁 review
- 复杂任务的派发必须通过 project task system 管理，不绕过 owner truth 直接私聊 worker
