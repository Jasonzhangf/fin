# 45 Runtime Module Inventory and Rename Map

> 配套 `docs/architecture/02-layer-boundaries.md`、`docs/architecture/09-workspace-and-crate-map.md`、`docs/architecture/44-runtime-error-center.md`、`docs/goals/architecture-cleanup-plan.md`。
> 实盘扫描：`rust/crates/runtime/src/` 2026-06-06。

## 1. domain 目录结构（当前真源）

`fin-runtime` 内部模块按 owning domain 收口为 11 个子目录入口：

| domain | 目录 | 入口文件 | 职责 |
| --- | --- | --- | --- |
| `pipeline` | `src/pipeline/` | `mod.rs` | input / reason / feedback / error 主链 + 4 套 static tests |
| `closure` | `src/closure/` | `mod.rs` | run / round / retry / events / finalize / state / records / checkpoint |
| `context` | `src/context/` | `mod.rs` | view / blocks / render / project_support + view tests |
| `tools` | `src/tools/` | `mod.rs` | catalog / dispatch + dispatch_extended_* 8 子域 + test_helpers |
| `session` | `src/session/` | `mod.rs` | materializer / materializer_events / materializer_support / journal / turn / trace |
| `control` | `src/control/` | `mod.rs` | feedback / plane / plane_segments / owner_loop / routing / scheduler |
| `task` | `src/task/` | `mod.rs` | assignment_queue / board_snapshot / handoff / managed_board / store |
| `agent` | `src/agent/` | `mod.rs` | naming + naming_tests |
| `runtime_home` | `src/runtime_home/` | `mod.rs` | skill_loader / source_visibility（`~/.fin` 持久化辅助） |
| `model` | `src/model/` | `mod.rs` | input assembler / parser / shapes + model tests |
| `prompt` | `src/prompt/` | `mod.rs` | assembly / basics / catalog / role_policy + prompt tests |

每个 domain 的 `mod.rs` 是该 domain 的唯一入口；上层模块通过 `crate::<domain>::<module>` 引用，禁止跨 domain 短路。

## 2. 当前 inventory（已落在子目录的模块清单）

| domain | 物理位置 | 状态 |
| --- | --- | --- |
| `pipeline` | `pipeline/{input,reason,feedback,error}_pipeline.rs` + 4 个 `_static_tests.rs` | 已落地 |
| `pipeline` | `pipeline/{naming,mod}.rs` | 已落地 |
| `closure` | `closure/{mod,events,finalize,records,retry,rounds,state,checkpoint}.rs` | 已落地 |
| `closure` | `closure_runtime_rounds_tools.rs` | 仍根目录（cross-domain 工具桥） |
| `closure` | `closure_runtime.rs` | 仍根目录（closure 编排主干） |
| `context` | `context/{mod,view,blocks,block_render,project_support,view_*,block_render_tests}.rs` | 已落地 |
| `tools` | `tools/*.rs` 34 个 | 已落地 |
| `session` | `session/{mod,materializer,materializer_events,materializer_support,journal,turn,trace}.rs` | 已落地 |
| `control` | `control/{mod,feedback,plane,plane_segments,owner_loop,routing,scheduler}.rs` + `plane_tests.rs` | 已落地 |
| `task` | `task/{mod,assignment_queue,board_snapshot,handoff,managed_board,store}.rs` | 已落地 |
| `agent` | `agent/{mod,naming,naming_tests}.rs` | 已落地 |
| `runtime_home` | `runtime_home/{mod,skill_loader,source_visibility}.rs` | 已落地 |

## 3. 仍位于根目录的模块（剩余 debt）

按 owning 关系归口后尚未迁入子目录的根级 `.rs`（37 个文件），分两类：

### 3.1 cross-domain 编排 / 测试入口（建议保持根目录）
- `lib.rs` — crate 入口
- `closure_runtime.rs` — closure 编排主干
- `closure_runtime_rounds_tools.rs` — closure 跨 tools 桥
- `tests.rs` / `tests_mainline.rs` / `tests_role_runtime.rs` / `tests_context_render.rs` — 顶层 mainline 红测
- `run_closure_error_center_tests.rs` — 错误中心红测

### 3.2 prompt / model / activity_cards（Phase 5 待落地）
prompt / model / activity_cards 三组模块存在大量跨域引用，bridge approach 多次失败，按 `note.md` 已知模式列入 Phase 5b/c/d 后续分批处理：
- `prompt_assembly.rs` + `prompt_tests*.rs` — 被 `context/view` + `model_input_assembler` 双向引用
- `model_output*.rs` / `model_input_assembler.rs` — 与 closure/retry / prompt_assembly 多向耦合
- `activity_cards*.rs` — 含 4 个 `#[path]` 子桥，迁入子目录需同步桥接
- `assembler_tests.rs` — 跨 prompt/model 测试
- `round_context.rs` — closure 跨 model 桥
- `round_loop_runtime_tests*.rs` — closure loop 回归测试
- `execution_checkpoint_tests.rs` — 历史 checkpoint 回归
- `model_output_runtime_tests*.rs` — model 路径回归
- `skill_loader.rs` 已迁入 `runtime_home/skill_loader.rs`，根级原文件已 `git rm` 物理删除。

## 4. function map（owner registry）

| feature_id | owner module | canonical types | canonical builders/parsers | forbidden paths |
| --- | --- | --- | --- | --- |
| `inference_mainline` | `pipeline::reason` | `ReasonReq01..05`, `ReasonResp06..09` | 唯一入口：`pipeline::reason::Reason*Pipeline::run` | closure 不得重写 model 语义；control 不得修改 payload |
| `feedback_chain` | `pipeline::feedback` | `FeedbackResp01..04` | 唯一入口：`pipeline::feedback::run_feedback_pipeline` | model 不得伪造 success-from-error |
| `error_center` | `pipeline::error` | `ErrorErr01..05` | 唯一入口：`closure_runtime::map_runtime_error_through_error_pipeline` | 禁止 fallback / silent salvage |
| `closure_orchestration` | `closure_runtime` + `closure::*` | `M1Runtime`, `ClosureRun` | 唯一入口：`M1Runtime::run_closure` | closure 不得绕过 `run_closure_inner` |
| `tool_dispatch` | `tools::tool_dispatch` | `ToolDispatchInput`, `ToolDispatchOutcome` | 唯一入口：`tools::tool_dispatch::execute_model_tools` | tool 不得写 event / ledger（属 runtime） |
| `context_assembly` | `context::view` | `MinimalContextView`, `ContextViewBuilder` | 唯一入口：`context::view::ContextViewBuilder::build` | control 不得重排 context block |
| `session_materialize` | `session::materializer` | `SessionMaterializer`, `SessionMessageRecord` | 唯一入口：`session::materializer::SessionMaterializer::materialize` | tool 不得直接写 session 目录 |
| `task_orchestration` | `task::store` + `task::handoff` | `StoredTaskRecord`, `TaskHandoffReceipt` | 唯一入口：`task::store::*` / `task::handoff::handoff_project_task` | control 不得直接修改 task status |
| `agent_identity` | `agent::naming` | `AllocatedAgentIdentity`, `AgentAssignmentSummary` | 唯一入口：`agent::naming::allocate_local_agent_identity` | tool 不得伪造 agent name |
| `control_plane` | `control::plane` | `ExecutionStateRecord`, `PendingInputDequeue` | 唯一入口：`control::plane::*` | session 不得修改 state machine |
| `runtime_home_persistence` | `runtime_home::skill_loader` + `runtime_home::source_visibility` | `LoadedSkill`, `uses_ephemeral_session_persistence` | 唯一入口：`runtime_home::skill_loader::load_global_skills` | 禁止 fallback 到默认 skill |

## 5. verification map（feature -> test gates）

| feature_id | 必跑 unit | 必跑 contract | 必跑 integration / red | 必跑 build / smoke |
| --- | --- | --- | --- | --- |
| `inference_mainline` | `tests::mainline` | `pipeline::reason_static_tests` | `run_closure_emits_expected_event_chain` | `cargo test -p fin-runtime` |
| `feedback_chain` | `prompt_tests_role_policy` | `pipeline::feedback_static_tests` | `feedback_pipeline_rejects_silent_invalid` | `cargo test -p fin-runtime` |
| `error_center` | `run_closure_error_center_tests` | `pipeline::error_static_tests` | `error_pipeline_classifies_source_and_runtime_decision` | `cargo test -p fin-runtime` |
| `closure_orchestration` | `tests::mainline`, `round_loop_runtime_tests` | `pipeline::naming_static_tests::lib_rs_no_helpers_or_support_mod_declarations` | `runtime_closure_uses_structured_user_response_for_session_visible_output` | `cargo test -p fin-runtime` |
| `tool_dispatch` | `tools::tool_dispatch_query_tests`, `tools::tool_dispatch_task_write_tests` | `tools::tool_dispatch_tests_*` | `tool_dispatch_*` 集成路径 | `cargo test -p fin-runtime` |
| `context_assembly` | `context::view_tests` | `pipeline::input_static_tests` | `context_view_builder_exposes_loaded_global_skills` | `cargo test -p fin-runtime` |
| `session_materialize` | `session::materializer` + `closure::*` | `pipeline::naming_static_tests::pipeline_modules_use_crate_path_not_legacy` | `session_materializer_persists_retry_attempt_provider_truth` | `cargo test -p fin-runtime` |
| `task_orchestration` | `task::store`, `task::managed_board` | `pipeline::naming_static_tests::tools_mod_v4a_mod_declarations_count_is_documented` | `system_owner_can_assign_worker_and_close_managed_task_loop` | `cargo test -p fin-runtime` |
| `agent_identity` | `agent::naming_tests` | `pipeline::naming_static_tests` | `persist_and_read_assignment_summary_round_trip` | `cargo test -p fin-runtime` |
| `control_plane` | `control::plane_tests` | `pipeline::naming_static_tests` | `dequeue_pending_input_marks_first_item_dequeued` | `cargo test -p fin-runtime` |
| `runtime_home_persistence` | `runtime_home::*` | `prompt_tests_basics::context_view_builder_exposes_loaded_global_skills` | n/a | `cargo test -p fin-runtime` |

## 6. 命名 / 边界 gate（参见 `pipeline/naming_static_tests.rs`）

- lib.rs 根 mod 声明禁止 `mod *_extended_*` / `mod *_v4a` / `mod *_support` / `mod *_helpers`（统一迁入子目录）
- 子目录 `mod.rs` 必须 `pub mod` 全部成员；禁止 `pub(super) re-export` 空壳
- 跨 domain 引用必须走 `crate::<domain>::<module>::<Item>`，禁止 `crate::<module>::<Item>` 直达
- `impl From<X> for Y` 仅允许在 owning domain 内；禁止跨 domain `From` shortcut
- `pub(super)` 仅限同 domain 内部；跨域需 `pub(crate)` 或 `pub` + 在 `mod.rs` re-export

## 7. Dead code 标记与回收

- `hub_pipeline` 已物理删除（commit a34766a），provider hub 19 个 dead_code warning 清零
- 5d/5e/5g 三个 prompt/model/activity_cards 迁移尝试因 cross-domain 桥接复杂度均已 revert；保留为 Phase 5b/c/d backlog
- `model_input_assembler.rs` 因 `prompt_assembly` / `context/view` 双向引用暂留根目录，标记为 Phase 5c backlog

## 8. 历史 lessons（promote 进全局 skill）

- 子目录 split 流程：先 `git mv` 物理文件 → 创建子目录 `mod.rs` → 修 `mod.rs` `pub mod` → `rg` 扫 cross-domain 引用 → 批量 `sed` 替换 → `cargo build` 验证 → `cargo test` 验证 → `git commit` + push
- `#[path]` 桥接的子文件命名时，必须先 `mod foo;` 声明，再在父文件内 `use crate::foo::Item` 引用；不能用 `crate::{super::foo::Item}` 路径
- `pub(super) fn` 在子目录内升级到 `pub(crate)` 是跨域调用的最小代价改动
- 跨域测试 helpers 应抽到 `tools/test_helpers.rs` 或对应子目录 `test_helpers.rs`，而非散落 root

## 9. update 责任

- 任何 module 迁入/迁出子目录必须同步更新 §1 §2 §3
- 任何新 feature 必须先在 §4 function map 注册 owner，再实现
- 任何 gate 红测失败必须回查 §6，并补充对应 §5 verification map 条目
