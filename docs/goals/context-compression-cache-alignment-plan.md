# Context Compression / Prompt Cache 对齐实施计划

关联审计：`docs/refactor/context-compression-cache-audit-2026-05-23.md`

## 1. 目标与验收标准

目标：把 `fin` 的 context/history build、context review、compression 统一成对 prompt cache 友好的单一链路，保证普通多轮任务/多轮绘画时稳定前缀最大化，只有达到阈值才 compact。

验收标准：

1. 请求装配顺序固定为：完全不变 → 极慢变化 → 慢变化 → append-only history → volatile tail。
2. 系统/Developer/Skills/role/tool static schema 不在普通 turn 反复重建；首 turn/full reinject 后只做 diff。
3. 最新工具调用、工具结果、当前用户输入始终位于 prompt 尾部。
4. compact 只由 token usage / context budget threshold / context limit 风险触发，普通 turn 不 compact。
5. compact 结果是 history replacement：summary + selected recent records + current tail，不是只写 current_context snapshot。
6. 多轮绘画记录以 append-only artifact records 管理，compact 后保留 image/artifact refs。
7. provider request/response 有 prompt cache key 与 usage/cached token 可观测字段；不支持 usage 的 provider 必须显式记录缺失。
8. 回归矩阵覆盖 request shape、baseline diff、compact trigger、compact replacement、多轮绘画 fixture。

## 2. 范围与边界

### In Scope

- `rust/crates/provider/src/lib.rs`
- `rust/crates/runtime/src/model_input_assembler.rs`
- `rust/crates/runtime/src/context_view.rs`
- `rust/crates/runtime/src/round_context.rs`
- `rust/crates/runtime/src/session_materializer.rs`
- `rust/crates/cli/src/web_debug_turns.rs`
- `rust/crates/cli/src/session_commands.rs`
- `rust/crates/config/src/lib.rs`
- 新增 context budget / assembly plan / compaction engine 相关 runtime/contracts 文件
- request shape / compact / drawing fixture tests

### Out of Scope

- 不改 Android UI 作为业务真源。
- 不通过裁剪真实 payload 来“提速”。
- 不引入 fallback provider 或静默降级。
- 不删除旧 compact/rebuild 代码，除非已证明替代链路覆盖并获得清理授权。

## 3. 设计原则

1. **稳定前缀优先**：越稳定越靠前；越接近当前 turn 越靠后。
2. **普通 turn append/diff**：不做 full rebuild；不触发 compact。
3. **阈值 compact**：基于 usage/context budget，而不是凭轮次或手工 recent window。
4. **唯一真源**：context review 与 compression 共用 `ContextBudgetManager / ContextAssemblyPlanner`。
5. **可观测**：每次 plan/compact 有 artifact 和事件证据。
6. **fin artifacts 不丢失**：digest/reasoning/tool/image refs 是 compact 输入和输出约束。

## 4. 技术方案

### 4.1 Provider observability

- 增加 `prompt_cache_key` 到 request。
- 增加 `TokenUsage` 到 response。
- provider 不返回 usage 时写 `usage: null` 与 `usage_source: unsupported`。

### 4.2 ContextAssemblyPlan

新增 plan 数据结构，至少包含：

- section id / layer / stability class
- section hash
- token estimate
- source artifact refs
- included/excluded reason
- compact trigger decision

### 4.3 ContextBaseline

- session 级 `context/baseline.json`。
- 首 turn/full reinject/compact 后更新 baseline。
- 普通 turn 只生成 diff item。

### 4.4 Compact engine

- 输入：history + digest/reasoning/tool/image artifacts + budget snapshot。
- 触发：pre-turn threshold、mid-turn context limit、manual `/compact`。
- 输出：compacted summary record + selected recent records + retained artifact refs + new baseline。

### 4.5 Drawing iteration records

新增或复用 artifact record，记录：

- iteration id
- user prompt / edit instruction
- image refs / seed / model / size
- accepted/rejected notes
- next edit target

compact 时保留 refs，压缩旧自然语言说明。

## 5. 风险与规避

| 风险 | 规避 |
|---|---|
| provider usage 缺失 | 先做估算分支，但报告 evidence weakness，不宣称真实 token threshold |
| prompt 顺序调整影响模型行为 | snapshot + live provider smoke 分阶段验证 |
| compact summary 丢绘画 refs | compact schema 强制 artifact refs retention |
| 双路径 compact/rebuild 并存 | 最终清理旧 rebuild 或改名 diagnostic，避免冒充 compression |
| 大文件膨胀 | 新模块切片，遵守 500 行限制 |

## 6. 测试计划

1. `model_input_assembler` order snapshot。
2. `context_assembly_plan` stability class ordering。
3. baseline first-turn/full + second-turn diff。
4. low usage no compact / high usage compact。
5. manual `/compact` 走同一 compact engine。
6. mid-turn tool follow-up context limit compact。
7. drawing fixture compact retains image refs。
8. provider request record contains prompt_cache_key and usage fields。
9. full Rust targeted test + Android unaffected smoke（只验证 UI 没承担 context 真相）。

## 7. 实施步骤

1. 写 failing snapshot tests，锁定当前 request shape 风险。
2. 增加 provider usage/cache key contract 与 records。
3. 新增 `ContextAssemblyPlan` 和稳定分层 enum。
4. 重构 assembler 消费 plan，调整稳定前缀顺序。
5. 实现 session baseline/diff。
6. 实现 budget manager 与 compact trigger。
7. 实现 compact replacement engine。
8. 接入 `/compact` 与 auto compact。
9. 加入 drawing iteration fixture。
10. 清理或改名旧 rebuild snapshot 路径。

## 8. 完成定义

- 所有验收标准均有测试或 artifact 证明。
- 审计文档中的待审批修改点已逐项落地或明确关闭。
- request shape 证明稳定前缀不被普通 turn 动态 history 破坏。
- compact 证明只在阈值触发，并产生 history replacement。
- 多轮绘画 fixture 证明 artifact refs 不丢失。

## 9. 2026-05-23 实现关闭记录

本节用于关闭审计文档中的待审批修改点，保持实现证据可追踪。

| 审计修改点 | 状态 | 实现/证据 |
|---|---|---|
| A. `ContextBudgetManager / ContextAssemblyPlanner` 作为 context review / compression 决策入口 | 已实现 | `rust/crates/runtime/src/context_assembly_plan.rs`、`rust/crates/runtime/src/context_budget.rs`；`closure_runtime_rounds::execute_round` 先构建 plan，再由 budget manager 决策是否 compact。 |
| A. plan 需包含 sections/order/hash/token/source refs/included reason/trigger | 已实现 | `ContextAssemblySection` 含 `section_id/stability/section_hash/token_estimate/source_artifact_refs/included_reason`；`ContextBudgetSnapshot` 含 threshold 与 trigger。 |
| B. provider request `prompt_cache_key` | 已实现 | `rust/crates/provider/src/lib.rs`、`rust/crates/contracts/src/records.rs`、`rust/crates/runtime/src/turn_records.rs`；测试 `prepare_request_preserves_prompt_cache_key`、`runtime_records_provider_prompt_cache_key_and_usage`。 |
| B. provider response usage/cached/reasoning tokens | 已实现 | `TokenUsage` / `TokenUsageRecord`；Anthropic usage parse 测试 `anthropic_response_parses_usage_and_cached_tokens`；runtime record 测试覆盖。 |
| C. stable prefix 重排，current request 在尾部 | 已实现 | `ModelInputAssembler` 消费 `ContextAssemblyPlan`；测试 `model_input_assembler_orders_stable_prefix_before_history_and_current_tail`。 |
| D. `ContextBaseline` / baseline diff | 已实现 | `rust/crates/runtime/src/context_baseline.rs`；`SessionMaterializer` 写 `context/baseline.json` 与 `runtime/current/current_context_baseline.json`；测试 `context_baseline_requires_full_once_then_diff_when_stable_prefix_unchanged`。 |
| E. `/compact` 从 rebuild snapshot 改为 history replacement | 已实现 | `rust/crates/runtime/src/context_compaction.rs`；`rust/crates/cli/src/session_commands.rs` 的 `/compact` 调用 `ContextCompactionEngine`，写 `context/compacted_history.json` 与 `context/compaction-events.jsonl`；旧 rebuild-index 保留为 diagnostic。 |
| E. auto compact 与 `/compact` 共用 engine | 已实现 | `closure_runtime_rounds::execute_round` 与 CLI `/compact` 均调用 `ContextCompactionEngine`；测试 `runtime_auto_compact_replaces_history_before_provider_request_when_budget_exceeds_threshold`。 |
| F. 多轮绘画 refs 不丢 | 已实现（复用 artifact records） | 暂不新增独立 drawing schema，复用 `DigestRecord.artifact_candidates` 与 `ToolExecutionRecord.artifact_refs`；测试 `compact_engine_replaces_history_and_retains_drawing_artifact_refs` 和 auto compact 测试覆盖 `images/iter-1.png`。 |
| 回归矩阵覆盖 request shape/baseline/trigger/replacement/drawing/provider usage | 已实现 | `scripts/regression/run_local_regression.sh` 增加 `g1_context_cache_assembly_tests`、`g1_context_cache_round_loop_tests`、`g1_provider_cache_usage_tests`；CI 已运行该 local regression 脚本。 |

### 当前明确关闭说明

- `conversation/compacted_history.json` 在实现中采用等价路径 `context/compacted_history.json`，因为 context/session materializer 是上下文真相目录；conversation 目录只保留用户可见消息 ledger。
- 旧 `current_rebuild_index.json` / `context/rebuild-index.json` 未物理删除，按计划文档 Out of Scope 与审计建议保留为 diagnostic，但不再冒充 compression truth；其 payload 指向 `ContextCompactionEngine` 产物。
- provider 不返回 usage 时仍为 `usage: null`，threshold compact 可使用 runtime estimate，并在 `ContextBudgetDecision.evidence_strength = "weak"` 中显式标记证据弱度；不宣称 provider-token 真实闭环。

### 2026-05-23 追加关闭证据

- 普通 turn 默认不 compact：`default_context_budget_does_not_compact_normal_turn` 证明 `ContextAssemblyPlanner::default()` 的 120k 阈值下普通消息为 `NoCompact`。
- mid-turn/tool follow-up compact：`runtime_mid_turn_tool_followup_compacts_when_context_exceeds_budget` 证明第一轮小上下文不 compact，工具 follow-up 因上一轮巨大 assistant/tool context 超预算而 compact。
- Assembly plan artifact：`SessionMaterializer` 写 `runtime/current/current_context_assembly_plan.json` 与 `sessions/.../context/assembly-plan.json`。
