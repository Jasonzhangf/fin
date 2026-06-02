# 44 Pipeline Unique Type and Error Chain

本文档冻结 fin 内部所有关键流水线的数据结构命名、请求/响应双向链条、错误处理连接关系。

目标：

1. 每个数据流节点只有一个唯一类型。
2. 类型名能直接表达数据方向、链路位置、节点序号和业务含义。
3. 请求链、响应链、错误链都必须显式相连，禁止隐式 fallback 或跨层 shortcut。
4. UI / channel / provider / runtime 不复制彼此语义。

---

## 1. 全局命名规则

所有关键链路类型使用固定模板：

```text
<Domain><Direction><NN><Node>
```

字段含义：

1. `Domain`：链路域。
   - `Input`：用户输入清洗链。
   - `Reason`：推理逻辑链。
   - `Hub`：provider wire 调用链。
   - `Feedback`：模型反馈响应清洗链。
   - `Error`：错误处理链。
2. `Direction`：数据方向。
   - `In`：外部进入内部。
   - `Req`：向下游发起请求。
   - `Resp`：从下游返回响应。
   - `Err`：错误传播与归一化。
3. `NN`：两位节点序号，沿数据流方向递增。
4. `Node`：节点语义名，必须具体；禁止只叫 `Request` / `Response` / `Payload` / `Data`。

例子：

```text
InputIn01ChannelRaw
ReasonReq04RenderedInput
HubResp05Process
FeedbackResp06SessionMaterialized
ErrorErr03RuntimeClassified
```

命名硬规则：

1. 同一节点不得有第二个同义类型。
2. 同一类型不得跨多个节点复用。
3. 只允许相邻节点转换。
4. 每个节点只有一个 owning builder/parser。
5. 阶段类型默认不对非 owning layer 导出；跨层只导出相邻 output 或 durable record。

---

## 1.1 节点编号稳定性与扩展规则

节点编号是架构 contract，不是普通代码顺序号。

硬规则：

1. 已发布或已被下游消费的节点编号不可重排、不可复用、不可改语义。
2. 默认禁止在两个既有节点中间插入新节点。
3. 新能力优先并入 owning 既有节点的内部 block / field / validator / parser，而不是新增流水线节点。
4. 只有当新能力改变跨层 ownership、durable truth 边界或错误处理边界时，才允许新增节点。
5. 新增节点必须追加在当前链尾，或开启新的 minor chain version；不得把 `03` 和 `04` 之间强行插成新的 `03.5`。
6. 禁止使用小数编号、字母后缀、临时编号表达正式节点，例如 `03a` / `03_1` / `03_5`。

扩展优先级：

```text
既有节点内部字段/子 block
  -> 既有节点 validator/parser 增强
  -> 链尾追加新节点
  -> 新 chain version
```

允许链尾追加：

```text
FeedbackResp07ChannelRender
  -> FeedbackResp08DeliveryReceipt
```

不允许中间插入：

```text
FeedbackResp03UserVisible
  -> FeedbackResp03aSafetyFiltered
  -> FeedbackResp04ControlFeedback
```

若确实需要改变中段语义，必须建立新版本链：

```text
FeedbackV2Resp01ModelRaw
  -> FeedbackV2Resp02TaggedBlocks
  -> FeedbackV2Resp03SafetyClassified
  -> FeedbackV2Resp04UserVisible
```

新版本链要求：

1. 写明从旧链到新链的迁移计划。
2. 明确旧链废弃和物理删除时机。
3. 禁止长期 V1/V2 双主路径并存。
4. 红测必须证明调用方不能混接 V1/V2 节点。

设计偏好：

> fin 当前架构优先避免中间新增节点；节点设计应足够粗粒度，节点内部可演进，节点之间不可频繁重排。

---

## 2. 总体双向链条

fin 的完整请求/响应链必须按以下方向连接：

```text
User / Channel
  -> InputIn01ChannelRaw
  -> InputIn02Normalized
  -> InputIn03Operation
  -> InputIn04SessionBound
  -> InputIn05ReasoningSeed
  -> ReasonReq01Seed
  -> ReasonReq02ContextPlan
  -> ReasonReq03BudgetedContext
  -> ReasonReq04RenderedInput
  -> ReasonReq05ProviderCall
  -> HubReq01Inbound
  -> HubReq02Process
  -> HubReq03Outbound
  -> Provider Wire
  -> HubResp04Inbound
  -> HubResp05Process
  -> HubResp06Outbound
  -> ReasonResp06ModelOutput
  -> FeedbackResp01ModelRaw
  -> FeedbackResp02TaggedBlocks
  -> FeedbackResp03UserVisible
  -> FeedbackResp04ControlFeedback
  -> FeedbackResp05ToolIntent
  -> ReasonResp07ParsedContract
  -> ReasonResp08RuntimeDecision
  -> ReasonResp09Closure
  -> FeedbackResp06SessionMaterialized
  -> FeedbackResp07ChannelRender
  -> User / Channel
```

连接规则：

1. `Input*` 只负责输入清洗与 operation 建立，不负责模型上下文和 provider wire。
2. `ReasonReq*` 只负责一次推理调用前的上下文装配与 provider call 准备。
3. `Hub*` 只负责 provider 协议和 HTTP/wire，不解析业务 control/tool 语义。
4. `FeedbackResp*` 只负责模型输出清洗、用户可见响应、control/tool 解析与 channel render。
5. `ReasonResp*` 只负责把反馈解析结果接回 runtime 决策、tool dispatch、closure。
6. durable truth 写入点只能出现在 `ReasonResp09Closure` / `FeedbackResp06SessionMaterialized`。

---

## 3. 用户输入清洗链 InputIn*

```text
InputIn01ChannelRaw
  -> InputIn02Normalized
  -> InputIn03Operation
  -> InputIn04SessionBound
  -> InputIn05ReasoningSeed
```

| 节点 | 职责 | 输出给 | 禁止 |
| --- | --- | --- | --- |
| `InputIn01ChannelRaw` | 保存 Web / Android / QQBot 原始输入和 channel metadata | `InputIn02Normalized` | 生成 task/topic/control JSON |
| `InputIn02Normalized` | 统一文本、附件摘要、origin、channel source | `InputIn03Operation` | 推断业务路由 |
| `InputIn03Operation` | 构造 operation envelope | `InputIn04SessionBound` | 绕过 operation 直接推理 |
| `InputIn04SessionBound` | 绑定 session/task/topic/agent refs | `InputIn05ReasoningSeed` | 由 channel 维护第二套 session truth |
| `InputIn05ReasoningSeed` | 生成推理入口 seed | `ReasonReq01Seed` | 携带 channel 私有业务语义 |

错误连接：

```text
InputIn01/02 invalid
  -> ErrorErr01Detected
  -> ErrorErr02SourceClassified
  -> ErrorErr05UserVisible
  -> FeedbackResp07ChannelRender
```

---

## 4. 推理请求链 ReasonReq*

```text
ReasonReq01Seed
  -> ReasonReq02ContextPlan
  -> ReasonReq03BudgetedContext
  -> ReasonReq04RenderedInput
  -> ReasonReq05ProviderCall
  -> HubReq01Inbound
```

| 节点 | 职责 | 输出给 | 禁止 |
| --- | --- | --- | --- |
| `ReasonReq01Seed` | 接收推理 seed + refs | `ReasonReq02ContextPlan` | 直接构造 provider 请求 |
| `ReasonReq02ContextPlan` | context section 计划 | `ReasonReq03BudgetedContext` | 直接渲染最终 prompt |
| `ReasonReq03BudgetedContext` | budget / compaction 决策 | `ReasonReq04RenderedInput` | 裁剪真实语义 payload |
| `ReasonReq04RenderedInput` | 唯一模型输入文本 | `ReasonReq05ProviderCall` | 多处 render input |
| `ReasonReq05ProviderCall` | provider 子链入口请求 | `HubReq01Inbound` | 构造 provider wire body |

错误连接：

```text
ReasonReq02/03/04 failure
  -> ErrorErr01Detected
  -> ErrorErr03RuntimeClassified
  -> ErrorErr04SessionRecorded
  -> ReasonResp09Closure(failed)
  -> FeedbackResp06SessionMaterialized
```

---

## 5. Provider 请求/响应链 Hub*

```text
HubReq01Inbound
  -> HubReq02Process
  -> HubReq03Outbound
  -> provider wire
  -> HubResp04Inbound
  -> HubResp05Process
  -> HubResp06Outbound
```

| 节点 | 职责 | 输出给 | 禁止 |
| --- | --- | --- | --- |
| `HubReq01Inbound` | provider 子链标准入口 | `HubReq02Process` | 接收 channel raw input |
| `HubReq02Process` | provider/model/endpoint/header 解析 | `HubReq03Outbound` | 生成 control/tool 语义 |
| `HubReq03Outbound` | 唯一 wire body + HTTP headers | provider wire | 多处直接拼 `serde_json::Value` |
| `HubResp04Inbound` | raw status/body/header fact | `HubResp05Process` | 修改业务语义 |
| `HubResp05Process` | provider 协议响应标准化 | `HubResp06Outbound` | 写 session artifacts |
| `HubResp06Outbound` | runtime 可消费标准输出 | `ReasonResp06ModelOutput` | 暴露 provider 私有 wire shape |

错误连接：

```text
HubReq/HubResp/provider wire failure
  -> ErrorErr01Detected
  -> ErrorErr02SourceClassified
  -> ErrorErr03RuntimeClassified
  -> ErrorErr04SessionRecorded
  -> ReasonResp08RuntimeDecision(retry/fail)
```

provider 限流/配额错误必须进入自动指数回退链；重试耗尽后才进入 final failed truth。

---

## 6. 反馈响应清洗链 FeedbackResp*

```text
ReasonResp06ModelOutput
  -> FeedbackResp01ModelRaw
  -> FeedbackResp02TaggedBlocks
  -> FeedbackResp03UserVisible
  -> FeedbackResp04ControlFeedback
  -> FeedbackResp05ToolIntent
  -> ReasonResp07ParsedContract
  -> ReasonResp08RuntimeDecision
  -> ReasonResp09Closure
  -> FeedbackResp06SessionMaterialized
  -> FeedbackResp07ChannelRender
```

| 节点 | 职责 | 输出给 | 禁止 |
| --- | --- | --- | --- |
| `FeedbackResp01ModelRaw` | 保存模型原文 | `FeedbackResp02TaggedBlocks` | 裁剪/改写模型语义 |
| `FeedbackResp02TaggedBlocks` | tag 检测、确定性 shape repair | `FeedbackResp03/04/05` | 语义推断修复 |
| `FeedbackResp03UserVisible` | 用户可见回复 | `FeedbackResp06SessionMaterialized` | channel 自己剥 tag |
| `FeedbackResp04ControlFeedback` | control feedback 真相 | `ReasonResp07ParsedContract` | fallback 成成功 truth |
| `FeedbackResp05ToolIntent` | tool intent exact/repaired/masked/invalid | `ReasonResp07ParsedContract` | masked/invalid 执行 |
| `FeedbackResp06SessionMaterialized` | session artifacts / ledger records | `FeedbackResp07ChannelRender` | UI 直接写业务 truth |
| `FeedbackResp07ChannelRender` | channel 渲染 DTO | User / Channel | 二次解析业务 JSON |

错误连接：

```text
FeedbackResp02/04/05 invalid
  -> ErrorErr01Detected
  -> ErrorErr03RuntimeClassified
  -> ReasonResp08RuntimeDecision(retry/fail)
  -> ErrorErr04SessionRecorded
```

边界：

1. `fin_control_feedback` missing/invalid 不得生成成功 control truth。
2. `fin_tool_calls` 的 `masked_partial` / `invalid` 只能进入 debug truth，不得执行。
3. 确定性 repair 只能补结构，不能补业务语义。

---

## 7. 错误处理链 ErrorErr*

错误不是旁路；错误必须接回主链的 session truth 和 channel render。

```text
ErrorErr01Detected
  -> ErrorErr02SourceClassified
  -> ErrorErr03RuntimeClassified
  -> ErrorErr04SessionRecorded
  -> ErrorErr05UserVisible
  -> FeedbackResp07ChannelRender
```

| 节点 | 职责 | 输入来源 | 输出给 |
| --- | --- | --- | --- |
| `ErrorErr01Detected` | 捕获原始错误 fact | 任意链路节点 | `ErrorErr02SourceClassified` |
| `ErrorErr02SourceClassified` | 按 input/provider/model/tool/runtime/channel 分类 | `ErrorErr01Detected` | `ErrorErr03RuntimeClassified` |
| `ErrorErr03RuntimeClassified` | 转换为 runtime failure / retry / blocked 决策 | `ErrorErr02SourceClassified` | `ErrorErr04SessionRecorded` 或 `ReasonResp08RuntimeDecision` |
| `ErrorErr04SessionRecorded` | 写入 event / ledger / session artifacts | `ErrorErr03RuntimeClassified` | `ErrorErr05UserVisible` |
| `ErrorErr05UserVisible` | 生成安全、可解释的用户可见错误 | `ErrorErr04SessionRecorded` | `FeedbackResp07ChannelRender` |

错误硬规则：

1. 禁止吞异常。
2. 禁止 fallback / 降级 / 双路径补偿。
3. retry 只允许作为显式 runtime decision，并且每次 attempt 必须有 durable provider/request/response record。
4. 用户可见错误必须来自 session truth，不得由 channel adapter 临时拼。
5. provider quota/rate limit 必须按全局重试规则重试，耗尽后记录 final failure。

---

## 8. Builder / Parser 归属

| 链路 | Owning layer | Builder / Parser 归属 |
| --- | --- | --- |
| `InputIn*` | runtime + channel boundary | runtime input normalizer / operation builder |
| `ReasonReq*` | runtime | context assembler / budget manager / rendered input builder |
| `HubReq*` | provider | provider request processor / wire builder |
| `HubResp*` | provider | raw response parser / response normalizer |
| `FeedbackResp*` | runtime | model output parser / session materializer / channel renderer |
| `ErrorErr*` | runtime + owning source layer | source classifier / runtime error classifier / session recorder |

任何非 owning layer 需要数据，只能消费：

1. 相邻 stage output。
2. durable records。
3. projection / render DTO。

---

## 9. 红测要求

必须有测试或静态门禁证明以下行为失败：

1. `InputIn01ChannelRaw` 直接进入 `ReasonReq01Seed`。
2. `ReasonReq01Seed` 直接构造 provider wire。
3. `HubResp04Inbound` 直接解析 `fin_control_feedback`。
4. `FeedbackResp05ToolIntent(masked_partial|invalid)` 被执行。
5. channel adapter 自己解析 control JSON 或 tool JSON。
6. missing/invalid control feedback 被 fallback 成成功 control truth。
7. provider quota/rate-limit 首次失败即 final fail。
8. critical record 缺少来源 stage 仍能构造成功。

---

## 10. 与现有架构文档关系

本文档补齐命名与链路连接关系，不替代以下文档：

1. `docs/architecture/24-session-render-truth-and-reasoning-input-assembly.md`：session render 与 reasoning input truth。
2. `docs/architecture/29-multi-turn-history-model.md`：turn / step / closure / context rebuild。
3. `docs/architecture/18-provider-gateway-with-litellm.md`：provider gateway 架构。
4. `docs/contracts/model-output-repair-contract.md`：模型输出 repair 边界。
5. `docs/contracts/provider-operation-contract.md` / `docs/contracts/provider-event-contract.md`：provider operation/event contract。

实现时如本文档与旧实现冲突，以本文档定义的链路和命名关系为迁移目标。
