# 44 Runtime Error Center

本文档冻结 `fin` 主推理链错误中心的唯一真源。

## 1. 目标

错误不能被 fallback、降级、默认成功、silent salvage 或 debug 层二次解释掩盖。

主路径必须是：

```text
crate-local typed error
  -> RuntimeError
  -> ErrorErr01Detected
  -> ErrorErr02SourceClassified
  -> ErrorErr03RuntimeClassified
  -> ErrorErr04SessionRecorded
  -> ErrorErr05UserVisible
  -> append-only events + session ledger + channel-safe user message
```

## 2. owning truth

- `fin-shared`：底层纯校验错误，例如 required field missing。
- `fin-config`：配置读取、映射、标准化错误。
- `fin-provider`：provider descriptor、wire、HTTP、protocol parse 错误。
- `fin-runtime`：主推理链唯一错误中心，拥有 `RuntimeError` 与 `ErrorErr*` pipeline。
- `fin-cli`：只负责命令入口与展示，不拥有 runtime failure 分类。
- `fin-debug-server` / Web：只读 error events / ledger，不生成第二错误真相。
- `fin-harness-core`：只验证错误事件和回放，不修复错误。

各 crate 可以保留本层 typed error；一旦错误进入 runtime 主链，必须归一到 `RuntimeError -> ErrorErr*`。

## 3. ErrorErr chain contract

| node | owning builder/parser | responsibility |
| --- | --- | --- |
| `ErrorErr01Detected` | `ErrorErr01DetectedBuilder` | 捕获失败事实、来源节点、时间 |
| `ErrorErr02SourceClassified` | `ErrorErr02SourceClassifiedBuilder` | 分类来源：`Input` / `Provider` / `Model` / `Tool` / `Runtime` / `Channel` |
| `ErrorErr03RuntimeClassified` | `ErrorErr03RuntimeClassifiedBuilder` | runtime 决策：`Retryable` / `Blocked` / `Failed` |
| `ErrorErr04SessionRecorded` | `ErrorErr04SessionRecordedBuilder` | 写入 event id 与 ledger path |
| `ErrorErr05UserVisible` | `ErrorErr05UserVisibleBuilder` | 生成 channel-safe 用户可见错误 |

规则：

1. 每个节点只能由对应 builder/parser 产生。
2. 只允许相邻节点转换。
3. 禁止 `impl From<ErrorErr*>`、跨节点 shortcut、重复 DTO。
4. 节点编号是 contract；禁止 `03a`、`03_1`、`v2` 临时编号。

## 4. Event contract

失败不是旁路。runtime 主链失败必须产生结构化事件：

- `error.detected`
- `error.source_classified`
- `error.runtime_classified`
- `error.session_recorded`
- `error.user_visible_prepared`
- 对应业务族失败事件，例如 `operation.failed`、`inference.failed`、`provider.failed`、`tool.failed`

所有 error event 必须携带：

- `operation_id`
- `trace_id`
- `source_node`
- `source_class`
- `runtime_decision`
- `ledger_path`
- `causation_id?`

Web/debug/projection 只能消费这些事件，不得自行推断失败类型。

## 5. No fallback rules

禁止：

1. provider/model/tool/runtime 错误 fallback 成成功响应。
2. 缺失配置用隐式默认值继续跑。
3. invalid model output 被 salvage 成 completed truth。
4. tool failure 被写成成功 tool result。
5. CLI/debug 捕获 runtime error 后重新分类并覆盖 runtime decision。
6. 只写日志不写 event / ledger。
7. 返回 `Ok` 同时把错误塞进 debug 字段。

允许：

1. 明确的 retry，但 retry attempt 必须记录；retry limit 后进入 `ErrorErr*`。
2. channel-safe message 可以裁剪敏感细节，但不能改变错误语义。
3. debug snapshot 可以裁剪内部观测数据，但真实 request/response payload 语义不可裁剪。

## 6. Mainline integration target

`M1Runtime::run_closure` 目标结构：

```text
run_closure(operation, provider)
  -> run_closure_inner(operation, provider)
  -> Ok(ClosureRun)
  -> Err(RuntimeError)
     -> map_runtime_error_through_error_pipeline
     -> append error events
     -> write session ledger
     -> return explicit failed ClosureRun or explicit RuntimeError with recorded receipt
```

不得存在多个 runtime failure 出口。`?` 可以在 `run_closure_inner` 内传播 typed error，但 outer runtime boundary 必须统一映射到 `ErrorErr*`。

## 7. Validation gates

最小红线测试：

1. provider HTTP 500 进入 `Provider -> Failed`，写 `provider.failed` + `error.*` events。
2. missing config 进入 `Input -> Failed`，不使用隐式默认 provider/model。
3. invalid model output retry limit 后进入 `Model -> Failed`，不生成 completed truth。
4. tool execution failure 进入 `Tool -> Failed`，不伪造 successful tool result。
5. `rg` 禁止主路径出现 `fallback_ok`、`treat_invalid_as_success`、`impl From<ErrorErr`。
6. `rg map_runtime_error_through_error_pipeline` 必须显示主链调用点，不只是函数定义。

## 8. Current known gap

截至 2026-06-04 审查：

- `rust/crates/runtime/src/error_pipeline.rs` 已有 `ErrorErr01..05` 骨架。
- `rust/crates/runtime/src/closure_runtime.rs::map_runtime_error_through_error_pipeline` 已存在。
- `map_runtime_error_through_error_pipeline` 尚未成为主链唯一出口；`M1Runtime::run_closure` 仍直接返回 `Result<ClosureRun, RuntimeError>`。
- 因此当前状态只能判定为“错误中心骨架存在”，不能判定为“唯一错误中心完成”。
