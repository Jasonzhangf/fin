# 45 Runtime Module Inventory and Rename Map

> 配套 `docs/architecture/02-layer-boundaries.md`、`docs/architecture/09-workspace-and-crate-map.md`、`docs/architecture/44-runtime-error-center.md`、`docs/goals/architecture-cleanup-plan.md`。
> 来源：`rust/crates/runtime/src/` 实盘扫描（2026-06-05）。

## 1. 当前 inventory（按 owning domain 分类）

| domain | 当前文件 | 状态 | 目标位置 |
| --- | --- | --- | --- |
| `error` | `error_pipeline.rs` | OK | `pipeline/error.rs` |
| `error` | `error_pipeline_static_tests.rs` | OK | `pipeline/error_static_tests.rs` |
| `pipeline` | `input_pipeline.rs` | OK | `pipeline/input.rs` |
| `pipeline` | `input_pipeline_static_tests.rs` | OK | `pipeline/input_static_tests.rs` |
| `pipeline` | `reason_pipeline.rs` | OK | `pipeline/reason.rs` |
| `pipeline` | `reason_pipeline_static_tests.rs` | OK | `pipeline/reason_static_tests.rs` |
| `pipeline` | `feedback_pipeline.rs` | OK | `pipeline/feedback.rs` |
| `pipeline` | `feedback_pipeline_static_tests.rs` | OK | `pipeline/feedback_static_tests.rs` |
| `closure` | `closure_runtime.rs` | OK | `closure/run.rs` |
| `closure` | `closure_runtime_rounds.rs` | OK | `closure/round.rs` |
| `closure` | `closure_runtime_rounds_tools.rs` | OK | `closure/round_tools.rs` |
| `closure` | `closure_runtime_contract_retry.rs` | OK | `closure/retry.rs` |
| `closure` | `closure_runtime_state.rs` | OK | `closure/state.rs` |
| `closure` | `closure_runtime_events.rs` | OK | `closure/events.rs` |
| `closure` | `closure_runtime_records.rs` | OK | `closure/records.rs` |
| `closure` | `closure_runtime_finalize.rs` | OK | `closure/finalize.rs` |
| `closure` | `closure_runtime_checkpoint.rs` | OK | `closure/checkpoint.rs` |
| `context` | `context_view.rs` | OK | `context/view.rs` |
| `context` | `context_blocks.rs` | OK | `context/blocks.rs` |
| `context` | `context_block_render.rs` | OK | `context/render.rs` |
| `context` | `context_project_support.rs` | OK | `context/project.rs` |
| `context` | `tool_history_render.rs` | OK | `context/history.rs` |
| `context` | `round_context.rs` | OK | `context/round_input.rs` |
| `tools` | `tool_catalog.rs` | OK | `tools/catalog.rs` |
| `tools` | `tool_catalog_dynamic.rs` | OK | `tools/catalog_dynamic.rs` |
| `tools` | `tool_catalog_task_tools.rs` | OK | `tools/catalog_task.rs` |
| `tools` | `tool_dispatch.rs` | OK | `tools/dispatch.rs` |
| `tools` | `tool_dispatch_assignment.rs` | rename | `tools/dispatch_assignment.rs`（已合规） |
| `tools` | `tool_dispatch_control.rs` | rename | `tools/dispatch_control.rs`（已合规） |
| `tools` | `tool_dispatch_control_support.rs` | rename | `tools/dispatch_control_helper.rs`（去 `support`） |
| `tools` | `tool_dispatch_peer.rs` | rename | `tools/dispatch_peer.rs`（已合规） |
| `tools` | `tool_dispatch_peer_support.rs` | rename | `tools/dispatch_peer_helper.rs`（去 `support`） |
| `tools` | `tool_dispatch_result_receipts.rs` | rename | `tools/dispatch_receipts.rs`（已合规） |
| `tools` | `tool_dispatch_extended.rs` | **删除** | 迁入 `tools/dispatch.rs` |
| `tools` | `tool_dispatch_extended_collab.rs` | **删除** | 迁入 `tools/dispatch_collab.rs` |
| `tools` | `tool_dispatch_extended_collab_coordination.rs` | **删除** | 迁入 `tools/dispatch_collab.rs`（子模块） |
| `tools` | `tool_dispatch_extended_collab_mailbox.rs` | **删除** | 迁入 `tools/dispatch_collab.rs`（子模块） |
| `tools` | `tool_dispatch_extended_exec.rs` | **删除** | 迁入 `tools/dispatch_exec.rs` |
| `tools` | `tool_dispatch_extended_exec_receipts.rs` | **删除** | 迁入 `tools/dispatch_exec.rs`（子模块） |
| `tools` | `tool_dispatch_extended_patch.rs` | **删除** | 迁入 `tools/dispatch_patch.rs` |
| `tools` | `tool_dispatch_extended_patch_utils.rs` | **删除** | 迁入 `tools/dispatch_patch.rs`（子模块） |
| `tools` | `tool_dispatch_extended_patch_v4a.rs` | **删除** | 临时版本，迁入 `tools/dispatch_patch.rs`（子模块） |
| `tools` | `tool_dispatch_extended_query.rs` | **删除** | 迁入 `tools/dispatch_query.rs` |
| `tools` | `tool_dispatch_extended_query_control.rs` | **删除** | 迁入 `tools/dispatch_query.rs`（子模块） |
| `tools` | `tool_dispatch_extended_query_history.rs` | **删除** | 迁入 `tools/dispatch_query.rs`（子模块） |
| `tools` | `tool_dispatch_extended_query_image.rs` | **删除** | 迁入 `tools/dispatch_query.rs`（子模块） |
| `tools` | `tool_dispatch_extended_query_task.rs` | **删除** | 迁入 `tools/dispatch_query.rs`（子模块） |
| `tools` | `tool_dispatch_extended_task_write.rs` | **删除** | 迁入 `tools/dispatch_task_write.rs` |
| `tools` | `tool_dispatch_extended_task_write_claim_guard.rs` | **删除** | 迁入 `tools/dispatch_task_write.rs`（子模块） |
| `tools` | `tool_semantics.rs` | OK | `tools/semantics.rs` |
| `session` | `session_materializer.rs` | OK | `session/materializer.rs` |
| `session` | `session_materializer_events.rs` | OK | `session/materializer_events.rs` |
| `session` | `session_materializer_support.rs` | rename | `session/materializer_helper.rs`（去 `support`） |
| `session` | `session_record_journal.rs` | OK | `session/journal.rs` |
| `session` | `trace_records.rs` | OK | `session/trace_records.rs` |
| `session` | `turn_records.rs` | OK | `session/turn_records.rs` |
| `session` | `task_store.rs` | OK | `session/task_store.rs` |
| `session` | `task_handoff.rs` | OK | `session/task_handoff.rs` |
| `session` | `managed_task_board.rs` | OK | `session/managed_task_board.rs` |
| `session` | `task_board_snapshot.rs` | OK | `session/task_board_snapshot.rs` |
| `session` | `activity_cards.rs` | OK | `session/activity_cards.rs` |
| `session` | `activity_cards_agents.rs` | rename | `session/activity_cards_agents.rs`（已合规） |
| `session` | `activity_cards_helpers.rs` | rename | `session/activity_cards_helper.rs`（去 `helpers`） |
| `session` | `activity_cards_store.rs` | OK | `session/activity_cards_store.rs` |
| `session` | `source_visibility.rs` | OK | `session/source_visibility.rs` |
| `control` | `control_feedback.rs` | OK | `control/feedback.rs` |
| `control` | `control_plane.rs` | OK | `control/plane.rs` |
| `control` | `control_plane_segments.rs` | OK | `control/plane_segments.rs` |
| `control` | `routing_actions.rs` | OK | `control/routing_actions.rs` |
| `control` | `scheduler.rs` | OK | `control/scheduler.rs` |
| `control` | `owner_loop.rs` | OK | `control/owner_loop.rs` |
| `model` | `model_input_assembler.rs` | OK | `closure/model_input.rs` |
| `model` | `model_output.rs` | OK | `closure/model_output.rs` |
| `model` | `model_output_shapes.rs` | OK | `closure/model_output_shapes.rs` |
| `model` | `model_output_tool_calls.rs` | OK | `closure/model_output_tool_calls.rs` |
| `model` | `model_output_feedback.rs` | OK | `closure/model_output_feedback.rs` |
| `prompt` | `prompt_assembly.rs` | OK | `closure/prompt_assembly.rs` |
| `naming` | `agent_naming.rs` | OK | `session/agent_naming.rs` |
| `support` | `skill_loader.rs` | OK | `session/skill_loader.rs` |
| `support` | `assignment_queue.rs` | OK | `session/assignment_queue.rs` |
| `tests` | 22 个 `*_tests*.rs` 文件 | 保留位置 | 按域就近或集中 `tests/` |

## 2. 命名收口规则

1. `extended` / `v4a` 必须删除；多文件聚合为单一 domain 模块。
2. `support` / `helpers` 仅作为迁移信号；落地后去前缀/去后缀或迁入子模块。
3. `tests` / `static_tests` 名称保持稳定（Rust 编译要求），但必须随主模块迁入 domain 目录。
4. 旧文件名迁移后必须物理删除；禁止保留空壳 re-export。

## 3. 实施顺序（与 cleanup plan 同步）

1. Pipeline / error / closure / context / tools / session / control 七大 domain 目录一次性建好。
2. 按 inventory 顺序逐个 domain 迁移：error → pipeline → closure → context → tools → session → control。
3. 每个 domain 迁移后跑 `cargo test -p fin-runtime` 全测，确认无 regression。
4. 旧文件物理删除（`git rm`）。
5. Phase 5 完成后跑 cargo test 全量与 dead_code 扫描。

## 4. 已知 blocker

- `extended` 文件中存在 `v4a` 临时版本名，属于历史多次补丁累积；迁移前必须先 `rg` 确认 v4a 与最新 `_patch.rs` 内容差，再决定合并。
- 17 个 `tool_dispatch_extended_*` 文件跨多个子能力（collab / exec / patch / query / task_write），重组后单文件会超过 1500 行；必须拆子模块。
- `round_loop_runtime_tests_*` 5 个文件跨多主题；需保留多文件结构（Rust 编译限制），只迁移位置。
- provider hub skeleton（`hub_pipeline.rs` 19 个 dead_code warning）属 Phase 6，本文档不处理。
