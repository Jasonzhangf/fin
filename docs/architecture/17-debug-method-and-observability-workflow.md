# 17 Debug Method and Observability Workflow

本文档定义 `fin` 的默认调试方法。

目标不是“看日志”，而是：

- 以 raw events 为真源
- 以 projection 为当前读取视图
- 以 trace / replay / diagnostic bundle 构建闭环调试能力

## 1. 核心判断

`fin` 的 debug 默认不是文本日志驱动，而是：

```text
raw events -> timeline -> causality -> projection -> diagnostic bundle
```

日志只是辅助，人类可读；不是系统事实真源。

## 2. 五层调试法

### Layer 1：Raw Event

这是第一真源。

排障时先回答：

- 实体到底收到过哪些事件
- 顺序是否正确
- 有没有缺事件
- 有没有中断
- 有没有 started 但没有 completed / failed

默认第一入口：

- `trace_id`
- `task_id`
- `dispatch_id`
- `worker_id`
- `session_id`

### Layer 2：Entity Timeline

把事件按实体串起来，形成 timeline：

- task timeline
- dispatch timeline
- worker timeline
- provider call timeline
- topic / session timeline

这层的目标是快速定位：

- 卡在哪一步
- 谁没回应
- 是 routing 问题还是执行问题
- 是框架没推进，还是外部依赖没返回

### Layer 3：Causality Chain

这层回答：

- 这个失败由哪个 operation 触发
- 这个 event 又导致了哪些后续 event
- 哪条链路在中间断开

因此必须保留：

- `trace_id`
- `operation_id`
- `causation_id`
- `correlation_id`

建议理解：

- `trace_id`：同一条大链路
- `operation_id`：本次动作本身
- `causation_id`：直接原因
- `correlation_id`：一组并行或同源动作归组

### Layer 4：Current Projection

这是给人快速看的当前状态层。

默认应能看到：

- current session
- current task
- current topic
- current progress block
- latest execution note
- latest digest
- active dispatches
- worker health
- last provider calls
- timeout / warning / stuck indicators

这层用于快速判断“现在系统怎么看自己”。

### Layer 5：Diagnostic Bundle

这是复盘与交接层。

每次异常、超时、失败或关键结束时，框架都应能导出调试包，至少包含：

- raw events 片段
- current projection snapshot
- recent progress blocks
- recent execution notes
- latest closure / digest
- error sample / provider metadata
- recovery attempts

用途：

- 用户反馈问题复现
- 子 agent 卡死后复盘
- CI 失败取证
- provider 调用异常分析
- 人工交接排查

## 3. Debug 默认顺序

调试顺序默认固定：

1. 先看 raw events
2. 再看 entity timeline
3. 再看 causality chain
4. 再看 current projection
5. 最后才看 Web 页面细节或文本日志

这条顺序是硬规则。

## 4. Web / CLI / Harness 的职责

### Web

Web 是观察窗口，适合：

- live event stream
- timeline
- trace inspector
- current state projection
- timeout / warning overview

但 Web 不是执行真源。

### CLI

CLI 是快速入口，适合：

- tail events
- filter by trace / task / dispatch
- show current projection
- export diagnostic bundle

### Harness

Harness 是验证入口，适合：

- replay
- fault injection
- assertion
- regression evidence generation

### CI

CI 只消费结构化证据，不直接相信人工描述。

## 5. Debug 与日志的关系

日志可以保留，但只能作为辅助层。

原则：

1. 日志不是事实真源
2. 关键结论必须能回到 raw events
3. 没有 raw event 支持的日志结论不算完成调试

## 6. Timeout / Health / Recovery 调试法

对于卡住、掉线、超时问题，默认看三件事：

1. `heartbeat` 有没有断
2. `progress` 有没有停止推进
3. `lease / dispatch` 有没有被回收、重试或升级

推荐观察链：

```text
worker heartbeat
-> dispatch timeline
-> last progress block
-> recovery / retry events
-> projection warning state
```

## 7. M1 Debug MVP

M1 不做完整调试平台，只要求最小闭环：

### 7.1 必须有

1. 单 runtime operation 入口
2. append-only event stream
3. 最小 projector
4. Web Debug MVP
5. CLI debug entry
6. replay smoke

### 7.2 Web Debug MVP 至少展示

- live event stream
- current projection
- trace / task / session filter
- latest error / timeout / warning

### 7.3 CLI 至少支持

- 查看事件流
- 按 trace / task / dispatch 过滤
- 查看 current projection
- 导出 diagnostic bundle

### 7.4 Replay 至少支持

- 读取 event stream
- 重建最小 projection
- 验证 event 模型是否自洽

## 8. 模块级 debug 设计要求

每个功能模块默认都要回答：

1. 这个模块的 raw events 是什么
2. 这个模块的主 timeline 是什么
3. 这个模块的 causality 通过什么 ID 串起来
4. 这个模块的 projection 给谁消费
5. 这个模块的异常如何导出 diagnostic bundle

如果这些都没有答案，就说明这个模块还不具备可维护的 debug 设计。

## 9. 当前冻结的调试硬规则

1. debug 先看 raw events，再看 projection，最后才看 UI
2. 没有 raw event，不下 runtime 结论
3. 没有 trace / causality 关联，不算可调试
4. 没有 replay 或等价回放，不算闭环修复
5. 文本日志不是事实真源
6. Diagnostic bundle 是异常复盘与交接的标准载体

## 10. 当前非目标

留到后续模块阶段再定：

- Web 页面最终布局
- 时序图渲染方式
- full time-travel UI
- replay 可视化脚本细节
- 复杂 cluster 级 health dashboard 公式