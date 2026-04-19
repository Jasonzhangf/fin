# M1 Current State Summary

本文档回答三个问题：

1. **fin 现在到底已经完成到什么程度**
2. **哪些部分已经可以视为当前稳定真源**
3. **下一阶段最小目标应该是什么**

它不是新架构文档，而是 M1 closeout 之后的阶段性状态快照。

## 1. 一句话结论

截至 2026-04-19，`fin` 已经具备：

- **真实 provider 可用**
- **单 agent 多轮推理闭环可用**
- **同 turn 自动 tool loop 可用**
- **session / turn / step / event / control plane 的 durable truth 已落地**
- **Web debug / status probe / compact / tick / reminder / queue / interrupt 基本闭环可用**
- **正式 build/install gate 已恢复**

因此当前项目状态应视为：

> **M1 已经从“边搭边试”进入“可冻结、可验证、可继续成熟化”的阶段。**

---

## 2. 当前已经稳定的能力边界

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
2. 跨机器 project agent 协作
3. detached/headless daemon
4. 真正从中间步骤恢复的 pause/resume
5. 普通用户输入的真正并行推理
6. 完整 session/task/topic 产品化交互
7. 成熟 knowledge graph / wiki / memory graph

这些仍然属于 M2/backlog，不应在当前阶段重新发散。

---

## 5. 当前最适合的下一阶段最小目标

当前最合理的下一步，不是继续扩新框架，而是：

## 5.1 目标：把“推理核心已可用”提升为“推理核心可稳定维护”

也就是做 **M1.1 stability pass**。

只聚焦三件事：

### A. receipt 标准化

把现在已经存在的：

- provider real smoke
- compact rebuild
- status probe
- install-dev

统一整理成更稳定、可重复引用的 receipt 体系。

当前第一步已落地：

- `scripts/build-receipt-index.py`
- `harness/reports/<build>/receipt-index.json`

### B. 推理主链 review 固化

把当前单 agent 推理主链收成一份固定真源清单：

1. 输入进入哪里
2. context 如何组装
3. provider 请求如何落盘
4. model output 如何分块
5. tool loop 如何继续
6. stop / wait / interrupt / queue 的边界是什么
7. 哪些 artifacts 是 UI/debug/replay 应消费的

### C. M1 冻结后的小范围成熟化

只允许继续做：

1. truth consistency 修复
2. regression / receipt 补强
3. 现有控制面和推理主链的小范围清理

不切回大框架扩张。

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

## 7. 推荐执行顺序

下一阶段建议严格按这个顺序走：

1. **完成当前状态总结**（本文）
2. **固定 M1.1 最小目标**
3. **补 receipt / regression / truth consistency**
4. **对推理主链做一次完整 review 文档化**
5. 再决定是否进入 M2

---

## 8. 当前阶段判断

如果只用一句话概括当前项目状态：

> `fin` 现在已经有了一个可运行、可观察、可安装、可回归的单 agent runtime 内核；下一步最重要的不是继续加能力，而是把这套内核稳定化、标准化、冻结化。
