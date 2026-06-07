# Architecture Gap Review and Layered Hardening Plan — 2026-06-07

> 配套：`AGENTS.md`、`~/.codex/AGENTS.md`、`coding-principals`、已有 `docs/goals/architecture-hardening-layered-plan.md`、`docs/architecture/45-runtime-module-inventory.md`。
> 状态：2026-06-07 二次 review（前一轮 Layer 0-3 已 commit，但 10 个 commit 因 LibreSSL push 失败未同步到 origin）。

## 1. 当前已确认面

### 1.1 已完成并可被脚本验证

1. `fin-runtime` 已收口为 12 个 domain 子目录：`pipeline / closure / context / tools / session / control / task / agent / runtime_home / model / prompt / activity_cards`。
2. `pipeline/naming_static_tests.rs` 13 个测试全绿：禁 `extended / v4a / router / support / helpers`、禁 `mod *_pipeline` 旧 inline 命名、禁 `impl From` 跨域 shortcut。
3. `provider` crate 的 `hub_pipeline` 已物理删除；dead_code 清零。
4. `activity_cards` 已物理迁入 `activity_cards/` 子目录，根目录不再有 `#[path]` 桥。
5. `tools` 完成 Layer 4-5 重命名：消除 `dispatch_router_*`，全部 dispatch 子域物理命名。
6. `line-limit` gate 当前 PASS（`limit=500, whitelist_entries=1`）。
7. `cargo fmt --check` 与 `cargo test --workspace` 已在 CI 跑通。
8. 真实 provider / qqbot live receipt 落地，M1 closeout 已签收。

### 1.2 本地已提交但未推送（关键 push 风险）

`git log origin/main..HEAD` 有 10 个 commit：

```
ed9e8f4 fix(cli+runtime): derive test session paths from receipt + implement append_framework_events
0b14aed fix(runtime): persist event stream for all sources including ephemeral control-plane
6bbe966 fix(runtime): add exec/write_stdin/patch/tool receipt paths to authoritative_receipt_ref
c1991fa refactor(runtime): Layer 4+5 - tools owning-layer renames + dedupe authoritative_receipt_ref
889ca78 docs(inventory): activity_cards migrated to domain dir, update inventory map (Layer 3)
0ad6ceb refactor(runtime): split oversized modules to pass line-limit gate (Layer 2)
0728021 refactor(runtime): Layer 3 - move activity_cards into own domain directory
10405bb ci: Layer 1 - upgrade governance gate + add fmt check to CI
6022684 docs(architecture): Layer 0 - fix routing + inventory + add pipeline unique-type doc
e03cd4d docs(note): update architecture cleanup session progress
```

push 被 `LibreSSL SSL_connect: SSL_ERROR_SYSCALL` 阻断，必须先解决传输再谈新 layer。

## 2. 二次 review 发现的 Gap（按硬护栏分组）

### 2.1 全局 AGENTS.md 硬护栏对照

| 护栏（编号） | 状态 | 缺口 |
| --- | --- | --- |
| 1 先验证后结论 | OK | 已带 receipt |
| 2 禁止 fallback | OK（prod code 0 命中） | `model/parser.rs` 的 `salvage_control_feedback` 仍保留 — 必须明确为"硬伤显式暴露并标记 contract violation"，而非静默 salvage |
| 3 非授权不破坏 | OK | — |
| 4 禁止 broad kill | OK | — |
| 7 行为对齐 USER.md | OK | — |
| 8 Skills 精华沉淀 | 部分 | L4-L5 经验（tools 重命名易破 cli、`#[path]` 桥 <15 域）未沉淀到全局 skill |
| 9 真源追踪优先 | 部分 | `pipeline/naming_static_tests.rs` 同时承担 inventory gate 与 naming gate，职责未切清；建议拆为 `naming_static_tests` + `inventory_static_tests` |
| 10 冗余代码物理删除 | 缺口 | `rust/crates/orchestrator/src/lib.rs` 只剩 `advance_task` + tests；`rust/crates/registry/src/lib.rs` 只剩 `WorkerHeartbeat` + `RegistrySnapshot::register`；`rust/crates/transport-http/src/lib.rs` 只剩 `HttpTransportConfig::default`；`rust/crates/harness-core/src/lib.rs` 只剩 `ReplayScenario` struct。四个 crate 都是空壳，违反"禁止以防万一"原则 |
| 11 禁止批量 checkout | OK | — |
| 16 记忆管理 | OK | CACHE.md / note.md 同步 |
| 17 Pipeline 唯一类型锁定 | 部分 | `salvage_control_feedback` 是隐式 error→success shortcut，必须改名 `reject_invalid_control_feedback` 并改为 return `None` + emit `model.output_contract_violation` 事件 |
| 18 功能定位唯一真源 | 部分 | 已有 inventory，但 `model / prompt` 两 domain 的 verification map 仅占位；orchestrator/registry/transport-http/harness-core 无 owner |
| 19 命名 + map 双锁 | 部分 | `salvage_*` 在 contracts/event 流中同时是合法字段（`control_feedback_salvaged`）和禁止语义，必须文档化字段语义为"重解析后留痕" |
| 20 架构 gate 强制 | 缺口 | CI 只有 `governance + fmt + test`，缺 `verify-runtime-architecture.py` 类的 inventory 自动化校验；orchestrator/registry 等空壳 crate 没有"必须被引用，否则删"的红测 |
| 21 重复实现物理禁止 | 缺口 | `tools/dispatch_control_args.rs`、`tools/dispatch_peer_records.rs` 等仍以"辅助"语义命名（args/records/helper），是事实上的"support"层；应改名为 `dispatch_control_payload` / `dispatch_peer_payload` 等行为主语命名 |
| 22 测试映射显式化 | 部分 | `model` / `prompt` 两 domain 的 verification map 条目过少 |
| 23 Rust 迁移方向 | OK | runtime 已 Rust 化 |

### 2.2 项目 AGENTS.md 硬护栏对照

| 护栏 | 状态 | 缺口 |
| --- | --- | --- |
| 先验证后结论 | OK | — |
| 禁止静默失败 | 缺口 | `model/parser.rs::salvage_control_feedback` 仍把 invalid JSON salvage 为 `ControlFeedback`；事件 `control_feedback_salvaged` 让上游误以为成功 |
| 真实 payload 不可裁剪 | OK | — |
| 结构化事件 | OK | — |
| 禁止 owning 跨层复制 | 部分 | 4 个空壳 crate 没有任何 owning 责任 |
| inference/provider/tool 真实 E2E | OK | — |
| live timeout 按阶段设置 | OK | — |
| prompt 不压缩 | OK | — |

### 2.3 实际硬 gap（必须修）

| 编号 | 缺口 | 风险 | 优先级 |
| --- | --- | --- | --- |
| G1 | `origin/main` 落后本地 10 个 commit，push 被 SSL 阻断 | 后续所有 review 与协作拿不到本地真实状态 | P0 |
| G2 | `salvage_control_feedback` 是事实上的"吞 invalid 变 salvage 成功"；必须改为显式 `reject_invalid_control_feedback` 并发 `model.output_contract_violation` 事件 | 违反 AGENTS.md 护栏 2 / 17 / 项目护栏"禁止静默失败" | P0 |
| G3 | 四个空壳 crate（`orchestrator`/`registry`/`transport-http`/`harness-core`）是事实 dead semantic；按护栏 10 必须物理删除 | 长期维护负担 + 违反"禁止以防万一" | P0 |
| G4 | `tools` 仍有 `dispatch_control_args` / `dispatch_peer_records` / `dispatch_query_history` / `dispatch_query_image` / `dispatch_query_task` / `dispatch_exec_receipts` / `dispatch_result_receipts` / `dispatch_collab_mailbox` / `dispatch_collab_coordination` / `dispatch_task_write` / `dispatch_assignment` 等"以子类型"命名（实际是事实 support 层） | 命名 + map 双锁不达标 | P1 |
| G5 | CI 缺 inventory 自动化 gate：必须按 `rust/crates/runtime/src/*/mod.rs` 与 inventory 表格做一致性校验；并自动统计 `v4a / extended / helpers / support` 残留 | 架构 gate 强制护栏不达标 | P1 |
| G6 | `model` / `prompt` 两个 domain 的 function map / verification map 仍占位（Layer 3 文档未补全） | 命名 + map 双锁不达标 | P1 |
| G7 | `naming_static_tests.rs` 同时承担 naming + inventory 两种 gate 职责，违反"一个功能一个 owner"；应拆为 `naming_static_tests.rs` + `inventory_static_tests.rs` | 单 crate 内部职责混合 | P2 |

## 3. 分层修复设计

### Layer A：先解锁 push 风险（P0-前置）

> 任何本地修复都要先推到 origin，否则后续验证无法跨机器复现。

1. 解 SSL：先 `git config --global --unset http.proxy` / `git config --global --unset https.proxy`；不行就试 `GIT_SSL_NO_VERIFY=true git push` 仅作应急；最终方案应切 SSH remote 或修系统 SSL backend。
2. push 后再继续任何 Layer。
3. 验收：`git log origin/main..HEAD` 为空，且 `git status` clean。

### Layer B：消灭 `salvage_control_feedback` 隐式吞错（P0）

1. 在 `docs/architecture/46-pipeline-unique-type-and-error-chain.md` 中加一节"硬伤显式暴露契约"：禁止把 invalid JSON salvage 为合法 `ControlFeedback`；唯一允许的是 `reject_invalid_control_feedback` → 事件 `model.output_contract_violation` → 落 `error_ledger.json`。
2. `rust/crates/runtime/src/model/parser.rs`：把 `salvage_control_feedback` 重命名为 `reject_invalid_control_feedback`，返回 `Result<Option<ControlFeedback>, ParseRejection>`。
3. 同步改 `model/parser.rs` 内的 `ParsedControlFeedback` 字段名 `salvaged` → `rejected`（或保留 `salvaged: false` + 新增 `rejected: true`），并在 `ClosureTraceRecord` 把 `control_feedback_salvaged` 改名为 `control_feedback_rejected`。
4. `rust/crates/contracts`：`ControlFeedback` / `ClosureTraceRecord` schema 同步更新。
5. 红测：
   - invalid JSON → `ParseRejection::ContractViolation`，`control_feedback_rejected = true`，`model.output_contract_violation` 事件必发。
   - 部分有效 JSON → 不再吞，按字段白名单丢弃未知字段、保留合法字段，事件 `model.output_contract_partial_accept`。
6. 验收：
   - `rg "salvage_control_feedback|salvaged" rust/crates/runtime/src rust/crates/contracts/src` 仅出现在历史注释 / 字段名 "salvaged" 已废止声明。
   - `cargo test -p fin-runtime model --manifest-path rust/Cargo.toml` 全绿。

### Layer C：物理删除 4 个空壳 crate（P0）

1. 删除前必须先证明零引用：
   - `rg "use fin_orchestrator|fin_orchestrator::" rust/`  → 应为 0
   - `rg "use fin_registry|fin_registry::" rust/` → 应为 0
   - `rg "use fin_transport_http|fin_transport_http::" rust/` → 应为 0
   - `rg "use fin_harness_core|fin_harness_core::" rust/` → 应为 0
2. 实际使用情况需要先查；若有引用，需要把这些引用迁到真正 own 该语义的 domain（典型迁移：`advance_task` 迁到 `task/store.rs` 的状态机辅助；`WorkerHeartbeat` / `RegistrySnapshot` 迁到 `agent/registry` 新建子模块；`HttpTransportConfig` 迁到 `debug-server/src/http.rs`；`ReplayScenario` 迁到 `harness-core` 不在删除范围时迁移到 `tests_mainline.rs` 的 fixture）。
3. 删除步骤：
   - `git rm -r rust/crates/orchestrator rust/crates/registry rust/crates/transport-http`
   - 若 `harness-core` 不为空则保留，否则同上删除。
   - `rust/Cargo.toml` workspace members 列表移除。
   - 同步更新 `docs/architecture/09-workspace-and-crate-map.md`、`docs/architecture/45-runtime-module-inventory.md`。
4. 验收：
   - `rg "fin_orchestrator|fin_registry|fin_transport_http" rust/` 为 0。
   - `cargo test --workspace --manifest-path rust/Cargo.toml` 全绿。
   - `cargo build --workspace --manifest-path rust/Cargo.toml` 无 dead_code 警告来自被删 crate。

### Layer D：tools 命名收敛（P1）

1. 把 `tools/dispatch_*` 重新按 owning 主语归类：
   - 保留 `dispatch_query_*`（query 是 owning 主语）
   - `dispatch_control_args` → `dispatch_control_payload`
   - `dispatch_peer_records` → `dispatch_peer_payload`
   - `dispatch_exec_receipts` → 合并到 `dispatch_exec`（receipts 是 exec 的子产物，不应有独立 module）
   - `dispatch_result_receipts` → 合并到 `dispatch_patch` 或 `dispatch_exec`（按 owning call site 决定）
   - `dispatch_collab_mailbox` / `dispatch_collab_coordination` → 合并为 `dispatch_collab`（子函数化）
   - `dispatch_assignment` → 合并到 `task/managed_board` 或在 `tools` 留 `dispatch_assignment` 不变（看使用面）
   - `dispatch_task_write` → 改名为 `dispatch_task_mutation`（write 是动词弱命名，mutation 表达语义）
2. `naming_static_tests.rs` 加红测：
   - 禁 `*_args`、`*_records`（除非是 record 类型的 owning file）
   - 禁 `*_receipts`（receipts 不是 owning 主体）
3. 验收：
   - `rg "dispatch_.*_args|dispatch_.*_records|dispatch_.*_receipts" rust/crates/runtime/src/tools` 0 命中。
   - `cargo test -p fin-runtime tools --manifest-path rust/Cargo.toml` 全绿。

### Layer E：补 inventory 自动化 gate（P1）

1. 新增 `scripts/verify-runtime-architecture.py`：
   - 读 `rust/crates/runtime/src/*/mod.rs` 列表，与 `docs/architecture/45-runtime-module-inventory.md` 表格对比；不一致则失败。
   - 统计 `tools/mod.rs` 命名前缀，禁词表：`args$`、`records$`、`receipts$`（除非是 owning record 类型）。
   - 统计 `lib.rs` 顶层 mod 声明数与 domain 子目录数是否一致。
   - 统计 fallback / salvage / degrade 词在 prod code 出现次数（应为 0，除非有明确注释豁免）。
2. 接入 `.github/workflows/ci.yml`：在 `verify-governance.sh` 之后增加 `python3 scripts/verify-runtime-architecture.py`。
3. 验收：
   - 改一个 domain 文件名不更新 inventory，CI 应红。
   - 加一个 `dispatch_*_args` 命名，CI 应红。

### Layer F：补 model / prompt 两条 domain 的 function map / verification map（P1）

1. `docs/architecture/45-runtime-module-inventory.md`：
   - §4 function map 加 `model_output_parse`、`prompt_assembly`、`prompt_role_policy`、`prompt_catalog` 四条 feature_id 行。
   - §5 verification map 加对应必跑测试名。
2. 验收：人工 review 一致；自动校验 E Layer 已能 catch 缺漏。

### Layer G：拆分 naming_static_tests 职责（P2）

1. `rust/crates/runtime/src/pipeline/naming_static_tests.rs` → 拆为：
   - `naming_static_tests.rs`：纯命名模板 / 禁词 / 编号合法性。
   - `inventory_static_tests.rs`：domain 子目录 vs lib.rs mod 声明、跨域 import、fallback 词统计。
2. `lib.rs` 把 `mod naming_static_tests` 拆为两个 `mod`。
3. 验收：`cargo test -p fin-runtime pipeline::naming_static_tests pipeline::inventory_static_tests --manifest-path rust/Cargo.toml` 全绿。

## 4. 风险与规避

| 风险 | 缓解 |
| --- | --- |
| `salvage_control_feedback` 改名破坏下游 closure / digest 渲染 | 先 `rg` 全仓引用点；分两阶段：① 加新函数 + 旧函数 deprecation；② 跑通测试后删旧函数 |
| 删除 4 个空壳 crate 误伤隐藏引用 | 删除前先做引用 grep；引用优先迁到真正 own 语义的 domain；所有迁移必须带测试 |
| push 解锁过程中断 | 每次只做最小推送；Layer A 单独一个 commit；push 成功后再开 Layer B |
| 静态 gate 误报 | `verify-runtime-architecture.py` 必须支持白名单文件，且白名单有 owner + 到期时间 |
| 命名收敛后 cli 集成测试破 | `cargo test -p fin-cli` 必须先于 `cargo test -p fin-runtime` 跑 |

## 5. 验证矩阵

| 阶段 | 命令 |
| --- | --- |
| A | `git log origin/main..HEAD` 为空；`git status` clean |
| B | `rg "salvage_control_feedback" rust/crates/runtime/src` 0 命中；`cargo test -p fin-runtime model` 全绿 |
| C | `rg "fin_orchestrator\|fin_registry\|fin_transport_http\|fin_harness_core" rust/` 0 命中；`cargo test --workspace` 全绿 |
| D | `rg "dispatch_.*_args\|dispatch_.*_records\|dispatch_.*_receipts" rust/crates/runtime/src/tools` 0 命中；`cargo test -p fin-runtime tools` 全绿 |
| E | `python3 scripts/verify-runtime-architecture.py` 退出 0；CI 新增 step 红测通过 |
| F | `docs/architecture/45-runtime-module-inventory.md` §4 / §5 行数变化受 review 验证 |
| G | `cargo test -p fin-runtime pipeline::naming_static_tests pipeline::inventory_static_tests` 全绿 |

## 6. 完成标准

1. `origin/main` 与本地 HEAD 一致；10 个未推 commit 全部到位。
2. `salvage_control_feedback` 全部消失，替换为 `reject_invalid_control_feedback` + `model.output_contract_violation` 事件。
3. 4 个空壳 crate 物理删除；workspace members 同步收敛。
4. `tools` 命名通过新增红测锁住。
5. CI 新增 `verify-runtime-architecture.py` 步骤且默认红测通过。
6. inventory §4 / §5 完整覆盖 12 个 domain。
7. naming gate 与 inventory gate 物理拆分为两个 test module。
8. 每个 Layer 完成后立即 commit + push（push 失败时记录原因到 `note.md` 并继续下一项，最后一并推送）。
