# 23 M1 First Implementation Order and Crate/File Landing

本文档定义 `fin` 进入真实实现时，M1 第一批工作应该按什么顺序做，以及每一步主要落在哪些 crate / 文件。

目标：

1. 先做最小真实切片，不并行发散
2. 让每一步都能形成证据闭环
3. 限制改动面，避免把未来模块提前搅进来

---

## 1. 核心判断

M1 第一批实现顺序必须围绕一条最小真实主线展开：

```text
role/runtime policy
  -> inference operation builder
  -> provider operation -> provider event
  -> recording
  -> event/projection/debug
```

冻结判断：

1. 先把“请求如何进入、如何执行、如何记录、如何观察”这条主线做真
2. 不先做 scheduler / mailbox / RPC / cluster
3. 每一步都要有明确 crate owning layer
4. 每一步都要有最小验证入口

---

## 2. 第一批实现固定顺序

当前建议固定为 4 步。

### Step 1. Role / Runtime Policy 最小类型与配置映射

先回答：

- 一个 runtime 是什么
- 一个 role profile 最小包含什么
- provider path / strategy policy 如何从 config 落到 runtime 默认值

### Step 2. Inference Operation Builder 独立成真边界

先回答：

- 什么输入进入 operation builder
- builder 产出的 operation 最小长什么样
- builder 与 provider execution 的边界是什么

### Step 3. Provider Operation -> Provider Event 最小真实切片

先回答：

- builder 生成的 operation 如何进入 provider slice
- provider 执行后如何变成结构化 provider event
- success / failure / timeout 的事件族如何落盘

### Step 4. Recording + Projection + Debug 闭环

最后回答：

- provider event 如何驱动 progress / note / digest
- projection 如何看到当前状态
- CLI / Web 如何观察这条真实链路

---

## 3. 为什么必须按这个顺序

### 3.1 先 policy，再 builder

如果先写 provider 执行，不先定义 role/runtime policy，后面一定返工：

- provider path 归属不清
- runtime 默认行为不清
- config 映射会反复改

### 3.2 先 builder，再 provider execution

如果不先把 operation builder 单独立起来，provider 很容易反向吞掉请求语义。

这会违反前面已经冻结的规则：

- 请求是 operation
- 响应是 event
- 二者必须分阶段隔离

### 3.3 provider 真切片在 recording 之前

recording 要记录真实执行，不应围绕 mock 结构无限生长。

### 3.4 最后再闭环 debug

debug 不应建立在虚假的内部变量上，而应消费：

- operation
- event
- projection

形成证据闭环。

---

## 4. 每一步的 crate / file 落点

下面是当前建议的最小写入面。

---

## Step 1：Role / Runtime Policy

### Owning crates

- `fin-config`
- `fin-runtime`
- `fin-contracts`

### 目标文件

#### `rust/crates/config/src/lib.rs`

新增/补强：

- `RoleProfileConfig`
- `RuntimePolicyConfig`
- `ProviderPathPolicy`
- user/system config -> runtime policy mapping

#### `rust/crates/contracts/src/lib.rs`

新增最小类型：

- `AgentId`
- `RoleId` / `RoleProfileRef`
- `ProviderPath`
- `ProviderStrategy`

#### `rust/crates/runtime/src/lib.rs`

新增最小 runtime policy 类型：

- `WorkerRuntime`
- `RuntimePolicySnapshot`

### Step 1 完成标准

- config 能解析最小 role/provider policy
- runtime 能拿到稳定默认 policy
- 不引入真实 provider 执行

### Step 1 最小验证

- config unit tests
- policy mapping unit tests
- runtime snapshot serialization test

---

## Step 2：Inference Operation Builder

### Owning crates

- `fin-runtime`
- `fin-contracts`

### 目标文件

#### `rust/crates/contracts/src/lib.rs`

补强：

- inference operation payload type
- provider selection fields
- `provider_path`
- `provider_strategy`

#### `rust/crates/runtime/src/lib.rs`

新增 builder 相关边界：

- `InferenceOperationBuilder`
- `build_inference_operation(...)`

builder 输入只接受：

- runtime policy
- current input
- minimal context view

builder 输出只产生：

- `OperationEnvelope<...>`

### Step 2 完成标准

- runtime 内 operation builder 独立存在
- provider slice 只接受 operation，不直接接用户输入
- operation 能稳定带上 trace / sender / protocol version / provider policy

### Step 2 最小验证

- builder unit tests
- operation serialization/contract tests
- CLI smoke：可打印或落盘 built operation sample

---

## Step 3：Provider Operation -> Provider Event Slice

### Owning crates

- `fin-provider`
- `fin-runtime`
- `fin-contracts`

### 目标文件

#### `rust/crates/provider/src/lib.rs`

从当前薄 descriptor 升级为最小真实 slice：

- `ProviderFacade`
- `execute_operation(...)`
- provider event normalization boundary

当前先不追求完整 LiteLLM 实现细节，但接口必须对：

- 输入：operation
- 输出：provider events / normalized result

#### `rust/crates/contracts/src/lib.rs`

补 provider 事件 payload 最小类型：

- request sent
- response received
- normalized
- completed
- failed

#### `rust/crates/runtime/src/lib.rs`

runtime 不再伪造 provider 成功，而是消费 provider slice 返回结果。

### Step 3 完成标准

- operation -> provider event 链成立
- success / failure 都能结构化记录
- provider slice 不直接推进 task 业务语义

### Step 3 最小验证

- provider unit tests
- runtime + provider integration tests
- CLI smoke：能看到 provider 事件链

---

## Step 4：Recording + Projection + Debug

### Owning crates

- `fin-runtime`
- `fin-debug-server`
- `fin-cli`

### 目标文件

#### `rust/crates/runtime/src/lib.rs`

补强：

- provider event -> progress
- provider event -> execution note
- closure -> digest

#### `rust/crates/debug-server/src/lib.rs`

补强：

- 更清楚的 provider activity projection
- latest error / failure projection
- current runtime snapshot

#### `rust/crates/cli/src/main.rs`

补强：

- 更稳定的 demo/debug entry
- provider event / projection smoke

### Step 4 完成标准

- 真实 provider 事件能驱动 recording
- projection 能看见 provider 当前状态
- Web/CLI 能看到完整最小链路

### Step 4 最小验证

- runtime tests
- debug-server tests
- `cargo test`
- CLI hand smoke
- Web debug smoke

---

## 5. 当前明确不该顺手做的事

在这 4 步里，禁止顺手扩展到：

- mailbox runtime
- eventbus runtime
- RPC transport
- multi-worker dispatch
- cluster registry
- full replay harness
- policy plugin system

原因：

这些都会打断当前最小真实切片闭环。

---

## 6. 执行时的最小验证矩阵

每步都按下面矩阵收口：

### L1 Unit

- 类型
- mapping
- builder
- normalizer

### L2 Contract

- operation/event serialization
- projection schema

### L3 Integration

- runtime + provider
- runtime + debug-server

### L4 CLI Smoke

- `home-init`
- `runtime-demo`
- `debug-projection`
- `web-debug`

### L5 Real Runtime Home Evidence

- `~/.fin/runtime/*`
- `~/.fin/sessions/*`
- `~/.fin/logs/*`

---

## 7. 建议的实际推进方式

如果开始进入真实实现，建议严格按下面节奏：

### Round 1

- Step 1 全做完并验证

### Round 2

- Step 2 全做完并验证

### Round 3

- Step 3 全做完并验证

### Round 4

- Step 4 全做完并验证

也就是：

```text
不要四步并发做
不要 provider/debug/runtime 一起乱改
```

先让边界稳定，再往后推。

---

## 8. 当前冻结的硬规则

1. M1 第一批真实实现固定按 4 步推进
2. 每一步都有明确 owning crate
3. operation builder 必须先于 provider execution 稳定
4. provider slice 只接受 operation、只产出 provider 事件
5. recording/debug 必须消费真实 provider 事件，不围绕临时变量长逻辑
6. 不得在这 4 步里顺手引入 mailbox/eventbus/RPC/cluster