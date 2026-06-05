# Architecture Cleanup Plan

## 1. 目标与验收标准

目标：按 Phase 1-8 执行计划，去掉所有 fallback，统一 pipeline 命名，隔离 runtime 内部模块，建立唯一错误中心。

验收标准：

1. 全项目 `rg` 无 `fallback_ok`、`treat_invalid_as_success`、`silent_salvage`、`unwrap_or` 默认成功路径。
2. `fin-runtime` 内部模块收口为 domain 入口：`pipeline/`、`closure/`、`context/`、`tools/`、`session/`、`control/`、`error/`。
3. 临时文件名（`extended`、`v4a`、`support`、`helpers`）全部 rename 或删除，不保留空壳 re-export。
4. 主推理链失败必须走 `RuntimeError -> ErrorErr01..05 -> event + ledger + user_visible`，无其他 failure 出口。
5. Provider hub skeleton 全部 dead_code warning 消除。
6. 四条 pipeline 静态测试全绿：`cargo test -p fin-runtime`、`cargo test -p fin-provider`。

真源文档：
- `docs/architecture/02-layer-boundaries.md`（Phase 1 已冻结）
- `docs/architecture/09-workspace-and-crate-map.md`（Phase 1 已冻结）
- `docs/architecture/44-runtime-error-center.md`（Phase 1 已冻结）
- `docs/goals/pipeline-unique-type-refactor-plan.md`

## 2. 范围与边界

In scope：

1. Runtime 内部模块目录隔离。
2. Provider hub skeleton 接入或删除。
3. 错误中心主链接入。
4. 全项目 fallback/silent 搜索与清理。
5. Pipeline 命名红测增强。
6. Dead code 物理删除。
7. Static gate 增强。

Out of scope：

1. 不重写 provider protocol 逻辑。
2. 不改 contracts schema。
3. 不做 UI / Web 重构。
4. 不引入新的 feature，只做架构清理。

## 3. 实施步骤

### Phase 1（已完成）：冻结架构 docs 真源
- 状态：**已完成**
- 变更：
  - `docs/architecture/02-layer-boundaries.md`：runtime domain 边界、命名收口、错误链入口
  - `docs/architecture/09-workspace-and-crate-map.md`：crate owning contract、runtime 目标目录布局
  - `docs/architecture/44-runtime-error-center.md`：错误中心唯一主路径、禁止 fallback 规则
  - `AGENTS.md`：更新路由索引
- 证据：`5e71f1a docs(architecture): define error center`

### Phase 2：统一命名清单与 module inventory
- 目标：列出所有 runtime 模块，按 domain 分类，标记坏名
- 步骤：
  1. 生成 `runtime/src/*.rs` 全量清单
  2. 分类：pipeline / closure / context / tools / session / control / error
  3. 标记：extended / v4a / support / helpers / 泛化 test
  4. 输出 rename map
- 验收：清单经过 review，确认坏名范围

### Phase 3：去掉 fallback / silent salvage
- 目标：消除所有隐式默认成功路径
- 步骤：
  1. 全局搜索：`fallback`、`unwrap_or_else`、`default`、`salvage`、`unknown`、`not configured`
  2. 按 source class 分类：Input / Provider / Model / Tool / Runtime
  3. 每个命中点改为显式 `ErrorErr*` 路径
  4. 加红测禁止 fallback
- 验收：`rg` 主路径无禁止词

### Phase 4：接入唯一错误中心
- 目标：`M1Runtime::run_closure` 失败走 `ErrorErr*`
- 步骤：
  1. 把 `run_closure` 改为 `run_closure_inner`
  2. 外层统一捕获 `RuntimeError`
  3. 调用 `map_runtime_error_through_error_pipeline`
  4. 追加 error events
  5. 删除其他 failure 出口
- 验收：`rg map_runtime_error_through_error_pipeline` 有主链调用；失败测试有 event + ledger

### Phase 5：Runtime 内部模块目录隔离
- 目标：`runtime/src` 按 domain 收口
- 步骤：
  1. 建立 `pipeline/`：`input.rs`、`reason.rs`、`feedback.rs`、`error.rs`
  2. 建立 `closure/`：`run.rs`、`round.rs`、`retry.rs`、`events.rs`、`finalize.rs`、`state.rs`
  3. 建立 `tools/`：`catalog.rs`、`dispatch.rs`、`query.rs`、`patch.rs`、`exec.rs`、`task.rs`、`collab.rs`、`peer.rs`
  4. 建立 `context/`：`view.rs`、`blocks.rs`、`render.rs`、`project.rs`、`history.rs`
  5. 建立 `session/`：`materializer.rs`、`journal.rs`、`turn.rs`、`trace.rs`
  6. 建立 `control/`：`feedback.rs`、`plane.rs`、`routing.rs`、`scheduler.rs`、`owner_loop.rs`
  7. 迁移后物理删除旧文件
  8. 删除空壳 re-export
- 验收：`runtime/src` 根目录只有 `lib.rs` 和 domain 入口

### Phase 6：Provider hub skeleton 收口
- 目标：消除 hub 19 个 dead_code warning
- 步骤：
  1. 确认 hub 是否进入 `ProviderFacade::execute_prepared` 执行路径
  2. 真接入：消除 dead_code；或确认非 M1 范围后物理删除 skeleton
  3. 删除死文件后同步删除静态测试
- 验收：`cargo test -p fin-provider` 无 hub dead_code warning

### Phase 7：命名锁与边界锁
- 目标：static tests 禁止 extended / v4a / 跨 pipeline / impl From
- 步骤：
  1. 增强 `error_pipeline_static_tests.rs`：禁止 extended/v4a
  2. 增强 `reason_pipeline_static_tests.rs`：禁止跨节点
  3. 增强 `feedback_pipeline_static_tests.rs`：禁止跨节点
  4. 新增 crate dependency check：contracts/shared 不依赖 runtime/provider
- 验收：红测全绿

### Phase 8：验证矩阵
- L1：`cargo test -p fin-runtime -- --nocapture` + `cargo test -p fin-provider -- --nocapture`
- L2：static gate（命名 + 编号 + 禁止 fallback）
- L3：失败注入（provider 500、missing config、invalid tool）
- L4：Live provider smoke
- L5：Web/debug 只读 event，不生成第二状态

## 4. 风险与规避

1. 风险：目录迁移期间主路径中断。规避：每个 domain 迁移后立即跑测试，绿了再迁下一个。
2. 风险：删除 dead hub skeleton 后发现还有隐式引用。规避：先 `rg hub_pipeline` 确认无引用。
3. 风险：去掉 fallback 后现有测试失败。规避：红测先写，失败路径改走 ErrorErr*，不要把 fallback 当通过测试的手段。
4. 风险：错误中心接入后 `?` 传播路径变化导致编译器报错。规避：保留 typed error 在 inner 函数内传播，只在外层统一映射。

## 5. 完成定义 DoD

1. Phase 1-8 全部完成。
2. 所有 pipeline 静态测试全绿。
3. `rg` 无禁止 fallback/silent 词汇。
4. Provider hub dead_code warning 清零。
5. Runtime 模块已目录化，无空壳 re-export。
6. 证据 commit 推送到 `origin/main`。
