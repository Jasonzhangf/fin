# 46 Pipeline Unique Type and Error Chain

本文档是 pipeline unique type 规则的真源路由文档，指向详细设计。

## Pipeline Unique Type Rules

所有关键流水线节点必须使用 `<Domain><Direction><NN><Node>` 命名模板。

**详细设计真源**：`docs/goals/pipeline-unique-type-refactor-plan.md`

### 四条主链

1. **用户输入链** `InputIn*`：channel raw → normalized → operation → session bound → reasoning seed
2. **推理逻辑链** `ReasonReq*` / `ReasonResp*`：seed → context plan → budgeted → rendered → provider call → model output → parsed → decision → closure
3. **Provider 调用链** `HubReq*` / `HubResp*`：inbound → process → outbound wire → inbound response → process → outbound
4. **反馈响应链** `FeedbackResp*`：model raw → tagged blocks → user visible → control → tool intent → session materialized → channel render

### 节点编号稳定性

- 编号是 contract；已发布/已消费的编号不可重排、不可复用、不可改语义。
- 默认禁止中间插节点，新增能力优先进入既有节点内部 block / validator / parser。
- 确需改变中段语义时，只能链尾追加或开启新 chain version。
- 禁止 `03a` / `03_1` / `03.5` / `V2` 等临时编号进入主路径。

### 禁止行为

- 禁止跨节点 shortcut、重复 DTO、同义 struct、散落 `From` 转换。
- 禁止在非相邻节点间构造或转换。
- Pipeline 节点只允许相邻 builder/parser 转换。

### 错误链路由

Pipeline 错误必须走 `ErrorErr*` 链，详见 `docs/architecture/44-runtime-error-center.md`。

错误链与 pipeline 链的交叉点：

- `ReasonResp07ParsedContract` 解析失败 → `ErrorErr01Detected`（source: Model）
- `ReasonResp08RuntimeDecision` 决策失败 → `ErrorErr01Detected`（source: Runtime）
- `HubResp05Process` 协议解析失败 → `ErrorErr01Detected`（source: Provider）

## 设计文档索引

| 文档 | 内容 |
| --- | --- |
| `docs/goals/pipeline-unique-type-refactor-plan.md` | 四条链详细节点表、ownership、禁止行为 |
| `docs/architecture/44-runtime-error-center.md` | ErrorErr chain contract、event model、no-fallback rules |
| `docs/architecture/02-layer-boundaries.md` | Layer 边界、runtime domain owning |

## 静态门禁

运行时验证：`rust/crates/runtime/src/pipeline/naming_static_tests.rs`

当前已锁定的 gate（截至 2026-06-06）：

- `error_pipeline_no_forbidden_numbering`：禁止 `ErrorErr03a` 等临时编号
- `feedback_pipeline_no_from_no_fallback`：禁止 `impl From<` 和 fallback/salvage
- `pipeline_pipeline_nodes_never_use_legacy_inline_node_name`：禁止 `ErrorReq` 等旧名进入 pipeline
- `lib_rs_extended_mod_declarations_count_is_documented`：lib.rs 禁止 `tool_dispatch_extended_*` 声明
- `tools_mod_v4a_mod_declarations_count_is_documented`：tools/mod.rs 允许 1 个 `_v4a`（Phase 5d backlog）
- `domain_dirs_have_no_fallback_or_salvage`：各 domain `mod.rs` 禁止出现 `fallback` 字样
- `cross_domain_no_direct_crate_file_imports`：禁止跨 domain 使用旧 crate 路径

待扩展的门禁（Layer 5 目标）：

- 禁止跨节点 shortcut：`pipeline_nodes_never_use_non_adjacent_conversion`
- 禁止非相邻 `impl From<` 跨 pipeline

## 更新规则

- 新增 pipeline 节点类型必须先在本 doc 登记 owner 和节点编号。
- 节点编号变更必须伴随 migration plan 和旧编号物理删除计划。
- `skills/fin-general-dev/SKILL.md` 路由指向本文件。
