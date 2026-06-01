# 红测补齐 — 实现文档

## 目标与验收标准

### 主目标
按本文档补齐 fin 项目黑盒红测（纯 Rust `#[test]` + 合成数据），优先 P0/P1，确保关键模块功能契约可由测试锁定。

### 验收标准
- `cd rust && cargo test --workspace --no-run` 编译通过
- `cd rust && cargo test --workspace` 功能通过
- P0/P1 所列模块必须出现对应测试文件或内联测试

## 范围

### In Scope
- P0: config / contracts / context_compaction
- P1: runtime 状态管理（closure/owner_loop/scheduler/task_store）
- P2: runtime 数据层（assignment_queue/context_budget/control_feedback）
- P3: runtime 辅助（append_only_message_log/skill_loader/tool_semantics）
- P4: 其他 crate（orchestrator/harness-core/debug-server/provider）

### Out of Scope
- 真实 provider E2E 闭环（不依赖真实 LLM）
- 重构业务逻辑（只做最小必要修复）
- Web/Android 集成测试

## 设计原则
1. 每个模块至少锁定：happy path、invalid path、serde roundtrip（如适用）
2. 测试优先使用公开 API；不为了测试扩大 public API
3. 文件系统测试使用临时目录，测试后清理
4. 禁止 mock 真实 LLM/provider

## 技术方案

### 新建测试文件清单

| 优先级 | 目标文件 | 说明 |
|--------|---------|------|
| P0 | `runtime/src/context_compaction_tests.rs` | CompactEngine 契约 |
| P0 | `contracts/src/records_tests.rs` | 所有 records/feedback/daemon 契约 |
| P0 | 内联追加 | `config/src/startup.rs` 现有 mod tests |
| P0 | 内联追加 | `config/src/provider_profile.rs` 现有 mod tests |
| P1 | `runtime/src/closure_runtime_tests.rs` | M1Runtime 状态机 |
| P1 | `runtime/src/owner_loop_tests.rs` | derive_owner_loop_action 5 分支 |
| P1 | `runtime/src/scheduler_tests.rs` | derive_scheduler_decision |
| P1 | `runtime/src/task_store_tests.rs` | StoredTaskRecord serde |
| P2 | `runtime/src/assignment_queue_tests.rs` | AssignmentRecord serde |
| P2 | `runtime/src/context_budget_tests.rs` | ContextBudgetManager 决策 |
| P2 | `runtime/src/control_feedback_tests.rs` | ControlFeedbackBuilder |
| P3 | `runtime/src/append_only_message_log_tests.rs` | append/compact 不变量 |
| P3 | `runtime/src/skill_loader_tests.rs` | skill 加载/summarize |
| P3 | `runtime/src/tool_semantics_tests.rs` | semantic_views 映射 |
| P3 | `shared/src/shared_tests.rs` | io roundtrip + agents enum |
| P4 | `harness-core/src/harness_tests.rs` | ReplayScenario serde |
| P4 | 追加 | orchestrator full lifecycle |
| P4 | 追加 | provider parsing/endpoint |

### lib.rs 修改清单

每个新建测试文件需在对应 crate 的 `lib.rs` 添加：
```rust
#[cfg(test)]
mod xxx_tests;
```

## 风险与规避

| 风险 | 规避 |
|------|------|
| 某些私有函数无法从测试访问 | 优先使用公开 API + serde roundtrip；必要时评估最小只读 accessor |
| 测试写完后业务逻辑发现缺口 | 先写红测确认失败，再实现最小修复 |
| 编译失败扩散到已有测试 | 分 crate 逐步验证，每阶段跑 `cargo test -p <crate>` |

## 测试计划

### Phase 1 — P0
1. config/src/startup.rs 内联追加
2. config/src/provider_profile.rs 内联追加
3. contracts/src/records_tests.rs 新建
4. runtime/src/context_compaction_tests.rs 新建

### Phase 2 — P1
5. runtime/src/owner_loop_tests.rs 新建
6. runtime/src/scheduler_tests.rs 新建
7. runtime/src/task_store_tests.rs 新建
8. runtime/src/closure_runtime_tests.rs 新建

### Phase 3 — P2/P3
9. P2/P3 测试文件新建
10. `cargo test --workspace`

### Phase 4 — P4
11. P4 crate 补测
12. `cargo test --workspace` 全通

## 实施步骤

1. 读取 `plans/red-test-remediation-2026-06-01.md` 中各模块详细用例
2. 按 Phase 顺序逐模块实施
3. 每完成一个 crate，执行对应 `cargo test -p <crate>`
4. 编译通过后执行 `cargo test --workspace`
5. 每阶段发现写入 `note.md`
6. 收口时提炼到 `MEMORY.md`

## 完成定义（DoD）
- P0/P1 所有模块有红测覆盖并通过
- P2/P3/P4 模块有红测覆盖并通过
- `cargo test --workspace` 全通
- 更新的 `note.md` / `MEMORY.md`
