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
