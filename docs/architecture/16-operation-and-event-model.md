# 16 Operation and Event Model

本文档定义 `fin` 的运行事实模型：

- `Operation`
- `Event`
- `Projection`
- 它们之间的控制关系
- M1 需要先冻结的最小集合

目标：

- 让 runtime、Web、CLI、harness、CI 共享同一套事实模型
- 让 debug、replay、health check、回归验证围绕统一真源工作

不展开：

- 最终 Rust type 命名
- 存储引擎实现细节
- HTTP/WS wire 最终 schema
- UI 组件细节

## 1. 核心判断

`fin` 的运行内核默认采用：

```text
operation -> state push -> append-only events -> projection
```

四条必须冻结的判断：

1. `Operation` 不是事实真源
2. `Event` 才是运行事实真源
3. `Projection` 不是事实，只是事件驱动的读取优化结果
4. `Web` 只消费 projection / raw events，不拥有业务语义

## 2. Operation 是什么

`Operation` 表示：

> 某个 actor / framework / user / worker / scheduler 想让系统做的一件事。

它是请求、意图、控制动作，而不是“已经发生的事实”。

### 2.1 Operation 的职责

Operation 负责表达：

- 想做什么
- 目标对象是谁
- 限制条件是什么
- 本次动作属于哪条 trace
- 超时、重试、幂等语义是什么

### 2.2 Operation 最小字段

M1 建议最小包含：

- `operation_id`
- `operation_type`
- `timestamp`
- `source`
- `sender_id`
- `trace_id`
- `sequence`
- `protocol_version`
- `correlation_id?`
- `causation_id?`
- `session_id?`
- `task_id?`
- `topic_thread_id?`
- `dispatch_id?`
- `worker_id?`
- `idempotency_key?`
- `timeout_ms?`
- `payload`

### 2.3 Operation 生命周期

Operation 自己也有控制态，但这些控制态最好也通过事件表达，而不是偷偷改内存：

- `submitted`
- `validated`
- `accepted`
- `rejected`
- `started`
- `waiting`
- `completed`
- `failed`
- `cancelled`
- `timed_out`

### 2.4 Operation sequence 语义

`operation.sequence` 可以存在，但建议理解为：

- operation 在其提交 scope 内的顺序
- 服务于提交侧调试与局部排查
- 不作为 subscription cursor 的最终真源

因此：

- operation sequence 有价值
- 但 event sequence 才是事件消费与回放的默认排序真源

## 3. Event 是什么

`Event` 表示：

> 系统中已经发生的事实。

Event 必须是 append-only。

允许：

- append
- archive
- snapshot
- projection

不允许：

- 覆盖写历史
- 静默改历史
- 用 projection 倒推后反写事实流

### 3.1 Event 的职责

Event 用于：

- 记录关键状态推进
- 作为 Web / CLI / harness / CI 的共同真源
- 作为 replay 的输入
- 作为 health / timeout / recovery 的观察依据

### 3.2 Event 最小 envelope

M1 建议统一 envelope：

- `event_id`
- `event_type`
- `timestamp`
- `source`
- `sender_id`
- `trace_id`
- `protocol_version`
- `correlation_id?`
- `causation_id?`
- `session_id?`
- `task_id?`
- `topic_thread_id?`
- `dispatch_id?`
- `worker_id?`
- `operation_id?`
- `sequence`
- `task_sequence?`
- `severity`
- `debug_visibility`
- `payload`

补充说明：

- `severity`：`info | warn | error | critical`
- `debug_visibility`：`normal | important | verbose | internal`
- `source`：来源模块
- `sender_id`：具体发送者实例
- `protocol_version`：当前 envelope / schema 协议版本

这样 Web 与 CLI 可以做分层过滤，不会被噪声淹没。

### 3.3 Event sequence 语义

`event.sequence` 冻结为：

- append-only event log sequence
- replay 输入顺序
- subscription cursor 的默认锚点

`event.task_sequence?` 是可选局部序：

- 只在 task 粒度下使用
- 适合 task timeline / digest / closure continuity
- 不替代 `event.sequence`

## 4. Projection 是什么

`Projection` 表示：

> 基于事件流构建出来的当前读取视图。

Projection 的作用：

- 给 Web 提供当前状态
- 给 CLI 提供快速查询
- 给 harness / CI 提供断言入口
- 给 debug server 提供聚合快照

Projection 的硬规则：

1. 只能消费事件
2. 不能发明新业务语义
3. 可以缓存，但不能覆盖事件事实
4. 出错时要能回到 raw events 复核

## 5. 三者关系

标准关系如下：

```text
operation submitted
  -> framework validate / route / execute
  -> append events for each key step
  -> projector consumes events
  -> web/cli/harness read projection or raw events
```

因此：

- operation 进入系统
- framework 推进状态机
- event 记录事实
- projection 供观察和断言

## 6. Event family 分层

M1 推荐按语义族命名，而不是按零散模块命名。

### A. Control Events

- `operation.submitted`
- `operation.accepted`
- `operation.rejected`
- `operation.started`
- `operation.completed`
- `operation.failed`
- `operation.timed_out`

### B. Session / Routing Events

- `session.tentative_opened`
- `session.formalized`
- `task.created`
- `topic.bound`
- `topic.switched`
- `topic.side_entered`

### C. Inference Events

- `inference.started`
- `provider.request_started`
- `provider.response_received`
- `inference.completed`
- `inference.failed`

### D. Tool Events

- `tool.started`
- `tool.snapshot_appended`
- `tool.completed`
- `tool.failed`

### E. Recording Events

- `progress.updated`
- `execution_note.appended`
- `digest.finalized`
- `artifact.published`

### F. Orchestration Events

- `dispatch.created`
- `dispatch.assigned`
- `heartbeat.missed`
- `lease.reclaimed`
- `recovery.started`
- `retry.scheduled`

### G. Debug / Replay Events

- `snapshot.created`
- `trace.exported`
- `replay.started`
- `replay.completed`
- `replay.assertion_failed`

## 7. Side effect 三段式规则

所有关键 side effect 默认都必须有：

- `started`
- `completed`
- `failed`

至少适用于：

## 8. Event 默认采用 producer / consumer 模型

`fin` 的事件模型不只是“记录日志”，而是默认按：

```text
producer -> append event -> subscriber / consumer -> projection / channel / debug
```

来设计。

冻结判断：

1. 事件先是运行事实，其次才是调试素材
2. producer 负责发出事实，不负责决定谁消费
3. consumer 只能订阅与消费事件，不能反向改写历史
4. debug 只是某一种 consumer，不是特殊旁路

## 8.1 Producer 是什么

producer 可以是：

- runtime loop
- provider gateway
- tool executor
- orchestrator
- health monitor
- harness / replay engine

producer 的职责只有两类：

1. 在关键状态推进点产出结构化事件
2. 保证 trace / operation / entity 关联完整

producer 不负责：

- UI 展示语义
- 特定 channel 的消费策略
- projection 物化细节

## 8.2 Consumer 是什么

consumer 可以是：

- projector
- Web debug console
- CLI tail/filter
- harness assertion runner
- replay engine
- channel bridge / notification dispatcher
- future external subscriber

consumer 的职责：

1. 订阅关心的事件族
2. 做读取、过滤、聚合、断言、转发
3. 形成 projection / timeline / diagnostics / channel payload

consumer 不能成为第二真源。

## 8.3 Subscription 是默认边界

事件消费默认通过 subscription/filter 完成，而不是让 producer 针对每个消费者单独写逻辑。

推荐理解：

```text
append-only event log
  -> consumer subscribes by event family / entity refs / visibility
  -> consumer builds its own view
```

因此后续：

- debug 订阅 debug 需要的事件
- Web 页面订阅当前页面关心的事件
- 不同 channel 订阅不同 event family
- harness 订阅断言需要的事件

它们共享同一套事实链，而不是复制多套架构。

## 8.4 Event family 与 consumer 的关系

推荐按事件族做订阅，而不是按 UI 页面硬编码：

- `provider.*`：provider gateway / model 执行相关
- `tool.*`：工具调用与快照
- `dispatch.*` / `heartbeat.*` / `recovery.*`：协作与健康
- `progress.*` / `execution_note.*` / `digest.*`：记录层
- `snapshot.*` / `replay.*`：调试与回放

这样同一事件族可以被：

- Web debug
- CLI
- harness
- channel bridge

以不同方式消费，但共享同一真源。

## 8.5 M1/M2 的演进约束

M1 阶段可以先是：

- append-only event files
- projector 直接读取
- Web/CLI 轮询当前快照

M2 及以后再进入：

- 真实 subscription registry
- WS push / channel fan-out
- per-consumer filters
- remote event subscribers

但无论阶段如何，producer / consumer 的架构边界从现在开始冻结。

- provider call
- tool call
- digest finalize
- artifact publish
- dispatch assign
- recovery action

这是 debug、超时检测、replay 对齐的基础要求。

## 8. 错误事件规则

错误不能只是一句日志。

错误事件至少应有：

- `error_code`
- `message`
- `scope`
- `related_ids`
- `retryable`
- `source_stage`

这样 recovery / Web / CI 才能做结构化判断。

## 9. Framework 与 Model 的职责分界

模型可以输出：

- routing feedback
- note candidate
- summary candidate
- intent / preview / confidence

但真正写入系统事实的动作必须由 framework 执行。

也就是：

- framework 写 operation acceptance / rejection
- framework 写 progress / event / digest / artifact append
- model 不直接拥有事实流写权限

## 10. M1 最小 operation 集合

M1 不求完整，只先冻结最小垂直切片所需的 operation。

### M1-A Routing / Session

- `UserInputOp`
- `RouteInputOp`
- `CreateTaskOp`

### M1-B Inference / Recording

- `StartInferenceOp`
- `InvokeProviderOp`
- `AppendProgressOp`
- `AppendExecutionNoteOp`
- `FinalizeDigestOp`

### M1-C Debug / Export

- `CreateSnapshotOp`
- `ExportTraceOp`

## 11. M1 最小 projection 集合

M1 projector 至少要提供：

- current session
- current task
- current topic
- current progress block
- latest execution note
- latest digest
- latest provider activity
- latest warning / timeout / error

## 12. 当前冻结的硬规则

当前可视为架构真源的规则：

1. `Event` 是运行事实真源
2. `Operation` 是请求，不是事实
3. `Projection` 只能消费事件，不得自创业务真相
4. 所有关键状态推进都必须发结构化事件
5. 所有关键 side effect 都必须有 started / completed / failed 三段事件
6. 错误必须结构化，不允许只留文本日志
7. Web / CLI / harness / CI 必须围绕同一套事件模型工作

## 13. 当前非目标

留到模块实现阶段再定：

- 最终 Rust enum / struct 名称
- event store 具体实现
- sequence 分配策略
- 事件压缩 / 分桶策略
- 复杂多机同步下的事件复制协议
