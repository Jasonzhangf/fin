# Progress Snapshot Contract

`ProgressSnapshot` 是 `fin` 的统一 progress updater 读取真源。

它不是新的业务真源，而是从 runtime 已有结构化事实投影出的统一快照，用于：

- QQ / text channel progress
- Web progress panel
- `/status` / status probe
- debug mode delivery

## 最小字段

### Snapshot head

- `schema`：固定为 `fin.progress-snapshot.v1`
- `session_id?`
- `task_id?`
- `generated_at`
- `global_status`
- `current_phase?`
- `headline?`
- `phase_since?`
- `last_meaningful_change_at?`

### Project progress

- `project?`
  - `project_id`
  - `display_name`
  - `status`
  - `summary?`
  - `task_summary`
  - `active_tasks[]`

### Resource snapshot

- `resources?`
  - `total`
  - `running`
  - `waiting`
  - `idle`
  - `offline`
  - `failed`
  - `members[]`

每个 `member` 最少包含：

- `agent_id`
- `display_name`
- `kind`
- `runtime_status`
- `task_id?`
- `phase?`
- `updated_at`

### Agent progress

- `agents[]`

每个 agent 最少包含：

- `agent_id`
- `display_name`
- `role`
- `runtime_status`
- `current_task_id?`
- `current_session_id?`
- `current_phase?`
- `progress_summary?`
- `waiting_reason?`
- `current_tool?`
- `latest_report?`
- `observation_source`
- `updated_at`

### Tool / delivery health

- `recent_tools[]`
- `delivery_health?`
  - `stagnant`
  - `repeated_action_fingerprint?`
  - `repeated_count?`
  - `last_delivery_at?`

## 契约要求

1. `ProgressSnapshot` 只能消费 runtime 已有结构化 truth，不得读取自然语言聊天去猜状态。
2. `ProgressSnapshot` 是 projection，不是事实真源；原始事实仍然是 operation / event / presence / task registry / tool record / control block。
3. 同一 session 内如果有多个 worker，`agents[]` 必须按 worker identity 独立保留，禁止折叠成单个 project agent。
4. 用户面默认显示 `display_name`，内部 id/type 保留在结构化字段中。
5. `channel_gateway.*` 默认不计入 `resources.total`，除非后续明确定义为 execution resource。
6. `latest_report` 只能来自显式结构化 report truth，不得从普通 assistant 文本中猜测。
7. progress delivery 必须支持基于结构化字段计算 fingerprint；`generated_at/updated_at` 这类纯时间字段不得单独触发新卡发送。
8. heartbeat 渲染必须基于 `current_phase + phase_since + waiting_reason + last_meaningful_change_at`，不得复用普通 progress 卡导致重复刷屏。
