# Pipeline Unique Type Refactor Plan

## 1. 目标与验收标准

目标：把 fin 内部所有关键数据流改造成“每个流水线节点一个唯一类型、一个唯一 builder/parser、只允许相邻转换”的结构，避免相同数据结构在多条链路中复用导致乱改、双真源和阶段边界失守。

架构真源：`docs/architecture/44-pipeline-unique-type-and-error-chain.md`。

验收标准：

1. 全局 `~/.codex/AGENTS.md` 存在通用 Pipeline Unique Type Rule，不只限定 provider Hub pipeline。
2. fin 至少锁定四条主链：用户输入链、推理逻辑链、provider 调用链、反馈响应链。
3. 每条链都有固定命名模板、节点序号、节点职责、唯一转换入口和禁止行为。
4. 泛化命名类型（例如不能表达链路位置的 `ProviderRequest` / `PreparedRequest` / `ProviderResponse` / `RoundExecution`）被迁移、替换或删除，不保留死别名。
5. 红测能证明跨节点构造、绕过 builder、UI/adapter 拼业务 JSON、masked/invalid tool 执行等行为会失败。
6. 节点编号稳定性被测试和 review 规则锁定：禁止中间插节点、禁止重编号、禁止小数/后缀编号。

## 2. 范围与边界

In scope：

1. `~/.codex/AGENTS.md` 的跨项目规则更新。
2. `rust/crates/contracts` 的公共 record / contract 类型边界。
3. `rust/crates/runtime` 的输入装配、推理 round、模型输出解析、closure materialization。
4. `rust/crates/provider` 的 provider request/response wire pipeline。
5. `rust/crates/debug-server`、`rust/crates/cli` 中 Web/Android/QQBot channel adapter 的输入/渲染边界。
6. 相关 docs / local skills 的经验沉淀。

Out of scope：

1. 不改变真实 payload 语义，不通过裁剪请求/响应换性能。
2. 不引入 fallback / 双路径兼容；迁移期也必须有唯一主路径。
3. 不把 UI、Android、QQBot 变成业务语义解析真源。
4. 不重做 provider strategy、模型选择、session ledger 总架构。

## 3. 设计原则

### 3.1 通用命名模板

所有关键链路类型使用：

```text
<Domain><Direction><NN><Node>
```

规则：

1. `Domain` 表示链路域：`Input` / `Reason` / `Hub` / `Feedback`。
2. `Direction` 表示方向：`In` / `Req` / `Resp`。
3. `NN` 是两位节点序号，按数据流方向递增。
4. `Node` 是节点语义名，不允许泛化为 `Request` / `Response` / `Data` / `Payload`。

### 3.2 转换边界

1. 只允许相邻节点转换。
2. 每个节点只有一个 owning builder/parser。
3. 节点内部字段不跨模块公开；下游只能消费相邻 output。
4. 运行时必须校验 stage id / contract version / required refs。
5. 任何跨节点 shortcut 必须由红测锁死。
6. 节点编号是 contract；新增能力优先进入既有节点内部 block / validator / parser，默认禁止中间插节点。

### 3.3 真源边界

1. Channel adapter 只拥有 channel 原始输入与最终渲染，不拥有业务语义。
2. Runtime 拥有用户输入标准化、推理装配、模型输出解析、closure materialization。
3. Provider 只拥有 wire 构建与 provider 响应标准化，不拥有推理语义。
4. Session artifacts / ledger records 是用户可见响应与调试回放真源。

## 4. 总体技术方案

### 4.1 用户输入链：InputIn*

目标：把 Web / Android / QQBot 的输入统一收敛到 runtime 唯一 operation 入口，禁止 channel 自己拼业务 JSON。

| 节点 | 类型 | 职责 | 唯一入口 | 禁止行为 |
| --- | --- | --- | --- | --- |
| 01 | `InputIn01ChannelRaw` | channel 原始输入，保留 channel id、message id、附件原始摘要 | channel adapter capture | 写入 runtime 业务字段 |
| 02 | `InputIn02Normalized` | 标准化自然语言、附件摘要、origin metadata | input normalizer | 生成 task/topic/control JSON |
| 03 | `InputIn03Operation` | 构造 `OperationEnvelope<InferenceOperationPayload>` | operation builder | 绕过 operation 直接调用 reasoning |
| 04 | `InputIn04SessionBound` | 绑定 session/task/topic/agent refs | session binding builder | channel adapter 私自绑定第二真相 |
| 05 | `InputIn05ReasoningSeed` | 推理链 current input seed | reasoning seed builder | 携带 channel 私有业务语义 |

主要落点：

1. `rust/crates/debug-server/src/mobile_ws.rs`
2. `rust/crates/cli/src/channel_peer_qqbot_bridge*.rs`
3. `rust/crates/runtime/src/closure_runtime.rs`
4. `rust/crates/contracts/src/lib.rs` / `records.rs`

### 4.2 推理逻辑链：ReasonReq* / ReasonResp*

目标：把 `execute_round` 中混在一个函数里的 context assembly、budget、provider call、parse、tool dispatch、control decision 拆成显式节点。

| 节点 | 类型 | 职责 | 唯一入口 | 禁止行为 |
| --- | --- | --- | --- | --- |
| 01 | `ReasonReq01Seed` | 当前输入 + refs + operation baseline | from `InputIn05ReasoningSeed` | 直接拼 provider request |
| 02 | `ReasonReq02ContextPlan` | `ModelInputAssembler` 装配计划 | context plan builder | 跳过 context plan |
| 03 | `ReasonReq03BudgetedContext` | context budget / compaction 决策 | budget manager | 裁剪真实 payload 语义 |
| 04 | `ReasonReq04RenderedInput` | 唯一模型输入文本 | rendered input builder | 多处 render input |
| 05 | `ReasonReq05ProviderCall` | 进入 provider 子链的请求 | provider call builder | runtime 构造 provider wire |
| 06 | `ReasonResp06ModelOutput` | provider 返回后的模型原文 | provider response adapter | 在 provider 层解析业务语义 |
| 07 | `ReasonResp07ParsedContract` | 解析三类结构块 | model output parser | prose 推断工具/控制语义 |
| 08 | `ReasonResp08RuntimeDecision` | retry/tool/closure/control 决策 | runtime decision builder | fallback 成成功 control truth |
| 09 | `ReasonResp09Closure` | durable records materialization 输入 | closure builder | 绕过 closure 写 session artifacts |

主要落点：

1. `rust/crates/runtime/src/closure_runtime_rounds.rs`
2. `rust/crates/runtime/src/model_input_assembler.rs`
3. `rust/crates/runtime/src/context_budget.rs`
4. `rust/crates/runtime/src/model_output.rs`
5. `rust/crates/runtime/src/model_output_tool_calls.rs`
6. `rust/crates/runtime/src/model_output_feedback.rs`
7. `rust/crates/runtime/src/closure_runtime_finalize.rs`
8. `rust/crates/runtime/src/turn_records.rs`

### 4.3 Provider 调用链：HubReq* / HubResp*

目标：把 provider wire pipeline 从推理链中独立出来，只承担 provider 协议转换和 provider 响应标准化。

| 节点 | 类型 | 职责 | 唯一入口 | 禁止行为 |
| --- | --- | --- | --- | --- |
| 01 | `HubReq01Inbound` | provider 子链入口请求 | from `ReasonReq05ProviderCall` | 接收 channel/raw input |
| 02 | `HubReq02Process` | provider/model/endpoint/header 解析 | provider process builder | 协议 wire build |
| 03 | `HubReq03Outbound` | OpenAI/Anthropic/LiteLLM wire body + headers | wire builder | 多处直接 `serde_json::Value` wire |
| 04 | `HubResp04Inbound` | raw HTTP status/body/headers | HTTP client adapter | 解析 control/tool 语义 |
| 05 | `HubResp05Process` | provider response 标准化 | protocol parser | 写 session truth |
| 06 | `HubResp06Outbound` | 返回 runtime 的标准输出 | outbound builder | 携带 provider 私有 wire shape |

主要落点：

1. `rust/crates/provider/src/blocks/request.rs`
2. `rust/crates/provider/src/blocks/response.rs`
3. `rust/crates/provider/src/wire/openai_wire.rs`
4. `rust/crates/provider/src/wire/anthropic_wire.rs`
5. `rust/crates/provider/src/lib.rs`

清理候选：

1. `rust/crates/provider/src/provider_static.rs` 当前疑似未接入旧实现；实现阶段必须先证明无引用，再物理删除或恢复为唯一实现。

### 4.4 反馈响应链：FeedbackResp*

目标：把模型输出到用户可见回复、control feedback、tool intent、session artifacts、channel render 的链路显式化。

| 节点 | 类型 | 职责 | 唯一入口 | 禁止行为 |
| --- | --- | --- | --- | --- |
| 01 | `FeedbackResp01ModelRaw` | 模型原文 | from `ReasonResp06ModelOutput` | 裁剪/改写真实输出 |
| 02 | `FeedbackResp02TaggedBlocks` | tag 检测与确定性 shape repair | tagged block parser | 语义推断修复 |
| 03 | `FeedbackResp03UserVisible` | 用户可见回复 | user response extractor | channel 自己剥 tag |
| 04 | `FeedbackResp04ControlFeedback` | control feedback 真相 | control feedback parser | fallback 成成功 control truth |
| 05 | `FeedbackResp05ToolIntent` | 可执行工具意图 | tool intent parser | masked/invalid tool 执行 |
| 06 | `FeedbackResp06SessionMaterialized` | session/ledger/artifact 写入材料 | materializer | UI 直接写业务真相 |
| 07 | `FeedbackResp07ChannelRender` | channel 渲染 DTO | channel renderer | 二次解析 JSON 语义 |

主要落点：

1. `rust/crates/runtime/src/model_output.rs`
2. `rust/crates/runtime/src/model_output_shapes.rs`
3. `rust/crates/runtime/src/model_output_tool_calls.rs`
4. `rust/crates/runtime/src/model_output_feedback.rs`
5. `rust/crates/runtime/src/closure_runtime_records.rs`
6. `rust/crates/cli/src/channel_peer_qqbot_bridge_support.rs`
7. `rust/crates/cli/src/channel_peer_activity_delivery_render*.rs`
8. Web debug session render readers

## 5. 子任务实现计划

### Subtask 1：全局规则与 fin 本地规则沉淀

目标：先冻结通用规则，避免后续实现继续按旧命名扩散。

实现步骤：

1. 更新 `~/.codex/AGENTS.md`，把现有 Hub Pipeline 规则升级为通用 Pipeline Unique Type Rule。
2. 更新 `AGENTS.md` 或 `skills/fin-general-dev/SKILL.md`，加入 fin 本地执行适配：新增 pipeline 类型必须先登记链路节点。
3. 在本计划中维护四条主链表，作为实施阶段索引。

验收：

1. 全局规则能覆盖任何项目的数据流水线，不只 provider。
2. fin 本地 skill 能把任务路由到本计划与对应 docs/contracts。

### Subtask 2：Contracts 层建立 pipeline stage 基础设施

目标：为所有链路提供统一 stage id、相邻转换、record 来源证明。

实现步骤：

1. 在 `rust/crates/contracts` 增加 pipeline stage 基础类型，例如 stage id、chain id、stage refs、conversion metadata。
2. 给 durable records 增加来源 stage 或通过 builder 强制记录来源。
3. 增加 contracts tests：非法 stage id、缺 required refs、跨链 refs 失败。

验收：

1. record builder 能证明某条 durable record 来自哪个 chain/stage。
2. 下游不能自由 new 出缺 stage 来源的 critical record。
3. stage id 校验拒绝小数编号、后缀编号、重排编号和 V1/V2 混接。

### Subtask 2.1：节点编号稳定性门禁

目标：避免后续在中间插节点导致全局命名再次混乱。

实现步骤：

1. 在 contracts 或 scripts 中登记每条链的 stage registry。
2. 增加检查：已登记节点编号不可改语义、不可复用、不可重排。
3. 增加检查：禁止 `03a`、`03_1`、`03.5` 等非两位整数正式编号。
4. 新能力 review 时必须先判断能否进入既有节点内部 block；只有 ownership/truth/error 边界变化才允许链尾追加或新 chain version。
5. 新 chain version 必须带迁移计划和旧链物理删除条件。

验收：

1. 中间插入新节点的 fixture 必红。
2. 链尾追加新节点的 fixture 可绿。
3. V1/V2 节点混接的 fixture 必红。

### Subtask 3：用户输入链 InputIn* 改造

目标：统一 Web/Android/QQBot 输入入口，禁止 adapter 拥有业务语义。

实现步骤：

1. 增加 `InputIn01ChannelRaw` 与 `InputIn02Normalized` 类型。
2. 将 `mobile_ws.rs`、QQBot bridge、Web debug 输入入口改为先产出 `InputIn01ChannelRaw`。
3. 增加唯一 normalizer，输出 `InputIn02Normalized`。
4. 增加 operation builder，输出 `InputIn03Operation`。
5. 增加 session binding builder，输出 `InputIn04SessionBound`。
6. 增加 reasoning seed builder，输出 `InputIn05ReasoningSeed`。

验收：

1. `rg` 证明 channel adapter 不再构造 task/topic/control JSON。
2. Web/Android/QQBot 三入口都经同一 normalizer 与 operation builder。

### Subtask 4：推理逻辑链 Reason* 改造

目标：拆开 `execute_round` 的隐式流水线，让每个推理阶段都有唯一类型。

实现步骤：

1. 用 `ReasonReq01Seed` 替代 round 函数中的裸 `input + refs + operation` 参数组合。
2. 用 `ReasonReq02ContextPlan` 包装 `ModelInputAssembler` 输出。
3. 用 `ReasonReq03BudgetedContext` 包装 budget / compaction 决策。
4. 用 `ReasonReq04RenderedInput` 包装唯一 rendered input。
5. 用 `ReasonReq05ProviderCall` 作为 provider 子链入口。
6. 用 `ReasonResp06ModelOutput` 接 provider 输出。
7. 用 `ReasonResp07ParsedContract` / `ReasonResp08RuntimeDecision` 替代 `RoundExecution` 内部散字段。
8. 用 `ReasonResp09Closure` 统一进入 records/materializer。

验收：

1. `closure_runtime_rounds.rs` 不再出现混合阶段的大型 `RoundExecution` 真源。
2. context render、provider call、model parse、tool dispatch、control decision 均只有一个转换入口。

### Subtask 5：Provider 链 Hub* 改造

目标：把 provider wire 与推理语义彻底分离。

实现步骤：

1. 将 `ProviderRequest` 迁移为 `HubReq01Inbound`。
2. 将 `PreparedRequest` 迁移为 `HubReq02Process`。
3. 将 wire payload/header 输出包成 `HubReq03Outbound`。
4. 增加 `HubResp04Inbound` 保存 raw HTTP fact。
5. 将 parser 输出改为 `HubResp05Process`。
6. 将 runtime 可消费响应改为 `HubResp06Outbound`。
7. 清理或唯一化 `provider_static.rs`。

验收：

1. provider crate 中 OpenAI/Anthropic wire build 只出现在 wire owning modules。
2. runtime 不直接接触 provider wire shape。

### Subtask 6：反馈响应链 FeedbackResp* 改造

目标：把模型输出解析、用户回复、control feedback、tool intent、session materialization、channel render 分开。

实现步骤：

1. 用 `FeedbackResp01ModelRaw` 接住原始模型输出。
2. 用 `FeedbackResp02TaggedBlocks` 表达 tag 检测和 deterministic repair 结果。
3. 用 `FeedbackResp03UserVisible` 表达可见回复。
4. 用 `FeedbackResp04ControlFeedback` 表达唯一 control truth；missing/invalid 进入 retry/failure，不做成功 fallback。
5. 用 `FeedbackResp05ToolIntent` 表达工具意图 exact/repaired/masked/invalid 状态。
6. 用 `FeedbackResp06SessionMaterialized` 统一写 session artifacts / ledger records。
7. 用 `FeedbackResp07ChannelRender` 统一给 Web/Android/QQBot 渲染。

验收：

1. `masked_partial` / `invalid` tool 永不执行，但进入 debug truth。
2. channel render 不再自己剥业务 tag 或解析 control JSON。

### Subtask 7：红测与门禁

目标：让违规路径必红，避免靠约定维护边界。

实现步骤：

1. 增加 compile-fail 或等价 contract tests：跨节点直接构造失败。
2. 增加 runtime tests：跳过 context plan、missing control feedback 成功 fallback、masked tool 执行均失败。
3. 增加 adapter tests：channel 拼业务 JSON、直接读取 provider/debug truth 作为用户渲染均失败。
4. 增加静态门禁脚本或测试：禁止关键旧泛名在主路径残留。

验收：

1. 所有红测先红后绿。
2. `rg "ProviderRequest|PreparedRequest|ProviderResponse|RoundExecution|fallback_feedback"` 在主路径无旧实现残留。

### Subtask 8：迁移验证与证据落盘

目标：用测试和真实 E2E 证明四条链闭环。

实现步骤：

1. 运行 contracts/provider/runtime/debug-server/cli 定向测试。
2. 运行真实 provider 多轮 smoke，证明输入、推理、provider、反馈、session/channel artifacts 全链可追踪。
3. 将测试输出、artifact 路径、rg 结果写入 receipt。
4. 将已验证规律提炼到 `MEMORY.md` 或 local skill。

验收：

1. 无真实 artifacts 不宣称闭环完成。
2. receipt 能定位每条链每个节点的输入、输出、record refs。

## 6. 风险与规避

1. 风险：一次性重命名过大导致全仓编译破碎。规避：按 Contracts → Input → Reason → Provider → Feedback → Tests 顺序推进，每步保持唯一主路径。
2. 风险：短期兼容别名变成永久双真源。规避：只允许同一 PR 内临时别名，最终 `rg` 清零。
3. 风险：control feedback fallback 掩盖模型输出错误。规避：missing/invalid control feedback 进入 retry/failure truth，不生成成功 control truth。
4. 风险：channel adapter 为了展示方便继续解析业务 JSON。规避：adapter 只消费 `FeedbackResp07ChannelRender`。
5. 风险：provider wire 改造影响真实 payload。规避：wire snapshot + live provider smoke 验证语义等价。

## 7. 验证矩阵

| 层级 | 命令/方式 | 验证点 |
| --- | --- | --- |
| Contracts | `cargo test -p fin-contracts` | stage/ref/record contract |
| Provider | `cargo test -p fin-provider` | HubReq/HubResp wire 与 parser |
| Runtime | `cargo test -p fin-runtime` | Input/Reason/Feedback 链路 |
| Debug Server | `cargo test -p fin-debug-server` | Android/Web input 与 provider smoke 入口 |
| CLI/Channel | `cargo test -p fin-cli` 定向 channel tests | QQBot/Web render 不解析业务真相 |
| Static gate | `rg` 旧泛名和违规 builder | 无旧主路径残留 |
| Live E2E | real provider smoke | 四链真实 artifacts 可追踪 |

## 8. 实施顺序

1. 更新全局规则与本地计划文档。
2. 建 contracts stage 基础设施。
3. 建节点编号稳定性门禁。
4. 改用户输入链。
5. 改推理逻辑链。
6. 改 provider 链。
7. 改反馈响应链。
8. 加红测和静态门禁。
9. 跑定向测试与真实 E2E，产出 receipt。
10. 更新 `MEMORY.md` / local skill，压缩 `CACHE.md`。

## 9. 完成定义 DoD

1. 四条链类型、builder/parser、tests 全部落地。
2. 泛化旧类型和死代码物理删除。
3. 红测覆盖关键违规路径。
4. 定向测试、静态门禁、真实 provider E2E 均有证据。
5. docs / skills / memory 已同步最终验证结论。
6. 节点新增策略已锁定：默认不插中间节点；必要变化走链尾追加或新 chain version + 旧链删除计划。

## 6. 完成证明（2026-06-02）

5 链全部落地，依次 commit：

| 链 | commit | owning layer | 红测文件 |
| --- | --- | --- | --- |
| Reason | `99d95d4 refactor(runtime): lock reasoning pipeline nodes` | runtime | `reason_pipeline_static_tests.rs` |
| Input  | `5ae5c7a refactor(runtime): lock input chain nodes` | runtime | `input_pipeline_static_tests.rs` |
| Hub    | `c93dd7c refactor(provider): lock hub pipeline nodes` | provider | `hub_pipeline_static_tests.rs` |
| Feedback | `7c6d98f refactor(runtime): lock feedback chain nodes` | runtime | `feedback_pipeline_static_tests.rs` |
| Error  | `f1f353e refactor(runtime): lock error chain nodes` | runtime | `error_pipeline_static_tests.rs` |

证据：

- 静态门禁（命名 + 编号 + 禁止结构）：10 项全过。
- 业务回归：`model_output_parser_*` 8 项、`runtime_closure_uses_structured_user_response_*` 1 项、`control_feedback_builder_uses_runtime_defaults_*` 1 项。
- Provider 全测 36 项（执行后未回退）。
- `cargo build -p fin-cli` 通过；未触碰 `cli` / `debug-server` 公开 API。
- 已物理删除/改名：`RoundExecution` 泛名 → `ReasonRoundExecution`；`merge_with_fallback` → `merge_with_runtime_defaults`；`model_output.rs` 中重复 `parsed_tool_calls` 中间变量删除；feedback 节点 tag 常量从 `model_output.rs` 迁出。
- 错误归一：`map_runtime_error_through_error_pipeline` 把 `RuntimeError` 显式串入 `ErrorErr01..05`，禁止 `try { ... } catch { return Ok }` 类吞异常。
- 真实 provider 多轮 E2E + receipt 落盘尚未做（留为后续目标，避免在受限 worktree 冒充真实验收）。
