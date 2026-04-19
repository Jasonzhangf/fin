# M1 Inference Mainline Review

本文档只做一件事：

> 把当前 **单 agent 推理主链** 的实现边界、真源分布、输入输出与消费面固定下来。

它不是新设计草案，而是对当前已实现系统的 review 与冻结。

---

## 1. 一句话结论

当前 `fin` 的单 agent 推理主链已经形成如下稳定结构：

```text
user/debug input
  -> chat policy / local command / status side-path
  -> context assembly
  -> inference operation build
  -> runtime run_closure
  -> provider round(s)
  -> model output parse
  -> tool dispatch / wait / stop
  -> progress / note / digest / reasoning / routing
  -> event emission
  -> session materialization
  -> web/status/debug consumption
```

这条链当前已经是：

- **可运行**
- **可追索**
- **可落盘**
- **可观察**

因此后续修改必须以这条主链为准，不得再在 Web 或 wrapper 层私自复制业务真相。

---

## 2. 当前主链 owning files

## 2.1 runtime 入口与主闭包

主要文件：

- `rust/crates/runtime/src/lib.rs`
- `rust/crates/runtime/src/closure_runtime.rs`
- `rust/crates/runtime/src/closure_runtime_rounds.rs`
- `rust/crates/runtime/src/closure_runtime_events.rs`

职责：

1. `lib.rs`
   - 暴露 runtime canonical types
   - 定义 `WorkerRuntime`、`InferenceRequest`、`InferenceOperationBuilder`
   - 定义 `ClosureRun`

2. `closure_runtime.rs`
   - 驱动一次完整 closure
   - 负责 round loop、progress/note/digest/finalize、operation completion

3. `closure_runtime_rounds.rs`
   - 负责单轮 provider + parse + tool dispatch 的执行
   - 负责 `RoundRecord` / `StepRecord` 构造

4. `closure_runtime_events.rs`
   - 负责事件发射
   - 负责把 round/step/progress/note/digest/routing 等事实转成 event truth

结论：

- **推理主链真源在 runtime**
- 不在 CLI
- 不在 debug server
- 不在 WebUI

---

## 2.2 session materialization

主要文件：

- `rust/crates/runtime/src/session_materializer.rs`
- `rust/crates/runtime/src/session_materializer_events.rs`
- `rust/crates/runtime/src/session_materializer_support.rs`
- `rust/crates/runtime/src/session_record_journal.rs`

职责：

1. 把 closure 结果写入 `runtime/current/*`
2. 把 closure 结果写入 `sessions/.../*`
3. 管理 recent windows / retention
4. 维护 `last_run.json`
5. 维护 event hot stream / archive / archive index

结论：

- **session 是 durable truth 的持久化层**
- materializer 是 runtime -> artifacts 的唯一出口
- UI/debug 若绕过 artifacts 自造事实，就会破坏主链

---

## 2.3 control plane

主要文件：

- `rust/crates/runtime/src/control_plane.rs`
- `rust/crates/runtime/src/routing_actions.rs`
- `rust/crates/runtime/src/scheduler.rs`

职责：

1. `control_plane.rs`
   - execution state
   - pending input
   - pause checkpoint
   - interrupted segment
   - merge record

2. `routing_actions.rs`
   - 从 `RoutingDecisionRecord` 派生 canonical `RoutingActionRecord`

3. `scheduler.rs`
   - 从当前状态派生 canonical `SchedulerDecisionRecord`

结论：

- control plane 的状态推进真源已经下沉到 runtime
- CLI 只应做：
  - 文件路径解析
  - 命令 glue
  - 调用 runtime truth

---

## 2.4 CLI orchestration

主要文件：

- `rust/crates/cli/src/web_debug.rs`
- `rust/crates/cli/src/status_probe.rs`
- `rust/crates/cli/src/session_commands.rs`
- `rust/crates/cli/src/demo.rs`

职责：

1. `web_debug.rs`
   - debug 输入总入口
   - request 分类：
     - status probe
     - local slash command
     - interrupt request
     - queue
     - run now
   - heartbeat / reminder / supervisor / daemon attached observation glue

2. `status_probe.rs`
   - 只读 session/runtime current truth
   - 绝不能生成新 closure

3. `session_commands.rs`
   - `/new /resume /pause /resume-run /tick /compact`
   - slash command 的 session/control 层 glue

4. `demo.rs`
   - 组装 demo request
   - 走同一条 runtime mainline

结论：

- CLI 是 orchestration / entry glue
- 不是推理真源
- 不是 context 真源
- 不是 control 语义真源

---

## 3. 当前主链的固定步骤

## 3.1 输入分类

输入首先在 CLI 层被分类：

1. `status_probe`
2. `local command`
3. `interrupt_request`
4. `queue`
5. `run_now`

固定规则：

- `status_probe` 必须走 side-path
- `local command` 必须先于普通推理处理
- `paused/running/waiting_external` 时普通输入优先排队
- interrupt request 可立即执行

---

## 3.2 context assembly

由：

- `ContextViewBuilder`
- `ModelInputAssembler`

共同完成。

固定输入块包括：

1. 当前用户输入
2. recent messages
3. recent digests
4. recent reasoning views
5. recent tool records
6. project/runtime/cwd/selected paths
7. role / provider path / overlays / prompt modules

固定结论：

- context 真源在 runtime 装配层
- 不是在 Web inspector
- 不是在 provider adapter

---

## 3.3 operation build

由 `InferenceOperationBuilder` 负责：

1. 把 `WorkerRuntime.policy` 写入 payload
2. 把 context 封装为 `InferenceOperationPayload`
3. 生成 canonical operation envelope

固定结论：

- 一次推理的最小执行单位是 **operation**
- 一次 operation 收束为一个 **closure run**

---

## 3.4 provider round execution

由 `execute_round()` 负责：

1. assemble rendered input
2. prepare request
3. execute prepared request
4. parse model output
5. dispatch tool calls
6. merge control feedback fallback

固定结论：

- round 是 closure 内的最小 provider 往返单位
- round truth 必须体现在：
  - `RoundRecord`
  - round-level events
  - step ledger

---

## 3.5 auto tool loop

由 `run_closure()` 中的 while loop 驱动。

当前固定边界：

1. 若模型发出 tool calls，则可继续 follow-up round
2. follow-up input 由：
   - original input
   - last assistant response
   - recent tool results
   共同构成
3. 有上限：`max_auto_tool_rounds = 6`
4. hit 上限会发出：
   - `reasoning.auto_tool_roundtrip_limit_reached`

固定结论：

- auto tool loop 是 runtime 事实，不是 UI 解释
- 每一轮都必须落成：
  - request/response
  - round record
  - step records
  - round events

---

## 3.6 stop / wait / interrupt 边界

当前固定语义：

1. `reasoning.stop`
   - 当前 closure 停止
   - `operation.completed.status = stopped`

2. `wait.remind`
   - 当前 closure yield
   - `operation.completed.status = waiting_external`
   - 不得继续同 turn 自动 tool loop

3. `interrupt_request`
   - 当前由 CLI orchestration 立即发起新 closure
   - open interrupted segment 仍保留，等待后续 merge

固定结论：

- stop / wait 是 runtime mainline 语义
- interrupt 是 orchestration 语义
- 不允许在前端解释层重定义这些边界

---

## 3.7 finalize 与 closure output

一轮 closure 的收口产物固定为：

1. `ProgressBlock`
2. `ExecutionNote`
3. `ReasoningViewRecord`
4. `DigestRecord`
5. `RoutingDecisionRecord`
6. `RoutingActionRecord`
7. `TurnRecord`
8. `ClosureTraceRecord`
9. `EventEnvelope[]`

固定结论：

- 这些记录一起构成 closure truth
- 不允许只依赖其中某一项就宣称“看懂了整轮推理”

---

## 4. 当前 artifacts 的固定消费边界

## 4.1 conversation/messages.json

只负责：

- user / assistant 会话渲染真源

不负责：

- 完整推理历史
- 工具完整执行历史
- reasoning 全量事实
- provider 往返细节

固定结论：

> `messages.json` 是 render truth，不是 full inference truth。

## 4.2 recent windows

主要包括：

- `recent_contexts.json`
- `recent_digests.json`
- `recent_reasoning_views.json`
- `recent_tool_records.json`
- `recent_rounds.json`
- `recent_steps.json`
- `recent_turns.json`

职责：

- 提供 bounded recent truth
- 为 Web/debug/replay 提供快速可见窗口

## 4.3 current/*

主要包括：

- `current_context.json`
- `current_control_feedback.json`
- `current_reasoning_view.json`
- `current_turn.json`
- `current_step_records.json`
- `current_provider_requests.json`
- `current_provider_responses.json`
- `current_rounds.json`
- `current_routing_decision.json`
- `current_routing_action.json`

职责：

- 提供“最近一次 closure”的 current truth

## 4.4 event stream / archive

当前固定语义：

1. hot `events/stream.jsonl`
2. local archive segments
3. cold archive directory
4. archive index 为显式浏览入口

固定结论：

- raw events 不做静默裁剪
- 只能 rotation / archive
- archive 浏览必须显式，不可偷偷混回 live timeline

---

## 5. 当前 Web / debug 的合法消费面

Web / debug 当前允许消费：

1. session messages
2. recent contexts / digests / reasoning / tool / rounds / turns
3. latest progress / note / control / routing / scheduler / heartbeat / daemon records
4. live events / archive index / archive segment

Web / debug 当前不允许：

1. 根据零散字段自己重建业务结论
2. 自己推断 routing action
3. 自己把 archive 事实伪装成 live truth
4. 跳过 session/runtime artifacts 去发明新的状态

---

## 6. 当前主链的固定反模式

后续开发必须避免这些反模式：

1. **在 Web 层补 runtime 结论**
2. **把完整推理历史塞回 `messages.json`**
3. **绕过 runtime 直接在 CLI 里发明 control plane 语义**
4. **把 archive 数据静默混回 live timeline**
5. **只看 digest/note/message 就假装理解整轮 closure**
6. **把 wait/stop/interrupt 三种语义混写**

---

## 7. 当前主链 review 结论

如果把当前系统抽象成三层：

```text
entry/orchestration -> runtime truth -> materialized/session truth -> web/debug observe
```

那么当前已经冻结的 owning layer 是：

1. **entry/orchestration**
   - `web_debug.rs`
   - `session_commands.rs`
   - `status_probe.rs`

2. **runtime truth**
   - `lib.rs`
   - `closure_runtime*.rs`
   - `control_plane.rs`
   - `routing_actions.rs`
   - `scheduler.rs`

3. **materialized/session truth**
   - `session_materializer*.rs`
   - `session_record_journal.rs`

4. **observe layer**
   - debug server
   - web ui

这条 ownership 现在已经足够清楚，而对应的 M1.1 收口动作也已经完成核心部分：

1. receipt 标准化已落地
2. mainline-focused receipts 已实际生成
3. truth consistency 的 P0/P1 修正已收口到当前 closeout 文档

---

## 8. 若继续推进，最小动作

基于本 review，当前若继续推进，最小动作应固定为：

1. **维持主链 receipts 持续可重建**
2. **维持 regression 为绿**
3. **只修 consistency，不重谈大架构**

也就是说：

> 当前最正确的方向是守住这条单 agent 推理主链的冻结边界，而不是重新打开架构发散。 
