# 28 Conversation Richness, Tool Semantics, and Interrupt Model

本文档冻结三件紧邻但容易混淆的事情：

1. 正常会话前台与 debug 后台的关系
2. reasoning / control / tool 执行结果在 UI 中应该如何被框架化渲染
3. pause/resume / pending input / interrupt / parallel execution 在当前阶段与后续阶段的边界

目标：

- 不让 Web 只停留在“debug 专用页”
- 不让 reasoning 渲染滑向原始 CoT 暴露
- 不让“并行推理”在缺少状态机与 ownership 的前提下被提前做乱

---

## 1. 核心判断

`fin` 的 Web 不应被理解成两个割裂系统：

- 一个 normal chat
- 一个 debug console

更准确的定义是：

```text
同一份 session render truth
  -> 不同 render richness
  -> 不同 audience / use case
```

冻结判断：

1. 正常会话与 debug 不能拥有两套消息真源
2. 所有会话渲染都必须来自 session artifacts
3. debug 只是 richest mode，不是独立真相系统
4. reasoning 只能以结构化 reasoning view 暴露，不能默认暴露原始 chain-of-thought
5. 工具执行结果必须以“有意义的工具语义”渲染，而不是裸 JSON
6. pause/resume 与 parallel execution 当前仍是架构保留，不是已实现能力

---

## 2. Render Truth：前台与后台共用同一真源

前台正常会话与 debug 后台都必须消费同一套 session truth：

- `conversation/messages.json`
- `events/stream.jsonl`
- `control/latest.json`
- `notes/latest.json`
- `digests/recent_digests.json`
- `context/recent_contexts.json`
- 后续的 tool execution / reasoning view materialized records

结论：

> 前台不是“轻量版伪数据”，后台也不是“另一套工程专用数据”。
> 二者都只是同一份 session truth 的不同 richness 渲染。

---

## 3. Render Richness Levels

`fin` 的会话前台建议冻结三档 richness。

### 3.1 Minimal

只显示：

- user message
- assistant visible answer

适合：

- 普通聊天
- 移动端
- 非工程态使用

不显示：

- control feedback
- reasoning summary
- tool execution details
- raw event timeline

### 3.2 Rich

这是“正常会话中的可选增强模式”，不是 debug 专属。

显示：

- user / assistant 消息
- 本轮 reasoning summary
- control block 解析结果
- tool execution cards
- 当前 task/topic continuity 摘要

适合：

- 持续协作开发
- 需要知道 agent 在干什么
- 希望看到框架判断但不想进入 full debug

### 3.3 Full Trace

这是当前 `web-debug` 主模式所在层级。

显示：

- request / response
- structured context blocks
- execution note
- digest
- provider details
- event timeline
- raw / normalized debug payloads

适合：

- runtime 调试
- harness / replay 对照
- prompt / context / event 链诊断

---

## 4. Rich Mode 不是 debug pane，而是正常会话增强层

Rich mode 的定位必须冻结：

1. 它属于正常 conversation UI
2. 它的开关是 render richness，不是“进入 debug 系统”
3. 它主要面向：
   - 我当前这轮在做什么
   - 为什么这么做
   - 工具做了什么
   - 当前 task 是否延续

因此 Rich mode 的核心区块建议冻结为：

### A. Answer

- 用户可见回答

### B. Reasoning Summary

- 本轮意图理解
- 决策摘要
- 当前风险 / blocker
- 下一步

### C. Control

- `is_continuation`
- `is_simple_query`
- `candidate_task_id`
- `candidate_topic_thread_id`
- `continuity_confidence`
- `topic_shift_confidence`
- `simple_query_confidence`
- `reason`

### D. Tools

- 每个工具执行卡片
- 工具用途
- 操作对象
- 成功/失败
- 输出摘要

### E. Task / Session

- 当前 session
- 当前 task
- 当前 topic / continuity 摘要

---

## 5. Reasoning 的渲染边界

`fin` 中“渲染 reasoning”不等于渲染原始 CoT。

冻结为三层：

### 5.1 User-visible answer

这是最终给用户看的自然语言回答。

### 5.2 Structured Reasoning View

这是允许被 UI 渲染的 reasoning 层。

它来自框架可持久化对象，例如：

- `ExecutionNote`
- `Digest`
- `ControlFeedback`
- tool execution summaries
- 关键事件归纳

它可以回答：

- 这轮在判断什么
- 为什么继续当前 task / 为什么要切 topic
- 为什么要调用工具
- 当前的 blocker / next step 是什么

### 5.3 Raw hidden reasoning

默认不进入：

- session render truth
- user-facing UI
- debug pane

也就是说：

> `fin` 可以有 reasoning visibility，但默认只能通过结构化 reasoning view 暴露，而不是通过裸思维流暴露。

---

## 6. Rich Tool Rendering：工具执行必须语义化

当前最小 `ToolSnapshot { tool_name, status, summary }` 不足以支撑有意义 UI。

后续需要升级为独立真源对象：

```text
ToolExecutionRecord
```

它至少要能回答：

1. 这是哪个工具
2. 工具是干什么的
3. 这轮为什么用它
4. 它操作的对象是什么
5. 成功 / 失败 / 中断
6. 它产出了什么
7. 它是否造成 side effect

Rich mode 中每个工具应渲染成语义卡片，而不是裸 JSON 展开。

推荐固定字段见：

- `docs/contracts/tool-execution-record-contract.md`

---

## 7. Reasoning View Contract

为了让 Rich mode 有稳定真源，reasoning 不能只依赖临时拼接字符串。

后续需要一个框架持久化对象：

```text
ReasoningViewRecord
```

它不是原始思维，而是框架允许渲染的 reasoning 摘要层。

推荐字段见：

- `docs/contracts/reasoning-view-contract.md`

---

## 8. 当前 M1 的真实能力边界

必须明确：

### 当前已经支持

- 单 runtime、单 closure 推理闭环
- session artifacts 落盘
- 多轮对话 history/context 装配
- control feedback 结构化解析
- Web 基于 session truth 观察

### 当前还不支持

- 中途 pause / resume
- 同一 session 内多个推理 closure 并行执行
- pending input queue
- interrupt / preemption
- 多 worker 编排执行核
- mailbox / eventbus / RPC 真正进入主 runtime

因此当前不能宣称：

- 推理核心已支持暂停恢复
- 新输入可并行推理

最多只能说：

> 架构已为 checkpoint / resume / mailbox / eventbus / interrupt / multi-worker 留边界，但 M1 尚未实现。

---

## 9. Pause / Resume 的正确归属

pause/resume 必须冻结为：

```text
framework-owned runtime state recovery
```

而不是：

- UI 层“暂停一下页面”
- provider transport 自己恢复
- 从 raw chat transcript 中猜测恢复点

### 9.1 恢复点粒度

后续只允许在明确 checkpoint 边界恢复，例如：

1. provider call 前
2. tool call 前
3. tool call 后
4. closure 边界
5. 显式 checkpoint event 之后

### 9.2 不建议做的恢复

不建议把 pause/resume 定义成：

- token-stream 中间断点继续
- UI 画面级恢复
- 靠最后一条 assistant 文本拼出“继续执行”

### 9.3 必备真源

真正实现 pause/resume 前，至少需要：

- checkpoint event
- resumable state store
- dispatch / lease / ownership state
- interrupted segment handling
- replayable task state

---

## 10. Pending Input / Interrupt Model

在真正做 parallel execution 前，应该先冻结：

### 10.1 新输入不等于立刻并行执行

如果当前 closure 正在运行，新输入建议先进入：

```text
PendingInputQueue
```

而不是立刻开第二条并发推理。

### 10.2 框架后续应该有的判断

当新输入到来时，框架判断：

1. 入队等待当前 closure 结束
2. 触发 interrupt request
3. 作为 side topic 暂存
4. 新建 tentative session
5. 绑定已有 task / topic

### 10.3 这里的控制权

模型可以通过 control block 提供判断信号，例如：

- continuity
- topic shift
- simple query

但最终：

- 是否中断
- 是否切换
- 是否排队
- 是否新建 session

都由框架控制，不由模型直接决定。

---

## 10.4 特殊并行请求：Status Probe / State Inquiry

有一类请求必须从普通新输入中单独切出来：

> 用户在任务正在推理/执行时，询问“当前状态是什么、做到哪了、卡在哪、现在在干嘛”。

这类请求冻结为：

```text
parallel inquiry request
```

它的核心要求是：

1. **不打断当前主推理**
2. **不等待当前 closure 完成才回复**
3. **必须带特殊标记输入**
4. **回复优先来自框架已知状态，而不是让忙碌 worker 停下来重新想一遍**

### 10.4.1 为什么它算并行请求

因为它与普通用户输入不同：

- 目的不是推进当前任务语义
- 目的不是补充新的业务约束
- 目的只是读取当前执行状态

因此它不应走：

- 普通 pending input queue
- topic switch 判定
- interrupt 主链路

而应走：

```text
status-probe side path
  -> read latest framework state
  -> build status response
  -> return
```

### 10.4.2 特殊标记输入

后续框架需要显式支持一种特殊输入标记，例如概念上：

- `input_kind = status_probe`
- `parallel_class = inquiry`
- `interrupt_policy = non_interrupting`

名字后续可调整，但语义必须冻结：

1. 这是并行查询，不是主任务输入
2. 不应打断当前主执行
3. 不应被当成 topic shift 或新 task 输入

### 10.4.3 回复来源

status probe 的回复必须优先组合自框架已有真源：

- `ProgressBlock`
- `ExecutionNote`
- 最新 `ControlFeedback`
- 后续的 `ReasoningViewRecord`
- 后续的 `ToolExecutionRecord`
- 必要的最新事件时间戳

其中：

- `ControlFeedback` 提供当前任务连续性、主题、简单问题判断等结构化视角
- `ProgressBlock` 提供当前 phase / blocker / next step
- `ExecutionNote` 提供更稳定的人类可读总结
- tool records 提供“现在在干什么”的可读执行状态

也就是说：

> status probe 的回答应该是框架从最新 control/progress/note/tool state 中组装出来，而不是要求主 worker 停下当前推理再回答。

### 10.4.4 与 control block 的关系

用户要求“通过 controlblock 输出”的正确冻结方式应为：

1. 主执行链继续推进
2. 框架使用**最近一次可用的 control block**
3. 若当前 closure 尚未产出新的 control block，则明确标记：
   - 当前状态基于最近 checkpoint / 最近 closure / 最近 progress
   - freshness 是多少

所以不是要求“正在忙的模型立即额外输出一个新 control block”，而是：

> status probe 回复围绕最近可用 control block 及其配套 state 构造。

### 10.4.5 freshness 要求

status probe 回复必须带 freshness 概念，例如：

- `state_freshness = live | recent | stale | unavailable`

语义：

- `live`：当前 task 正在持续更新且刚有新 state
- `recent`：状态来自最近 checkpoint / note / progress
- `stale`：只有更早的 closure / checkpoint
- `unavailable`：还没有足够状态

这样用户不会误以为：

- 这是一条实时中断后的重新判断
- 或这是模型刚刚停下来专门回复的结果

### 10.4.6 status probe 不应造成的副作用

冻结：

1. 不关闭当前 closure
2. 不生成新的 task digest
3. 不改变当前 task ownership
4. 不触发 topic switch
5. 不把 probe 本身当作主对话历史的一轮业务输入

允许：

- 记录 probe request / probe reply 事件
- 记录用户查状态这一事实
- 在 UI 中显示这是 side-path inquiry

### 10.4.7 最小测试要求

后续实现这条能力时，至少验证：

1. 主任务正在 busy 时发送 `status_probe`
2. 主任务不被中断
3. probe 在合理 SLA 内返回
4. 返回内容来自最近 `progress/note/control/tool` 真源
5. probe 回复能看到 freshness
6. probe 不产生新的 digest closure
7. probe 不把 session / task/topic 路由打乱

这条测试必须是真实闭环测试，不是只看 unit schema。

---

## 11. Parallel Execution 的分期建议

### Phase A：Pending-first

先支持：

- 新输入入队
- 当前执行 busy 状态
- interrupt request 标记
- closure 完成后再决策下一步

不支持真正并行推理。

### Phase B：Cross-task / Cross-worker parallel

再支持：

- 同一 project 下不同 task 分配给不同 worker
- project leader 汇总多 worker 结果
- worker 各自持有 session branch / journal

### Phase C：Cross-process / Cross-network

最后接入：

- RPC ingress
- mailbox
- eventbus
- heartbeat / lease / recovery

---

## 12. Web 前台与 Debug 后台的关系

后续建议把当前 `web-debug` 的部分能力沉入通用 conversation UI：

### 通用前台

- Minimal / Rich richness switch
- 以会话推进为中心
- 不默认暴露 raw event

### Full Trace / Debug

- 作为 richest mode 或独立 developer panel
- 聚焦 request/event/context/provider 诊断

关键规则不变：

> 不管 UI 入口怎么分，所有显示都必须来自同一份 session render truth。

---

## 13. 对当前实现的约束

从现在开始，Web 相关实现建议遵循：

1. 新的 reasoning/control/tool 渲染先设计 session truth，再做 UI
2. 不允许只在前端临时拼“reasoning”字段
3. 不允许把 Rich mode 做成只在 debug 页面存在的特例
4. 不允许在没有 interrupt state machine 的情况下直接引入并行 closure
5. pause/resume 先做文档与 contract，不先做表面交互按钮

---

## 14. 当前收口结论

当前冻结结论如下：

1. `fin` 当前推理核心 **不支持真正 pause/resume**
2. `fin` 当前推理核心 **不支持同 session 的并行推理**
3. Web 下一步不应只继续做 debug，而应升级为：
   - 同一会话真源
   - 不同 richness 渲染
4. Rich mode 必须支持：
   - reasoning summary
   - control block
   - tool execution semantic cards
5. Full Trace 继续承担最强 debug 能力
6. interrupt / pending input / parallel execution 先冻结状态机与 contract，再实现
