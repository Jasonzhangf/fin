# 41 Progress Updater And Delivery

## 索引概要
- L1-L9 `purpose`：冻结统一 progress updater 的目标与边界。
- L11-L36 `core-conclusion`：唯一真源、被动框架、delta-first delivery。
- L38-L69 `layering`：runtime truth / projection / policy / reporter 四层。
- L71-L104 `snapshot-model`：统一 `ProgressSnapshot` 的结构分层。
- L106-L134 `meaningful-delta`：什么变化才值得发送给用户。
- L136-L157 `heartbeat`：heartbeat 不是空刷屏，而是等待说明。
- L159-L181 `resource-visibility`：资源命名池、资源展示、gateway 排除规则。
- L183-L199 `agent-reporting`：per-agent report 的来源与约束。
- L201-L220 `channel-debug`：QQ / Web / status / debug 的统一输出要求。
- L222-L250 `implementation-phases`：落地阶段与验收标准。

## purpose

本文档冻结 `fin` 的统一 `progress updater` 设计。

目标：

1. 让 QQ / Web / `/status` 使用同一份 progress truth
2. 避免文字 channel 周期性重复发送低信息量快照
3. 把资源、项目进度、agent report、tool activity 分层固定
4. 保持 `fin` 的核心边界：**framework 是被动触发执行者，不理解自然语言**

---

## 1. Core conclusion

`fin` 的 progress updater 不应继续被实现为：

```text
QQ 专用 activity text renderer + heartbeat 重发
```

而应冻结为：

```text
runtime truth
-> progress projection
-> delivery policy
-> reporter/output adapter
```

四条核心结论：

1. **progress truth 只能来自 framework 已知的结构化事实**
2. **reporter 不是第二真源**；QQ/Web 只能读取 projection 渲染
3. **默认采用 delta-first delivery**；没有有效变化就不发送新卡
4. **progress 必须忠实表达 truth / trigger / agent_report，不得渲染成“框架已替模型执行完成”**

### 1.1 Passive framework rule

本设计必须服从 `fin` 已冻结的框架边界：

- framework 不能理解自然语言用户输入
- framework 不能从聊天文本、文案、关键词、阈值推断任务状态
- framework 只能消费：
  - 用户显式刚性命令
  - 模型显式 `control block` / schema code
  - runtime 已持久化的结构化 operation / event / projection

因此：

- project progress 不能从普通聊天自由文本里“猜”出来
- agent report 不能从普通 assistant 回复里“猜”出来
- stagnation / repetition 检测也只能比较结构化 fingerprint，不能靠自然语言理解
- recovery / wake / resume 相关文案必须标明是 `trigger/observation` 还是 `agent_report/execution`

### 1.2 No-work / no-delta rule

冻结两条刚规则：

1. **Heartbeat：没有任务，不要动 Agent**
   - heartbeat 只允许检查结构化 due work / stale lease / due reminder / resumable assignment
   - 若没有待推进事项，heartbeat 只能记录 observation truth，**不得触发新的 agent turn**
   - “只是时间过去了”不是触发 agent 的理由

2. **Progress：没有更新，不要更新**
   - progress delivery 默认严格 `delta-only`
   - 若本次 snapshot 与上次 delivered snapshot 在用户可见语义上没有变化，则**不得发送新的 progress**
   - 不允许借 heartbeat 名义重复发送同一批旧等待/旧失败/旧查询摘要

一句话冻结：

```text
no work -> no touch
no delta -> no delivery
```

### 1.3 Hidden heartbeat turn persistence rule

heartbeat / checkpoint-resume / assignment-resume 这类 **framework-owned hidden turn** 还必须再服从一条刚规则：

```text
hidden control-plane turn != normal session turn
```

也就是：

- 可以更新 framework current/control truth
- 可以写 control-plane 事件与 checkpoint 真相
- **不得**把这类隐藏 turn materialize 到正常 session 的：
  - `conversation/messages.json`
  - `digests/recent_digests.json`
  - `reasoning/recent_reasoning_views.json`
  - `tools/recent_tool_records.json`
  - `provider/rounds/steps/turns/closures` 历史窗口
- 不得让 heartbeat/hidden resume 偷改前台 `last_run` 绑定，污染当前业务 session 的可见上下文

---

## 2. Layering

统一分为四层。

### 2.1 Runtime truth layer

唯一允许的输入事实：

- agent presence
- session / task / dispatch / assignment 状态
- tool execution semantic record
- operation / event timeline
- control block 中显式 report / wait / completion 信息
- channel binding / delivery state
- startup / wake / recovery / supervision truth

### 2.2 Progress projection layer

由 runtime 从结构化事实派生统一：

```text
ProgressSnapshot
```

这层负责：

- 聚合同一 session / task 的当前推进状态
- 提供 team/resource/project/agent/tool/report 的统一读取视图
- 明确区分：
  - `framework_observation`
  - `trigger_visible`
  - `agent_report`
- 提供 delivery fingerprint 与 delta 检测输入

这层不负责：

- 渲染 QQ 文本
- 决定 agent 是否该被派发
- 补写自然语言解释

### 2.3 Delivery policy layer

策略层回答：

- 哪些 section 要显示
- 更新模式是 `event` / `heartbeat` / `both`
- detail level 是 `compact` / `detailed` / `verbose` / `debug`
- 没变化时是否抑制
- heartbeat 多久触发

### 2.4 Reporter / output adapter layer

输出层只做：

```text
ProgressSnapshot + DeliveryPolicy -> rendered output
```

目标通道：

- QQ text channel
- Web debug / frontstage progress panel
- `/status` / status probe
- debug mode tagged text

输出层禁止：

- 拼第二套业务状态
- 重新推断 agent 是否正在做任务
- 只因时间流逝就不断重发同文案
- 把 trigger 写成“已执行完成”
- 在没有 due work 时借 heartbeat 触发 agent

---

## 3. Snapshot model

建议新增统一 contract：

```text
ProgressSnapshot
```

最小结构分为五组：

### 3.1 Headline / global status

- `session_id?`
- `task_id?`
- `generated_at`
- `global_status`：`idle | running | waiting | failed | completed`
- `headline?`
- `current_phase?`
- `phase_since?`
- `last_meaningful_change_at?`

### 3.2 Project progress

若当前未进入 managed project path，可为空。

建议字段：

- `project_id`
- `display_name`
- `status`
- `summary?`
- `task_summary { total/queued/running/blocked/review_ready/completed }`
- `active_tasks[]`
- `recovery_trigger?`

### 3.3 Resource / team status

建议字段：

- `total`
- `running`
- `waiting`
- `idle`
- `offline`
- `failed`
- `members[]`

其中每个成员至少带：

- `agent_id`
- `display_name`
- `kind`
- `runtime_status`
- `task_id?`
- `phase?`
- `updated_at`

### 3.4 Agent progress / agent report

每个 agent / worker 独立一条，而不是折叠为单个 project agent。

建议字段：

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
- `report_kind`：`framework_observation | trigger_visible | agent_report`

### 3.5 Tool / delivery health

建议字段：

- `recent_tools[]`
- `delivery_health.stagnant`
- `delivery_health.repeated_action_fingerprint?`
- `delivery_health.repeated_count?`
- `delivery_health.last_delivery_at?`

---

## 4. Meaningful delta gate

默认规则：**没有有效变化，不发送新的 progress card。**

### 4.1 允许发送的变化

只有下列变化至少发生一项，才允许发送新的 progress update：

1. `current_phase` 变化
2. `current_task_id` 变化
3. 资源分布变化（running/waiting/idle/failed）
4. active agent 名单变化
5. tool `started/completed/failed` 变化
6. `waiting_reason` 变化
7. `failure_detail` 变化
8. `latest_report` 的 `status/summary/next_action/evidence` 变化
9. project task summary 变化
10. recovery / wake / review trigger 的可见状态变化
11. 仅时间戳变化、lease 刷新、同文案重算，不算变化

### 4.2 不应发送的场景

以下情况默认抑制：

- 只是 heartbeat tick
- 只是 `generated_at/updated_at` 变化
- 文本渲染几乎相同、只是重算一次 snapshot
- 仍然只是“已收到，正在处理”且没有新等待原因/新 phase/新 tool
- 只是 framework 重新扫描了一次 startup/recovery truth，但用户可见语义没变

### 4.3 Delivery fingerprint

每次 delivery 应计算结构化 fingerprint，建议最少包含：

- `current_phase`
- `task_id`
- `running/waiting/idle/failed counts`
- `active_agent_ids`
- `current_tool summary`
- `waiting_reason`
- `latest_report summary`
- `project task summary`
- `visible_trigger summary`

若 fingerprint 不变，则不发送新的 progress update。

---

## 5. Heartbeat

heartbeat 只用于**内部调度观察**，以及在确有新等待语义时说明“仍在进行，但目前处于等待/长耗时阶段”。

### 5.1 Heartbeat semantics

heartbeat 必须回答：

- 当前卡在哪个 phase
- 已持续多久
- 在等什么
- 最近一次有效进展是什么

但前提是：

- 确实存在 due work 或新的等待语义
- 否则 heartbeat 只能留在 framework truth，不应打扰 agent，也不应打扰用户

### 5.2 Heartbeat anti-pattern

禁止：

- 每隔 N 秒重复发送同一张 card
- heartbeat 与 progress update 共用同一模板，导致重复刷屏
- 没有等待原因、持续时长、最近有效动作，却只发“已收到，正在处理”
- 没有 due task / due reminder / due resume，却因为定时器到了就推进 agent

---

## 6. Resource visibility

### 6.1 Naming truth

资源显示必须使用 framework 已有命名池真源：

- system worker：`system-worker-01..N`
- project worker：`<project-agent-base>-worker-01..N`
- project agent：按 project base name

resource reporter 只能读统一 `display_name`，不能在 renderer 内重新猜名字。

### 6.2 Gateway exclusion

`channel_gateway.*` 属于 ingress/bridge，不属于 execution resource pool。

因此：

- 可显示为 active source
- 但默认不计入资源总数 `total/running/waiting/idle`

### 6.3 Render rule

资源展示分两层：

1. **summary line**：总数与 running/waiting/idle/failed 计数
2. **detail line**：仅在有变化或 detail level 足够高时，展开名字列表

默认不要在每条 heartbeat 中重复打印整份空闲 worker 名单。

---

## 7. Agent reporting

### 7.1 Source of truth

per-agent report 只能来自：

1. agent 在 `control block` 中显式输出的结构化 report
2. project task system 已持久化的结构化 submit/review/report record
3. framework 已知的结构化 observation（例如 waiting/tool/presence）

### 7.2 Strict boundary

禁止：

- 从普通 assistant 自然语言回复中猜 report
- 从 channel 文案里反推 task completion
- framework 在没有 report contract 的情况下伪造“agent 已完成/已阻塞”的 rich report

### 7.3 Observation vs report

用户面要能区分：

- `observation`：framework 观察到的 busy/waiting/tool/presence
- `trigger_visible`：framework 观察到“存在恢复/派发/审核候选”
- `agent_report`：agent 自己通过结构化 contract 汇报的 summary/status

---

## 8. Channel / debug output

### 8.1 Unified reporter

QQ / Web / `/status` 必须共用同一个 progress projection。

区别只允许存在于：

- detail level
- section toggle
- debug tag 开关
- 文本与卡片样式

### 8.2 Debug mode tagging

debug mode 建议显式标示来源，例如：

- `[framework][observation]`
- `[framework][trigger]`
- `[agent][system]`
- `[agent][project:<name>]`
- `[agent][worker:<name>]`

这用于区分：

- 是框架观察摘要
- 还是 trigger 可见项
- 还是 agent 自己的结构化 report
- 还是工具/控制面事件

---

## 9. Implementation phases

### Phase 1 — Unified projection truth

1. 新增 `ProgressSnapshot` contract
2. 将 resource/team/agent/project/tool 统一进 projection
3. 让 QQ / Web / `/status` 读取同一份 projection

验收：

- 资源名来自统一 naming pool
- gateway 不再计入资源总数
- 不再需要 channel-local 拼第二套业务状态

### Phase 2 — Delivery policy + delta gate

1. 把现有 `qqbot progress policy` 升级为通用 `ProgressDeliveryPolicy`
2. 接入 fingerprint + meaningful delta gate
3. heartbeat 改用单独模板

验收：

- 无变化时不再周期刷屏
- heartbeat 能说明等待原因与持续时长
- QQ 状态卡不会连续多次发送相同低信息量文本

### Phase 3 — Structured per-agent report

1. 新增 `AgentStructuredReport`
2. 将 project task submit/review/report 真相映射进 progress snapshot
3. 区分 `observation` / `trigger_visible` / `agent_report`

验收：

- 用户能看到每个 agent/worker 的独立进展
- 能区分“框架观察”“存在触发候选”“agent 自报”

### Phase 4 — Stagnation / delivery health

1. 引入 `DeliveryHealthSnapshot`
2. 基于结构化 fingerprint 做重复/停滞检测
3. debug mode 显示 stagnation 线索

验收：

- 能发现“同一操作反复执行且内容无变化”的空转
- 能区分正常等待与重复无进展
