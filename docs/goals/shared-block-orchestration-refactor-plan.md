# fin 共享函数库 + Block + 纯编排拆分计划

## 目标

把 `fin` 当前“crate 边界存在但实现仍大量混装”的代码库，收敛成符合项目真源文档的三层结构：

1. **Shared Functions**
   - 纯函数、错误工具、ID/时间/path helper、通用 retry/normalize/summary
   - 不含 IO、不含状态推进、不含 UI 语义
2. **Blocks**
   - 稳定的数据块、记录块、session/provider/agent/context/tool execution 等事实单元
   - block 负责承载稳定输入/输出与记录边界
   - block 不承担跨模块编排决策
3. **Pure Orchestration**
   - 只负责状态推进、生命周期、task routing、dispatch、resume/review/close
   - 不定义协议字段，不复制 block 结构，不直写 UI 语义

## 验收标准

### Shared

- 跨 crate 通用纯逻辑都收敛到 `fin-shared`
- `cli/runtime/provider/debug-server` 不再各自维护第二套同义 helper
- 至少完成首批 shared owner 收敛：
  - retry/backoff
  - error chain summary
  - id/time/path helpers
  - 小型 normalize/sanitize helpers

### Blocks

- `session / context / tool execution / agent control / provider event` 至少形成明确 block owner
- block 模块只暴露稳定结构和纯转换，不承担 run loop / dispatch / CLI command branching
- runtime/debug/web/channel 不再直接拼装“临时业务形状”冒充 block

### Orchestration

- `session command routing / agent dispatch / closure round loop / supervisor/startup flow` 从混装文件中抽出
- orchestration 只消费 shared + blocks + adapters
- `cli` 只做入口适配，`debug-server` 只做观察/transport 适配

### Non-goals / 排除项

- 不在本轮重构中重新设计协议语义
- 不新增第二套 runtime truth
- 不为“先拆分”而先迁移所有 crate；先按 owning layer 收口

## 范围

### In Scope

- `rust/crates/shared`
- `rust/crates/runtime`
- `rust/crates/provider`
- `rust/crates/cli`
- `rust/crates/debug-server`
- 必要的 `contracts` 边界整理
- 对应 docs / tests / regression gates

### Out of Scope

- Android / Web 视觉重做
- provider 新协议扩展
- 新的 cluster / remote execution 设计
- 非 owning layer 的顺手重构

## 真源依据

- `docs/architecture/02-layer-boundaries.md`
- `docs/architecture/09-workspace-and-crate-map.md`
- `docs/architecture/23-m1-first-implementation-order-and-crate-landing.md`

## 审计结论摘要

### 1. `fin-shared` 过薄

当前 `fin-shared` 只承载少量 helper：

- `rust/crates/shared/src/lib.rs`

说明大量本应共享的纯逻辑仍散落在：

- `fin-cli`
- `fin-provider`
- `fin-runtime`
- `fin-debug-server`

### 2. `fin-cli` 过重，不是纯入口

当前 `cli` 同时拥有：

- session command 业务
- channel/peer 业务
- startup/supervisor 业务
- install/build smoke
- local multi-agent harness

关键热点：

- `rust/crates/cli/src/session_commands.rs`
- `rust/crates/cli/src/local_multi_agent_node.rs`
- `rust/crates/cli/src/status_probe.rs`
- `rust/crates/cli/src/startup_topology.rs`

### 3. `fin-runtime` 混装了 block + orchestration

当前 `runtime` 既包含：

- record/block
- prompt/context assembly
- tool dispatch
- closure round loop
- owner loop
- scheduler
- agent control

关键热点：

- `rust/crates/runtime/src/closure_runtime.rs`
- `rust/crates/runtime/src/agent_control.rs`
- `rust/crates/runtime/src/session_materializer.rs`
- `rust/crates/runtime/src/prompt_assembly.rs`

### 4. `fin-provider` 仍偏单体

同一文件同时承担：

- error model
- descriptor
- request/response types
- protocol mapping
- execute loop
- retry path

关键热点：

- `rust/crates/provider/src/lib.rs`

### 5. `debug-server` 混了 transport + read model + business glue

关键热点：

- `rust/crates/debug-server/src/mobile_ws.rs`
- `rust/crates/debug-server/src/agent_rpc.rs`

## 目标结构

## A. Shared Functions 层

### Owning crate

- `rust/crates/shared`

### 应承载内容

1. retry/backoff policy
2. error chain summary
3. id / trace / timestamp helpers
4. path / runtime-home / session-id helper
5. sanitize / normalize 小型纯函数
6. 小型 shared error model

### 禁止内容

- 文件 IO
- HTTP/WS 调用
- session/materializer 状态推进
- CLI branch / command routing

## B. Blocks 层

### 先按模块切，不强行先拆 crate

建议先在 owning crate 内形成稳定 block 目录，而不是第一步就大规模挪 crate：

#### Runtime blocks

- `runtime/blocks/session/*`
- `runtime/blocks/context/*`
- `runtime/blocks/tool_execution/*`
- `runtime/blocks/agent_control/*`
- `runtime/blocks/activity_cards/*`

#### Provider blocks

- `provider/blocks/request/*`
- `provider/blocks/response/*`
- `provider/blocks/errors/*`

#### Debug read blocks

- `debug-server/blocks/session_view/*`
- `debug-server/blocks/mobile_snapshot/*`

### Block 规则

- 一个 block 只负责稳定输入/输出和纯组装
- block 可读写 record/type，但不做跨 block 编排
- block 输出必须可测试、可序列化、可被 orchestrator 消费

## C. Pure Orchestration 层

### 目标 owning layer

- 优先收敛到 `fin-orchestrator`
- 在迁移前允许先在 `runtime/orchestration/*`、`cli/orchestration/*` 临时隔离

### 应承载内容

1. session command -> runtime action mapping
2. closure round loop推进
3. project/system dispatch
4. startup / supervisor / wakeup flow
5. review/submit/claim lifecycle

### 禁止内容

- 定义 block 结构
- 改协议字段
- 直接拼 UI 数据结构
- 重写 shared helper

## 分阶段实施步骤

## Phase 0：冻结边界与红测

### 目标

- 先把 “谁属于 shared / block / orchestration” 冻结
- 对热点文件补红测，防止拆分时丢行为

### 需要补的定向测试

1. `closure_runtime` 行为快照测试
2. `session_commands` 命令路由/绑定行为测试
3. `agent_control` mailbox/run lifecycle 测试
4. `mobile_ws` 握手/订阅/错误路径测试
5. `provider` execute/retry/error 分类测试

## Phase 1：先抽 Shared

### 目标

- 先把跨 crate 重复纯逻辑从 `cli/provider/runtime/debug-server` 下沉到 `fin-shared`

### 第一批 shared 提取清单

1. retry/backoff
2. error chain summary
3. trace/session/path helpers
4. payload sanitize helpers
5. 常见 `read_json_or_empty` / `write_json` 前的纯 path/shape helper

### 文件落点

- `rust/crates/shared/src/lib.rs`
- 必要时拆：
  - `rust/crates/shared/src/retry.rs`
  - `rust/crates/shared/src/error.rs`
  - `rust/crates/shared/src/id.rs`
  - `rust/crates/shared/src/path.rs`

## Phase 2：切 Runtime Blocks

### 目标

- 把 `runtime` 里“稳定数据块”从大编排文件中剥离

### 第一批 block

1. `agent_control` block
2. `session_materializer` block
3. `tool_execution_record` block
4. `context snapshot / digest / turn record` block

### 重点文件

- `rust/crates/runtime/src/agent_control.rs`
- `rust/crates/runtime/src/session_materializer.rs`
- `rust/crates/runtime/src/closure_runtime.rs`
- `rust/crates/runtime/src/activity_cards_render.rs`

## Phase 3：切 Provider Blocks

### 目标

- 把 `provider/src/lib.rs` 切成：
  - errors
  - request/response blocks
  - protocol-specific payload builders
  - execution orchestrator

### 建议文件

- `rust/crates/provider/src/errors.rs`
- `rust/crates/provider/src/request_block.rs`
- `rust/crates/provider/src/response_block.rs`
- `rust/crates/provider/src/openai_wire.rs`
- `rust/crates/provider/src/anthropic_wire.rs`
- `rust/crates/provider/src/execute.rs`

## Phase 4：切 CLI 纯入口 vs 编排

### 目标

- `cli` 只保留入口/参数/命令分发
- session/peer/startup/install 的业务推进抽到纯编排模块

### 第一批热点

1. `session_commands.rs`
2. `startup_topology.rs`
3. `status_probe.rs`
4. `install_smoke.rs`
5. `local_multi_agent_*`

### 建议结构

- `cli/entry/*`
- `cli/orchestration/session/*`
- `cli/orchestration/startup/*`
- `cli/orchestration/install/*`
- `cli/orchestration/multi_agent/*`

## Phase 5：切 Debug Server 纯 adapter

### 目标

- `mobile_ws` / `agent_rpc` 只保留 transport + adapter
- 读模型拼装和业务 glue 抽到独立 block / service

### 重点文件

- `rust/crates/debug-server/src/mobile_ws.rs`
- `rust/crates/debug-server/src/agent_rpc.rs`
- `rust/crates/debug-server/src/session_view.rs`

## 风险与规避

### 风险 1：先拆 crate，后发现行为没冻结

- 规避：先补红测，先做文件内模块拆，再决定是否迁 crate

### 风险 2：把 polling/adapter wait 误当 retry policy

- 规避：只对错误 owner 的 retry 收敛；轮询 wait 单独看

### 风险 3：入口层拆分后编排丢真相

- 规避：orchestration 只消费 block/shared，不重定义 contracts

### 风险 4：UI/debug 读路径被拆坏

- 规避：debug/web regression 必须保持绿色

## 测试计划

### Shared

- `cargo test -p fin-shared --manifest-path rust/Cargo.toml -- --nocapture`

### Provider

- `cargo test -p fin-provider --manifest-path rust/Cargo.toml -- --nocapture`

### Runtime

- `cargo test -p fin-runtime agent_control_tests --manifest-path rust/Cargo.toml -- --nocapture`
- `cargo test -p fin-runtime prompt_tests --manifest-path rust/Cargo.toml -- --nocapture`
- `cargo test -p fin-runtime tool_dispatch_tests --manifest-path rust/Cargo.toml -- --nocapture`

### CLI

- `cargo test -p fin-cli --manifest-path rust/Cargo.toml -- --nocapture`

### Debug

- `cargo test -p fin-debug-server --manifest-path rust/Cargo.toml -- --nocapture`

### Build / regression

- `scripts/regression/run_local_regression.sh`
- 必要时：
  - `scripts/run-local-multi-agent-e2e.sh static <run-id>`
  - `scripts/run-local-multi-agent-e2e.sh live <run-id>`

## 完成定义（DoD）

满足以下条件才算这一轮拆分完成：

1. shared helper 不再在 `cli/provider/runtime/debug-server` 四处重复实现
2. 至少一条 runtime 主链完成 block 化与 orchestration 分离
3. `cli` 至少一个大热点文件完成“入口/编排/共享逻辑”分离
4. `debug-server` 至少一个大热点文件完成“adapter/read-block/business glue”分离
5. 定向测试 + 回归门禁通过

## 第一阶段建议执行顺序（唯一主路径）

1. `fin-shared` 继续扩成真正 shared owner
2. 拆 `runtime::agent_control`
3. 拆 `runtime::closure_runtime`
4. 拆 `provider::lib`
5. 拆 `cli::session_commands`
6. 拆 `debug-server::mobile_ws`

这是当前唯一合理主路径，因为：

- 先抽 shared 才能防止后续越拆越重复
- `agent_control + closure_runtime` 是 runtime 最核心的 block/orchestration 混装点
- `provider` 和 `cli/debug` 都依赖前面 shared/runtime 边界稳定后再拆才不会返工

## 当前进度追踪（2026-05-24）

| Phase | 目标 | 状态 | 备注 |
|-------|------|------|------|
| Phase 1 | fin-shared 扩成真正 shared owner | ✅ 完成 | retry/error/trace/path/id helpers |
| Phase 2a | runtime::agent_control block/IO 分离 | ✅ 完成 | agents.rs(shared) + agent_control_io.rs |
| Phase 2b | runtime::closure_runtime 拆分 | 🔄 进行中 | 已拆分 10 个子模块，round_loop 测试全绿 |
| Phase 3 | provider::lib 拆解 (766行→blocks+orchestration) | ⏳ 待开始 | |
| Phase 4 | cli::session_commands 拆解 (879行) | ⏳ 待开始 | |
| Phase 5 | debug-server::mobile_ws 拆解 | ⏳ 待开始 | |

**最近完成**：
- `closure_runtime_accumulator.rs` 抽取完成
- `closure_runtime` 已拆成 10 个子模块
- `fin-shared` 6 个测试全绿
