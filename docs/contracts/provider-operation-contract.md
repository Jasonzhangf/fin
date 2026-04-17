# Provider Operation Contract

本文档定义 `fin` 中 provider 请求侧的 contract 真源。

目标：

- 明确 provider 请求属于 operation，而不是事件
- 明确 operation 与 response/event 的边界
- 为 LiteLLM gateway 执行层提供稳定输入
- 为后续 provider path / strategy 插件保留稳定边界

---

## 1. 核心判断

provider request 在 `fin` 中必须被建模为 operation。

operation 只表达：

- 想请求什么模型能力
- 由谁发起
- 使用什么 policy
- 当前使用哪条 provider path
- 当前使用什么 provider selection strategy
- 超时 / 重试 / stream / tool 等约束是什么

operation 不表达：

- 模型已经返回了什么
- 最终 finish reason
- usage final
- tool 结果 final

---

## 2. 最小字段

推荐最小字段集合：

- `operation_id`
- `operation_type`
- `submitted_at`
- `trace_id`
- `source`
- `session_id?`
- `task_id?`
- `dispatch_id?`
- `worker_id?`
- `agent_role`
- `provider_name`
- `provider_path`
- `provider_strategy`
- `model_target`
- `stream`
- `input_blocks`
- `context_view_ref?`
- `tool_policy?`
- `timeout_ms?`
- `retry_policy?`
- `payload`

---

## 3. payload 语义

`payload` 应描述请求参数，而不是协议细节。

建议包括：

- user/assistant/tool message blocks
- system prompt
- tool specs
- gateway metadata
- budget / latency tier

当前 provider 相关约束：

- `provider_path` 表示显式候选 `provider.model` 路径
- `provider_strategy` 当前只允许 `priority`
- request build 阶段只选择本次实际目标，不做动态 routing
- 失败后不隐式切下一个 provider

不建议在 contract 真源中直接绑定：

- OpenAI wire shape
- Anthropic wire shape
- LiteLLM 私有 transport 细节

这些属于 gateway execution layer。

---

## 4. 与 LiteLLM 的关系

provider operation 进入 LiteLLM 之前，需要经过单独 execution 映射阶段：

```text
ProviderOperation
  -> GatewayRequestBuilder
  -> LiteLLM HTTP Request
```

因此：

- operation 不是 LiteLLM 请求体本身
- LiteLLM 请求体可以变化
- operation contract 仍保持稳定

## 4.1 当前策略冻结

当前冻结：

1. LiteLLM 只作为统一 gateway
2. `fin` 自己决定实际发送到哪个 `provider.model`
3. 当前仅支持 `priority` 路径选择
4. 当前不做 fallback

未来若支持：

- round robin
- weighted
- cost aware
- health aware

也必须在 strategy plugin 中扩展，而不是修改 operation/event 真源语义。

---

## 5. owning layer

此 contract 的 owning layer：

- `docs/contracts/`
- `rust/crates/contracts`
- `rust/crates/provider`

runtime / web / harness 只能消费，不应复制第二份定义。
