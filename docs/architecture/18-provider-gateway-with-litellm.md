# 18 Provider Gateway with LiteLLM and Event Pub/Sub

本文档冻结 `fin` 的真实 provider 架构方向。

核心前提来自当前项目决策：

1. runtime 只有一个统一核心 loop
2. 不同 agent / role / worker 可以配置不同 provider policy
3. `fin` 不自己维护多协议 adapter，统一通过 LiteLLM gateway 接入外部模型
4. 请求是 operation，响应是 event，二者分阶段隔离
5. event 默认按 producer / consumer + subscription 设计
6. provider 当前不做 routing，只按显式 `provider.model` 发请求
7. provider 当前不做隐式切换；多 provider 路径只允许显式配置

---

## 1. 核心判断

`fin` 的 provider 架构默认采用：

```text
agent/runtime policy
  -> inference operation
  -> LiteLLM gateway execution
  -> provider events
  -> projection / debug / channel subscribers
```

冻结判断：

1. LiteLLM 是外部 gateway，不是业务真源
2. Rust runtime 仍然是执行真源
3. provider 不拥有第二套 task/session/context 状态机
4. debug 与 channel fan-out 使用同一事件架构，只是订阅目标不同
5. 当前 provider 选择策略只支持显式 priority path

---

## 2. 统一核心 loop 的边界

统一核心 loop 只保留一份，负责：

- task / closure 推进
- context view 组装
- tool orchestration
- progress / execution note / digest
- operation 提交与 event 消费

provider gateway 不负责：

- task state machine
- session / topic 治理
- digest / rebuild
- 多 worker 协作状态

provider gateway 只负责：

- 接收 inference operation
- 调 LiteLLM
- 产出 provider 事实事件

---

## 3. 多 agent provider policy

不同 agent 可以绑定不同 provider，但绑定必须是 framework policy，而不是自由漂移。

推荐绑定层级：

```text
RoleProfile -> AgentRuntime -> InferenceOperation override
```

### 3.1 RoleProfile 默认

例如：

- `project_leader` -> 更强模型
- `coder_worker` -> coding model
- `summarizer` -> 低成本模型

### 3.2 AgentRuntime 实例化

worker 启动时继承 role 默认 provider policy。

### 3.3 Operation 级 override

某次 operation 允许 override：

- provider
- model
- timeout tier
- stream / non-stream

但 override 只能在 policy 允许范围内进行。

### 3.4 显式 provider path

当前 `fin` 不做“根据请求内容自动路由到最优模型”的 provider routing。

允许的是 framework 显式配置 provider path：

```text
[provider.model, provider.model, ...]
```

当前阶段冻结：

1. path 中的元素是显式 `provider.model`
2. 当前只支持 `priority` 选择方式
3. framework 为本次 operation 选中一个实际目标
4. 不做内容感知路由
5. 不做健康分数路由
6. 不做成本感知路由

未来若支持更多策略，也必须作为独立可插拔模块，而不是写死在 gateway client 中。

---

## 4. 为什么使用 LiteLLM

选择 LiteLLM 的原因不是让它成为主框架，而是把协议适配复杂度外包出去。

LiteLLM 负责：

- 多模型 / 多协议接入
- 对外统一 HTTP API
- 下游 provider transport 兼容

`fin` 自己保留：

- provider policy
- operation / event truth
- debug / replay / diagnostics
- health / timeout / retry 的上层框架语义

### 4.1 LiteLLM 的边界

LiteLLM 不应隐式取代 `fin` 的运行事实模型。

默认要求：

1. fin 明确指定 provider/model target
2. LiteLLM 不负责替 `fin` 做 provider routing 判断
3. LiteLLM 不负责替 `fin` 做隐式切换
4. fin 必须能把 LiteLLM 的执行结果正规化为结构化事件

### 4.2 当前不做 routing / implicit switch

当前阶段冻结：

1. provider 请求只根据显式 `provider.model` 发送
2. 不做自动 routing
3. 不做隐式切换
4. 若配置了多个 provider path，只允许按 `priority` 顺序选择一个发送目标

---

## 5. Operation 与 Event 的分阶段隔离

这是 provider 架构的硬规则。

### 5.1 Phase 1: Operation Planning

runtime 只决定：

- 谁发起
- 用哪个 provider policy
- 目标 model 是什么
- 输入 context 是什么
- timeout / retry / stream 策略是什么

这时仍然没有“响应”。

### 5.2 Phase 2: Gateway Execution

LiteLLM client 执行网络调用：

- build gateway request
- send request
- wait response / stream

这一层不拥有 task 业务语义。

### 5.3 Phase 3: Event Materialization

gateway 结果被转成结构化事件：

- sent
- delta
- response_received
- normalized
- completed
- failed

### 5.4 Phase 4: Runtime Reaction

runtime 只消费 event，并据此：

- 更新 progress
- 写 execution note
- 决定下一轮推理 / tool / closure

因此：

- operation 不是 response schema
- event 不是 future request schema
- 两者通过 execution phase 隔离

---

## 6. Provider operation 模型

provider 请求在框架中应被视为一种 operation。

推荐最小字段：

- `operation_id`
- `trace_id`
- `session_id?`
- `task_id?`
- `worker_id?`
- `agent_role`
- `provider_selection`
- `provider_path`
- `provider_strategy`
- `model_target`
- `input_blocks`
- `context_view_ref`
- `tool_policy`
- `timeout_policy`
- `retry_policy`

operation 中不应直接包含：

- response text
- finish reason
- usage final
- tool result final

这些都属于 event。

---

## 7. Provider event family

provider 返回的事实必须被正规化为统一 event family。

推荐最小事件族：

- `provider.operation_accepted`
- `provider.dispatch_started`
- `provider.gateway_request_sent`
- `provider.gateway_stream_delta`
- `provider.gateway_response_received`
- `provider.response_normalized`
- `provider.completed`
- `provider.failed`

失败建议再细分：

- `provider.failed.auth`
- `provider.failed.timeout`
- `provider.failed.http_status`
- `provider.failed.transport`
- `provider.failed.parse`
- `provider.failed.route`
- `provider.failed.capability_mismatch`
- `provider.failed.path_exhausted`

---

## 8. Event producer / consumer 架构

provider 事件默认也遵循整个系统的 producer / consumer 模型。

```text
gateway/runtime/tool/orchestrator -> produce events
  -> append-only event stream
  -> subscribers consume by family/filter
```

### 8.1 Producer

可能的 producer：

- runtime
- provider gateway client
- tool executor
- health monitor
- replay engine

producer 只负责发事实，不关心谁消费。

### 8.2 Consumer

可能的 consumer：

- Web debug
- CLI tail/filter
- projector
- harness assertions
- channel bridge
- future external subscribers

consumer 只负责订阅与聚合，不得回写事实。

---

## 9. 为什么 debug 与 channel fan-out 共用一套架构

好处是：

1. debug 不再是特殊分支
2. channel/notification 不需要第二套 pipeline
3. Web / CLI / harness / external channel 都可以消费同一事实链
4. 未来 cross-process / cross-node 时，事件复制与订阅模型更自然

因此后续：

- debug 页面可以订阅 `provider.*` + `progress.*`
- 通知通道可以只订阅 `digest.*` + `provider.failed.*`
- cluster/leader 面板可以订阅 `dispatch.*` + `heartbeat.*`

它们只是订阅目标不同，而不是架构不同。

---

## 10. 调试与原始 payload 规则

真实 payload 不可裁剪语义，但 event 不应直接塞入完整大 body。

推荐分层：

### event 中保存

- provider name
- target model
- actual model（若 LiteLLM 返回）
- request id / trace id
- status code
- duration
- usage
- redacted preview
- raw sample path

### 诊断目录保存

- raw request sample
- raw response sample
- gateway error body
- stream transcript sample

推荐目录：

- `~/.fin/logs/provider/`
- `~/.fin/diagnostics/error-samples/`
- `~/.fin/diagnostics/traces/`

---

## 11. 当前建议实现顺序

### P1

先做：

- LiteLLM gateway 非流式请求
- provider operation/event 最小 contract
- provider success/failure 事件落盘
- Web 可见 provider 执行状态
- priority provider path 的静态选择

### P2

再做：

- streaming delta
- per-agent provider policy
- provider diagnostics snapshot

### P3

最后再做：

- subscription registry
- channel bridge consumer
- remote subscribers
- cross-process / cross-network event fan-out
- provider strategy plugin（后续）

---

## 12. 当前冻结的硬规则

1. 统一核心 loop 只保留一份
2. 不同 agent 可以绑定不同 provider policy
3. `fin` 不自己做多协议 adapter，统一对接 LiteLLM
4. provider 当前不做 routing，只按显式 `provider.model` 请求
5. provider 当前不做隐式切换
6. 多 provider path 当前只支持 `priority` 策略
7. 若未来扩展策略，必须作为独立可插拔模块
8. 请求是 operation，响应是 event
9. operation 与 event 必须分阶段隔离
10. event 默认按 producer / consumer + subscription 架构设计
11. debug 与 channel fan-out 共享同一事件架构
