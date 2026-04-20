# M1 Current State Summary

本文档回答三个问题：

1. **fin 现在到底已经完成到什么程度**
2. **哪些部分已经可以视为当前稳定真源**
3. **下一阶段最小目标应该是什么**

它不是新架构文档，而是 M1 closeout 之后的阶段性状态快照。

## 1. 一句话结论

截至 2026-04-20，`fin` 已经具备：

- **真实 provider 可用**
- **单 agent 多轮推理闭环可用**
- **同 turn 自动 tool loop 可用**
- **session / turn / step / event / control plane 的 durable truth 已落地**
- **Web debug / status probe / compact / tick / reminder / queue / interrupt 基本闭环可用**
- **正式 build/install gate 已恢复**
- **qqbot 真实 channel gateway 对话闭环可用**
  - 已覆盖 `message.ingest -> conversation/session restore -> session truth -> runtime inference -> session truth -> message.emit`
  - repo 内已有 `process_inbound_message(...)` 集成 E2E
  - 真实 runtime 已生成 live receipt：`~/.fin/harness/runs/qqbot-live-receipt-20260420-real/qqbot-live-receipt.json`
  - 当前 live receipt 证明：
    - `ack_notice_present = true`
    - `session_reply_present = true`
    - `provider_request_present = true`
    - `provider_response_present = true`

因此当前项目状态应视为：

> **M1 最小闭环已经成立；当前更重要的是守住冻结边界、补齐 receipts，并把剩余 always-on / daemon / distributed 扩展留在 M2。**

---

## 2. 当前已经稳定的能力边界

## 2.0 agent taxonomy

当前已冻结的 agent taxonomy：

1. prompt role 真源只有 `system` 与 `project`
2. `default` 仅作为历史兼容 alias，运行时解析到 `project`
3. `worker` 不再是 role，而是 `project agent` 可派生/复用的 runtime 执行体

这意味着当前 M1 的多执行体扩展方向已经固定为：

- 多 worker runtime
- supervision / mailbox / assign / merge

而不是再增加新的 role family。

## 2.1 推理主链

当前已经具备的最小推理闭环：

1. 用户输入进入 session truth
2. runtime 组装 context
3. provider 发真实请求
4. 模型输出解析为：
   - assistant response
   - control feedback
   - tool calls
5. 工具执行结果进入 tool records / events
6. runtime 产出：
   - progress
   - note
   - digest
   - reasoning view
   - turn record
   - step ledger
   - routing decision / action
7. session / runtime current / debug projection 同步可见

这条链已经不是 demo 级拼接，而是有 durable truth 支撑的最小 runtime。

## 2.2 session / history / rebuild

当前已经稳定的 session 相关能力：

1. `conversation/messages.json` 作为会话渲染真源
2. `recent_contexts / recent_digests / recent_reasoning_views / recent_tool_records`
3. `turns / steps / provider / routing / closures` 作为完整追索链
4. `/compact` 走 framework rebuild，而不是模型压缩

当前语义已经明确：

- 对话渲染真源 ≠ 完整推理历史真源
- 完整推理历史必须通过 materialized artifacts 追索
- Web 只能消费这些 artifacts，不能自己补 runtime 事实

## 2.3 control plane

当前已经形成最小控制面：

1. `RoutingDecisionRecord`
2. `RoutingActionRecord`
3. `ExecutionStateRecord`
4. `PendingInputRecord`
5. `InterruptedSegmentRecord`
6. `SegmentMergeRecord`
7. `SchedulerDecisionRecord`
8. `SchedulerTickRecord`
9. `SupervisorCycleRecord`
10. `SupervisorHeartbeatRecord`
11. `DaemonStateRecord`
12. `DaemonRecoveryActionRecord`

这些记录让系统已经不再只是“有个聊天推理器”，而是开始具备：

- 任务继续/阻断判断
- queue 推进判断
- wake / reminder / tick 驱动
- attached daemon 健康观察

## 2.4 可观测性

当前已经稳定的可观测边界：

1. `events/stream.jsonl` 为 hot raw event truth
2. local archive / cold archive 为历史归档
3. archive index 为显式浏览入口
4. Web debug / status probe 读 session truth
5. provider debug 只允许脱敏信息进入 projection

当前已经可以可靠回答：

- 这次 turn 做了什么
- 每一步发生了什么
- 哪一轮 provider / control / tool 做了什么
- 当前 queue / reminder / tick / heartbeat / daemon 是什么状态

## 2.5 工程门禁

当前已经恢复并通过：

1. `cargo fmt --all --check`
2. `cargo test -p fin-runtime -p fin-cli -p fin-debug-server --manifest-path rust/Cargo.toml`
3. `python3 scripts/check-code-line-limit.py`
4. `cargo run -p fin-cli --manifest-path rust/Cargo.toml -- install-dev ~/.fin/config/user.toml 0.1.0001`

这意味着当前项目不是“代码看起来能跑”，而是：

- 开发态可回归
- 行数门禁可执行
- 安装态可验证

---

## 3. 当前的唯一真源分布

## 3.1 runtime 真源

由 Rust runtime 持有：

- context assembly
- model output parsing
- tool dispatch
- control feedback truth
- routing action truth
- turn / step / round / digest / note / progress truth

## 3.2 session 真源

由 session artifacts 持有：

- conversation render truth
- recent context/digest/reasoning/tool windows
- turn / step / provider / routing / closure durable history
- queue / interrupts / control plane / archive index

## 3.3 Web 真源边界

Web 不持有业务真相，只持有：

- rendering
- filtering
- archive drill-down
- inspector 展示

换句话说：

> **Rust runtime 负责生产事实，session 负责持久化事实，Web 负责观察事实。**

---

## 4. 当前还没有做的事情

这些事情现在不要误判成“已经完成”：

1. 真多 agent 执行面
2. detached/headless daemon
3. 跨机器 project agent 协作
4. 真正从中间步骤恢复的 pause/resume
5. 普通用户输入的真正并行推理
6. 完整 session/task/topic 产品化交互
7. 成熟 knowledge graph / wiki / memory graph

以上均属于 M2/backlog，不应在当前阶段重新发散。

---

## 5. 当前收口结果

截至当前，原本计划中的 **M1.1 stability pass 核心目标已经收口**，其中 **qqbot channel gateway 闭环** 已经用 repo 内 E2E + live receipt 双重证据收下。

已经实际收下的证据包括：

### 5.1 receipt 标准化

当前已完成：

- `scripts/build-receipt-index.py`
- `harness/reports/<build>/receipt-index.json`
- `install-dev / build-dev` 默认刷新 `receipt-index.json`
- `install_smoke` 摘要带 `session_id / task_id / operation_id / verified_paths`

### 5.2 推理主链 review 固化

当前已完成：

- 单 agent inference mainline review 已文档化
- runtime / session / web 的 owning layer 已冻结
- Web 不再被允许补 runtime 事实

### 5.3 mainline receipts 已成组通过

当前 closeout run 中，三类 mainline receipt 都已 `passed`：

1. `history_context`
2. `auto_tool_roundtrip`
3. `control_boundary`

其中 `control_boundary` 已升级到 **async control-plane receipt**，覆盖：

- `wait_external`
- `reminder_scheduled`
- `reminder_fired`
- `supervisor_heartbeat_due`
- `stale_lease`

### 5.4 qqbot live receipt

当前已新增：

- 命令：`fin qqbot-live-receipt <user.toml> <qqbot-target> [run-id]`
- live receipt：
  - `~/.fin/harness/runs/qqbot-live-receipt-20260420-real/qqbot-live-receipt.json`

receipt 当前固定验证：

- target / conversation / session 绑定真相
- latest inbound / latest delivered cursor
- ack 已发出
- session truth reply 已发出
- provider request / response artifacts 存在

这说明 `qqbot` 已经不再只是 peer skeleton，而是进入了 **真实 channel 闭环 + receipt 可回收** 的状态。

结论：

> 当前 `fin` 不再只是“推理核心能跑”，而是已经进入“推理核心可验证、可回归、可冻结，并且拥有真实 channel receipt”的状态。

---

## 6. 不建议的下一步

当前不建议立即切回：

1. remote peer / distributed agent
2. 真 daemon 常驻体系
3. 新 UI 大改版
4. 大规模 tool/channel/gateway 家族扩张
5. memory graph / wiki 系统化重构

原因很简单：

这些都建立在“当前单 agent runtime 足够稳”之上；
现在更高收益的是先把已有闭环冻结清楚。

---

## 7. 若继续推进，推荐执行顺序

当前若继续推进，建议严格按这个顺序走：

1. **保持当前 receipt / regression 持续为绿**
2. **只做 truth consistency 与防回退修复**
3. **若要扩能力，先明确是否正式切入 M2**
4. **进入 M2 前，先回到 architecture docs 冻结 owning layer**

---

## 8. 当前阶段判断

如果只用一句话概括当前项目状态：

> `fin` 现在已经有了一个可运行、可观察、可安装、可回归，并且主链 receipt 已闭合的单 agent runtime 内核；下一步最重要的不是继续补 M1 功能，而是守住冻结边界，并谨慎决定何时进入 M2。
- `context.peer` 已能把 ensured local worker peer state 回流到后续上下文，因此 project->worker 的本地协作骨架不再只是 assignments/mailbox 落盘，而是进入推理与观察真源。
