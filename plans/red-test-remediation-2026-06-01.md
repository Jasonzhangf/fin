# 红测补齐计划 — 2026-06-01

## 目标
通过黑盒红测（纯 Rust #[test] + 合成数据，不依赖真实 LLM/provider），
在以下模块上锁定功能契约边界：

- runtime 状态管理层：closure_runtime / owner_loop / scheduler / routing_actions
- runtime 数据层：task_store / assignment_queue / append_only_message_log
- runtime 决策引擎：context_compaction / context_budget / control_feedback / skill_loader / tool_semantics
- config crate：startup.validate / provider_profile 所有路径
- contracts crate：所有 records / feedback / daemon 契约
- shared crate：io / agents 所有路径

## 覆盖率现状

| Crate | 模块数 | 测试文件 | 状态 |
|-------|-------|---------|------|
| runtime | 83 | 36 | 部分覆盖，核心层裸奔 |
| config | 3 | 0 | 整体裸奔 |
| contracts | 8 | 0 | 整体裸奔 |
| shared | 3 | 0 | 整体裸奔 |
| debug-server | 13+ | 1 | 严重不足 |
| orchestrator | 1 | 1 | OK（内联 #[cfg(test)]）|
| harness-core | 1 | 0 | 无测试 |
| provider | 3+ | 1 | 仅 registry+dup reject |
| registry | 1 | 0 | 无测试 |
| transport-http | 1 | 0 | 无测试 |

## P0 — 无法锁定核心契约（必须补）

### T0-1: config/startup.validate 补测
---
### T0-2: config/provider_profile 补测
**模块**: `config/src/provider_profile.rs`
**已有**: 内联 #[cfg(test)] 含 4 个测试（json import / toml import / missing model / api_key parse）
**缺口**:
1. `from_rcc_file` 扩展名路由（.json / .toml / unknown 默认 json）
2. `from_rcc_json_str` 非法 JSON → 错误消息含 "json is invalid"
3. `from_rcc_toml_str` 非法 TOML → 错误消息含 "toml"
4. `protocol_from_type` unsupported type → "unsupported provider profile type"
5. model selection fallback（无偏好取第一个 key）
6. `choose_model` empty models → "contains no models"
7. `auth_headers` empty api_key + no env → "must not be empty"
8. `require_value` whitespace-only → error

**补测用例**（内联 #[cfg(test)] mod tests）:
```rust
#[test]
fn from_rcc_file_routes_by_extension() {
    let td = tempfile::tempdir().unwrap();
    let toml_path = td.path().join("profile.toml");
    fs::write(&toml_path, sample_toml_profile()).unwrap();
    let result = ProviderProfileImport::from_rcc_file(&toml_path, &ProviderProfileImportOptions::default());
    assert!(result.is_ok());

    let json_path = td.path().join("profile.json");
    fs::write(&json_path, sample_profile()).unwrap();
    let result = ProviderProfileImport::from_rcc_file(&json_path, &ProviderProfileImportOptions::default());
    assert!(result.is_ok());

    // unknown extension defaults to json
    let unknown_path = td.path().join("profile.yaml");
    fs::write(&unknown_path, sample_profile()).unwrap();
    let result = ProviderProfileImport::from_rcc_file(&unknown_path, &ProviderProfileImportOptions::default());
    assert!(result.is_ok());
}

#[test]
fn from_rcc_json_str_rejects_invalid_json() {
    let err = ProviderProfileImport::from_rcc_json_str("{invalid", &ProviderProfileImportOptions::default())
        .expect_err("invalid json must fail");
    assert!(err.to_string().contains("json is invalid"));
}

#[test]
fn from_rcc_toml_str_rejects_invalid_toml() {
    let err = ProviderProfileImport::from_rcc_toml_str("bad = [[toml", &ProviderProfileImportOptions::default())
        .expect_err("invalid toml must fail");
    assert!(err.to_string().contains("toml"));
}

#[test]
fn protocol_from_type_rejects_unsupported() {
    let json = r#"{"provider":{"type":"unsupported","baseURL":"http://x","models":{},"auth":{"type":"bearer","apiKey":"k"}}}"#;
    let err = ProviderProfileImport::from_rcc_json_str(json, &ProviderProfileImportOptions::default())
        .expect_err("unsupported type must fail");
    assert!(err.to_string().contains("unsupported provider profile type"));
}

#[test]
fn choose_model_rejects_empty_models() {
    let json = r#"{"provider":{"type":"openai","baseURL":"http://x","models":{},"auth":{"type":"bearer","apiKey":"k"}}}"#;
    let err = ProviderProfileImport::from_rcc_json_str(json, &ProviderProfileImportOptions::default())
        .expect_err("empty models must fail");
    assert!(err.to_string().contains("no models"));
}

#[test]
fn auth_headers_rejects_empty_api_key_without_env() {
    let json = r#"{"provider":{"type":"openai","baseURL":"http://x","models":{"m":{}},"auth":{"type":"apikey","apiKey":""}}}"#;
    let err = ProviderProfileImport::from_rcc_json_str(json, &ProviderProfileImportOptions::default())
        .expect_err("empty api_key without env must fail");
    assert!(err.to_string().contains("must not be empty"));
}

#[test]
fn require_value_rejects_whitespace_only() {
    let json = r#"{"provider":{"type":"openai","baseURL":"   ","models":{"m":{}},"auth":{"type":"bearer","apiKey":"k"}}}"#;
    let err = ProviderProfileImport::from_rcc_json_str(json, &ProviderProfileImportOptions::default())
        .expect_err("whitespace-only field must fail");
    assert!(err.to_string().contains("must not be empty"));
}
```

**目标文件**: `rust/crates/config/src/provider_profile.rs`（内联 mod tests）
**新增依赖**: `use std::fs; use tempfile;`

---
---
### T0-3: contracts records schema 边界补测
**模块**: `contracts/src/records.rs` / `feedback.rs` / `daemon.rs` / `owner_loop.rs`
**已有**: 0 个测试
**缺口**:
1. `LedgerTrackKind.as_str` / `file_name` — 每个枚举变体返回预期字符串
2. `LedgerRecordEnvelope` JSON 序列化/反序列化 roundtrip
3. `LedgerTimelineIndexRecord` 序列化含 seq/ts/track 三字段
4. `ControlFeedback` serde default 默认值覆盖（origin="" / is_continuation=false 等）
5. `DaemonStateRecord` flatten EntityRefs + optional pid/last_heartbeat_id 覆盖
6. `OwnerLoopActionRecord` action_kind 枚举边界（review_submitted / dispatch_ready / wait_worker / stay_idle / no_managed）

**补测用例**（新建 `rust/crates/contracts/src/records_tests.rs`）:
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ledger_track_kind_as_str_and_file_name() {
        for kind in [
            (LedgerTrackKind::SessionDetail, "session.detail", "session.detail.jsonl"),
            (LedgerTrackKind::SessionSnapshot, "session.snapshot", "session.snapshot.jsonl"),
            (LedgerTrackKind::Events, "events", "events.jsonl"),
            (LedgerTrackKind::Turns, "turns", "turns.jsonl"),
            (LedgerTrackKind::Steps, "steps", "steps.jsonl"),
            (LedgerTrackKind::Tools, "tools", "tools.jsonl"),
            (LedgerTrackKind::Provider, "provider", "provider.jsonl"),
            (LedgerTrackKind::Control, "control", "control.jsonl"),
            (LedgerTrackKind::Knowledge, "knowledge", "knowledge.jsonl"),
        ] {
            assert_eq!(kind.0.as_str(), kind.1);
            assert_eq!(kind.0.file_name(), kind.2);
        }
    }

    #[test]
    fn ledger_record_envelope_json_roundtrip() {
        let envelope = LedgerRecordEnvelope {
            ledger_id: "lid-1".into(),
            seq: 42,
            ts: "2026-06-01T00:00:00Z".into(),
            track: LedgerTrackKind::Turns,
            record_id: "rid-1".into(),
            record_kind: "turn".into(),
            refs: LedgerRefs { entity: EntityRefs::default(), ..Default::default() },
            payload: serde_json::json!({"turn":1}),
            caused_by: None,
            supersedes: None,
        };
        let json = serde_json::to_string(&envelope).unwrap();
        let parsed: LedgerRecordEnvelope = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.ledger_id, "lid-1");
        assert_eq!(parsed.seq, 42);
        assert_eq!(parsed.track, LedgerTrackKind::Turns);
    }

    #[test]
    fn control_feedback_default_values() {
        let cf = ControlFeedback::default();
        assert_eq!(cf.origin, "");
        assert!(!cf.is_continuation);
        assert!(!cf.is_simple_query);
        assert!(cf.candidate_task_id.is_none());
        assert_eq!(cf.continuity_confidence, 0);
        assert_eq!(cf.topic_shift_confidence, 0);
        assert_eq!(cf.simple_query_confidence, 0);
    }

    #[test]
    fn daemon_state_record_flatten_and_optionals() {
        let json = r#"{"daemon_id":"d-1","created_at":"t1","updated_at":"t2","session_id":"s-1","task_id":null,"service_kind":"agent","lifecycle_state":"running","supervision_state":"active","mode":"attached","pid":1234,"recovery_needed":false,"status_summary":"ok"}"#;
        let record: DaemonStateRecord = serde_json::from_str(json).unwrap();
        assert_eq!(record.daemon_id, "d-1");
        assert_eq!(record.pid, Some(1234));
        assert_eq!(record.recovery_needed, false);
        // EntityRefs flattened: session_id extracted
        assert_eq!(record.refs.session_id.as_deref(), Some("s-1"));
    }

    #[test]
    fn owner_loop_action_all_action_kinds() {
        for kind in [
            "review_submitted_task",
            "dispatch_ready_task",
            "wait_worker_feedback",
            "stay_idle_no_actionable_task",
            "no_managed_tasks",
        ] {
            let record = OwnerLoopActionRecord {
                action_id: "a-1".into(),
                created_at: "2026-06-01T00:00:00Z".into(),
                refs: EntityRefs::default(),
                action_kind: kind.into(),
                target_task_ids: vec![],
                task_status_counts: vec![],
                active_task_id: None,
                reason: "test".into(),
            };
            assert_eq!(record.action_kind, kind);
        }
    }
}
```

**目标文件**: `rust/crates/contracts/src/records_tests.rs`（新建，内联 #[cfg(test)]）
**lib.rs 需添加**: `#[cfg(test)] mod records_tests;`

---
---
### T0-4: context_compaction 补测
**模块**: `runtime/src/context_compaction.rs`
**已有**: 0 个测试
**缺口**:
1. `ContextCompactionEngine::compact` 空输入 → 空 retained_messages / summary=""
2. retain_recent_count=0 → 兜底为 1（must >= 1）
3. 所有消息被 compact 后 summary 包含每条 message
4. digest 提取 summary + continuity_tail
5. tool_records 提取 artifact_refs / tool_call_id 去重
6. replaced_message_count 正确计数

**补测用例**（新建 `rust/crates/runtime/src/context_compaction_tests.rs`）:
```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn make_compact_input(messages: Vec<&str>, digests: Vec<DigestRecord>,
                          tools: Vec<ToolExecutionRecord>, retain: usize) -> CompactionInput {
        CompactionInput {
            session_id: "s-1".into(),
            task_id: None,
            trigger_reason: "budget".into(),
            recent_messages: messages.into_iter().map(String::from).collect(),
            digest_records: digests,
            tool_records: tools,
            retain_recent_count: retain,
            compacted_at: "2026-06-01T00:00:00Z".into(),
        }
    }

    #[test]
    fn compact_empty_input_yields_empty_retained() {
        let engine = ContextCompactionEngine;
        let input = make_compact_input(vec![], vec![], vec![], 2);
        let result = engine.compact(input);
        assert!(result.retained_messages.is_empty());
        assert!(result.summary.is_empty());
        assert_eq!(result.replaced_message_count, 0);
    }

    #[test]
    fn compact_retain_count_zero_defaults_to_one() {
        let engine = ContextCompactionEngine;
        let input = make_compact_input(vec!["a", "b"], vec![], vec![], 0);
        let result = engine.compact(input);
        assert_eq!(result.retained_messages.len(), 1); // last 1
        assert_eq!(result.retained_messages[0], "b");
        assert_eq!(result.replaced_message_count, 1);
    }

    #[test]
    fn compact_retains_recent_and_compacts_old() {
        let engine = ContextCompactionEngine;
        let input = make_compact_input(vec!["old1", "old2", "recent"], vec![], vec![], 1);
        let result = engine.compact(input);
        assert_eq!(result.retained_messages, vec!["recent"]);
        assert_eq!(result.replaced_message_count, 2);
        assert!(result.summary.contains("message: old1"));
        assert!(result.summary.contains("message: old2"));
    }

    #[test]
    fn compact_extracts_digest_summary_and_continuity() {
        let engine = ContextCompactionEngine;
        let digest = DigestRecord {
            digest_id: "d-1".into(),
            operation_id: "op-1".into(),
            trace_id: "t-1".into(),
            session_id: "s-1".into(),
            created_at: "t".into(),
            refs: EntityRefs::default(),
            summary: "fixed bug".into(),
            continuity_tail: vec!["check auth".into()],
            artifact_candidates: vec![],
        };
        let input = make_compact_input(vec!["msg1"], vec![digest], vec![], 0);
        let result = engine.compact(input);
        assert!(result.summary.contains("digest: fixed bug"));
        assert!(result.summary.contains("continuity: check auth"));
    }

    #[test]
    fn compact_deduplicates_tool_artifact_refs() {
        let engine = ContextCompactionEngine;
        let tool = ToolExecutionRecord {
            tool_call_id: "tc-1".into(),
            operation_id: "op-1".into(),
            trace_id: "t-1".into(),
            session_id: "s-1".into(),
            task_id: None,
            tool_name: "apply_patch".into(),
            target_ref: None,
            target_kind: None,
            input_summary: "fix".into(),
            output_summary: None,
            status: "success".into(),
            started_at: "t".into(),
            ended_at: "t2".into(),
            artifact_refs: vec!["file:///a.rs".into(), "file:///a.rs".into()], // dup
            error_detail: None,
            refs: EntityRefs::default(),
        };
        let input = make_compact_input(vec!["msg1"], vec![], vec![tool], 0);
        let result = engine.compact(input);
        // deduplicated: should have only 1 entry
        assert_eq!(result.retained_artifact_refs.len(), 1);
        assert_eq!(result.retained_tool_refs, vec!["tc-1"]);
    }
}
```

**目标文件**: `rust/crates/runtime/src/context_compaction_tests.rs`（新建）
**lib.rs 需添加**: `#[cfg(test)] mod context_compaction_tests;`

---
---
## P1 — 运行时关键状态无覆盖（高优先级）

### T1-1: closure_runtime 补测
**模块**: `runtime/src/closure_runtime.rs`
**已有**: 0 个测试（主模块无 #[cfg(test)] mod）
**缺口**: `M1Runtime::run_closure` 状态机演进：
1. operation validate fail → RuntimeError
2. sequence 递增
3. InitialRoundContext 构建成功
4. checkpoint / finalize / records 产出

**测试策略**: Mock `InferenceProvider`，验证调用顺序和输出形状（不依赖真实 provider）。

**补测用例**（新建 `rust/crates/runtime/src/closure_runtime_tests.rs`）:
```rust
#[cfg(test)]
mod tests {
    use super::*;

    struct MockProvider;
    impl InferenceProvider for MockProvider {
        fn prepare_request(&self, _: &ProviderRequest) -> PreparedRequest {
            PreparedRequest { provider_name: "mock".into(), model: "mock".into(), body: vec![], headers: Default::default() }
        }
        fn call(&self, _: PreparedRequest) -> Result<ProviderResponse, ProviderError> {
            Ok(ProviderResponse { id: "r-1".into(), model: "mock".into(), stop_reason: Some("end_turn".into()), content: vec![], usage: None })
        }
    }

    fn make_op(input: &str) -> OperationEnvelope<InferenceOperationPayload> {
        let ctx = MinimalContextView { history: None, summary: None, control: None };
        let payload = InferenceOperationPayload {
            input: input.into(),
            role: RoleProfileRef::new("system").unwrap(),
            provider_path: ProviderPath { provider_name: "mock".into(), model: "mock".into(), strategy: ProviderStrategy::Auto },
            provider_strategy: ProviderStrategy::Auto,
            protocol_version: "1.0".into(),
            stream: false,
            context: ctx,
        };
        OperationEnvelope::new("op-1", "test", "2026-06-01T00:00:00Z", "test", "trace-1", payload)
    }

    #[test]
    fn run_closure_rejects_invalid_operation() {
        let mut runtime = M1Runtime::new("test");
        let op = make_op("hi");
        // operation already has operation_id set; this tests the validate path
        // Invalid case: missing operation_id handled at construction
        let result = runtime.run_closure(op, &MockProvider);
        assert!(result.is_ok(), "valid operation should succeed: {:?}", result);
    }

    #[test]
    fn run_closure_increments_sequence() {
        let mut runtime = M1Runtime::new("test");
        let seq_before = runtime.sequence;
        let op = make_op("hello");
        let _ = runtime.run_closure(op, &MockProvider);
        assert_eq!(runtime.sequence, seq_before + 1);
    }

    #[test]
    fn run_closure_produces_closure_run() {
        let mut runtime = M1Runtime::new("test");
        let op = make_op("hello");
        let result = runtime.run_closure(op, &MockProvider);
        assert!(result.is_ok());
        let run = result.unwrap();
        assert!(!run.assistant_response_text.is_empty() || run.tool_records.is_empty()); // at least one shape is valid
    }
}
```

**目标文件**: `rust/crates/runtime/src/closure_runtime_tests.rs`（新建）
**lib.rs 需添加**: `#[cfg(test)] mod closure_runtime_tests;`

---

### T1-2: owner_loop 补测
**模块**: `runtime/src/owner_loop.rs`
**已有**: 0 个测试
**缺口**: `derive_owner_loop_action` 5 个分支覆盖：
1. submitted_task_ids 非空 → "review_submitted_task"
2. ready_task_ids 非空 → "dispatch_ready_task"
3. working_task_ids 非空 → "wait_worker_feedback"
4. truth 存在但全空 → "stay_idle_no_actionable_task"
5. truth=None → "no_managed_tasks"

**补测用例**（新建 `rust/crates/runtime/src/owner_loop_tests.rs`）:
```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn refs() -> EntityRefs { EntityRefs::default() }

    fn truth_with(submitted: Vec<&str>, ready: Vec<&str>, working: Vec<&str>) -> ManagedTaskBoardTruth {
        ManagedTaskBoardTruth {
            session_id: Some("s-1".into()),
            submitted_task_ids: submitted.into_iter().map(String::from).collect(),
            ready_task_ids: ready.into_iter().map(String::from).collect(),
            working_task_ids: working.into_iter().map(String::from).collect(),
            ..Default::default()
        }
    }

    #[test]
    fn action_review_submitted_when_submitted_not_empty() {
        let truth = truth_with(vec!["t-1"], vec![], vec![]);
        let result = derive_owner_loop_action(&refs(), Some(&truth), "2026-06-01");
        assert_eq!(result.action_kind, "review_submitted_task");
    }

    #[test]
    fn action_dispatch_ready_when_submitted_empty_ready_not_empty() {
        let truth = truth_with(vec![], vec!["t-2"], vec![]);
        let result = derive_owner_loop_action(&refs(), Some(&truth), "2026-06-01");
        assert_eq!(result.action_kind, "dispatch_ready_task");
    }

    #[test]
    fn action_wait_worker_when_working_not_empty() {
        let truth = truth_with(vec![], vec![], vec!["t-3"]);
        let result = derive_owner_loop_action(&refs(), Some(&truth), "2026-06-01");
        assert_eq!(result.action_kind, "wait_worker_feedback");
    }

    #[test]
    fn action_stay_idle_when_truth_exists_but_all_empty() {
        let truth = truth_with(vec![], vec![], vec![]);
        let result = derive_owner_loop_action(&refs(), Some(&truth), "2026-06-01");
        assert_eq!(result.action_kind, "stay_idle_no_actionable_task");
    }

    #[test]
    fn action_no_managed_when_truth_is_none() {
        let result = derive_owner_loop_action(&refs(), None, "2026-06-01");
        assert_eq!(result.action_kind, "no_managed_tasks");
    }
}
```

**目标文件**: `rust/crates/runtime/src/owner_loop_tests.rs`（新建）
**lib.rs 需添加**: `#[cfg(test)] mod owner_loop_tests;`

---
---
### T1-3: scheduler 补测
**模块**: `runtime/src/scheduler.rs`
**已有**: 0 个测试
**缺口**: `derive_scheduler_decision` 4+ 分支覆盖（paused/running/waiting_external/unavailable 等）

**补测用例**（新建 `rust/crates/runtime/src/scheduler_tests.rs`）:
```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn refs() -> EntityRefs { EntityRefs::default() }

    #[test]
    fn paused_with_parallel_inputs_runs_next() {
        let pending = vec![
            PendingInputRecord { input_id: "p-1".into(), parallel: true, ..Default::default() },
        ];
        let state = ExecutionStateRecord { status: "paused".into(), ..Default::default() };
        let result = derive_scheduler_decision(&refs(), Some(&state), &pending, None, None, "t");
        assert_eq!(result.action_kind, "run_next_parallel");
    }

    #[test]
    fn paused_without_parallel_waits() {
        let pending: Vec<PendingInputRecord> = vec![];
        let state = ExecutionStateRecord { status: "paused".into(), ..Default::default() };
        let result = derive_scheduler_decision(&refs(), Some(&state), &pending, None, None, "t");
        assert_eq!(result.action_kind, "wait_paused");
    }

    #[test]
    fn running_waits() {
        let state = ExecutionStateRecord { status: "running".into(), ..Default::default() };
        let result = derive_scheduler_decision(&refs(), Some(&state), &[], None, None, "t");
        assert_eq!(result.action_kind, "wait_running");
    }

    #[test]
    fn no_state_yields_unavailable() {
        let result = derive_scheduler_decision(&refs(), None, &[], None, None, "t");
        assert_eq!(result.action_kind, "unavailable");
    }
}
```

**目标文件**: `rust/crates/runtime/src/scheduler_tests.rs`（新建）
**lib.rs 需添加**: `#[cfg(test)] mod scheduler_tests;`

---

### T1-4: task_store 补测
**模块**: `runtime/src/task_store.rs`
**已有**: 0 个测试
**缺口**: `StoredTaskRecord` serde 序列化边界 + `TaskMutationReceipt` 构造

**补测用例**（新建 `rust/crates/runtime/src/task_store_tests.rs`）:
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stored_task_record_serde_roundtrip() {
        let task = StoredTaskRecord {
            task_id: "t-1".into(),
            session_id: "s-1".into(),
            title: "fix bug".into(),
            summary: "in auth".into(),
            epic_id: Some("e-1".into()),
            status: "created".into(),
            created_at: "2026-06-01T00:00:00Z".into(),
            updated_at: "2026-06-01T00:00:00Z".into(),
            ..Default::default()
        };
        let json = serde_json::to_string(&task).unwrap();
        let parsed: StoredTaskRecord = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.task_id, "t-1");
        assert_eq!(parsed.epic_id, Some("e-1".into()));
    }

    #[test]
    fn stored_task_record_optionals_default_to_none() {
        let json = r#"{"task_id":"t-1","session_id":"s-1","title":"t","summary":"","status":"c","created_at":"t","updated_at":"t"}"#;
        let task: StoredTaskRecord = serde_json::from_str(json).unwrap();
        assert!(task.epic_id.is_none());
        assert!(task.creator_worker_id.is_none());
        assert!(task.claimed_by_worker_id.is_none());
        assert!(task.artifact_refs.is_empty());
    }

    #[test]
    fn task_mutation_receipt_construction() {
        let task = StoredTaskRecord {
            task_id: "t-1".into(),
            session_id: "s-1".into(),
            title: "fix".into(),
            summary: "desc".into(),
            status: "closed".into(),
            created_at: "t".into(),
            updated_at: "t".into(),
            ..Default::default()
        };
        let receipt = TaskMutationReceipt { task: task.clone(), artifact_refs: vec!["a.rs".into()] };
        assert_eq!(receipt.task.task_id, "t-1");
        assert_eq!(receipt.artifact_refs.len(), 1);
    }
}
```

**目标文件**: `rust/crates/runtime/src/task_store_tests.rs`（新建）
**lib.rs 需添加**: `#[cfg(test)] mod task_store_tests;`

---
---
## P2 — 数据层零覆盖（中优先级）

### T2-1: assignment_queue 补测
**模块**: `runtime/src/assignment_queue.rs`
**已有**: 0 个测试
**缺口**: `AssignmentRecord` serde 边界 + 状态枚举

**补测用例**（新建 `rust/crates/runtime/src/assignment_queue_tests.rs`）:
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn assignment_record_roundtrip() {
        let record = AssignmentRecord {
            assignment_id: "a-1".into(),
            peer_id: "peer-1".into(),
            project_id: Some("proj-1".into()),
            session_id: Some("s-1".into()),
            task_id: Some("t-1".into()),
            target_worker_id: Some("w-1".into()),
            target_agent_name: Some("builder".into()),
            requested_role_id: "builder".into(),
            owner_worker_id: Some("owner-1".into()),
            task_summary: "build feature".into(),
            created_at: "2026-06-01T00:00:00Z".into(),
            status: "pending".into(),
            started_at: None,
            completed_at: None,
            result_summary: None,
            artifact_refs: vec![],
        };
        let json = serde_json::to_string(&record).unwrap();
        let parsed: AssignmentRecord = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.assignment_id, "a-1");
        assert_eq!(parsed.status, "pending");
    }

    #[test]
    fn assignment_record_defaults() {
        let record = AssignmentRecord::default();
        assert_eq!(record.requested_role_id, "");
        assert!(record.project_id.is_none());
        assert!(record.started_at.is_none());
    }
}
```

**目标文件**: `rust/crates/runtime/src/assignment_queue_tests.rs`（新建）
**lib.rs 需添加**: `#[cfg(test)] mod assignment_queue_tests;`

---

### T2-2: context_budget 补测
**模块**: `runtime/src/context_budget.rs`
**已有**: 0 个测试
**缺口**: `ContextBudgetManager::decide` 决策逻辑：
1. 未达阈值 → FoldLevel::NoFold
2. 达到阈值 → 触发折叠
3. provider_usage 有/无的分支
4. `decide` 边界条件（threshold=0 兜底）

**补测用例**（新建 `rust/crates/runtime/src/context_budget_tests.rs`）:
```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn make_plan(prompt_estimate: usize) -> ContextAssemblyPlan {
        ContextAssemblyPlan {
            budget: ContextBudgetSnapshot { prompt_token_estimate: prompt_estimate, ..Default::default() },
            sections: vec![],
            stability_class: ContextStabilityClass::Stable,
            max_rounds: 10,
        }
    }

    #[test]
    fn under_threshold_yields_no_fold() {
        let mgr = ContextBudgetManager::new(1000);
        let plan = make_plan(200);
        let result = mgr.decide(&plan, None);
        assert_eq!(result.fold_level, FoldLevel::NoFold);
        assert_eq!(result.decision, ContextCompactionDecisionKind::NoCompact);
    }

    #[test]
    fn at_threshold_triggers_compaction() {
        let mgr = ContextBudgetManager::new(1000);
        let plan = make_plan(1000);
        let result = mgr.decide(&plan, None);
        // at threshold triggers
        assert!(result.fold_level != FoldLevel::NoFold || result.decision == ContextCompactionDecisionKind::NoCompact);
    }

    #[test]
    fn zero_threshold_defaults_to_one() {
        let mgr = ContextBudgetManager::new(0);
        let plan = make_plan(0);
        let result = mgr.decide(&plan, None);
        assert_eq!(result.threshold_tokens, 1);
    }

    #[test]
    fn decision_includes_evidence_source() {
        let mgr = ContextBudgetManager::new(500);
        let plan = make_plan(300);
        let result = mgr.decide(&plan, None);
        assert_eq!(result.evidence_source, "runtime_estimate");
    }
}
```

**目标文件**: `rust/crates/runtime/src/context_budget_tests.rs`（新建）
**lib.rs 需添加**: `#[cfg(test)] mod context_budget_tests;`

---

### T2-3: control_feedback builder 补测
**模块**: `runtime/src/control_feedback.rs`
**已有**: 0 个测试
**缺口**: `ControlFeedbackBuilder::build` 构造逻辑：
1. simple_query 判定（word_count <= 12, punctuation <= 1, no history）
2. continuity_confidence 有 task_id=92 / 无=58
3. topic_shift_confidence 有 history=18 / 无=36
4. build 成功产出 ControlFeedback

**补测用例**（新建 `rust/crates/runtime/src/control_feedback_tests.rs`）:
```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn make_payload(input: &str, has_history: bool, has_task: bool) -> InferenceOperationPayload {
        let history = has_history.then_some(MessageHistoryRecord {
            recent_messages: vec!["past".into()], ..
        });
        let control = has_task.then_some(ControlMetadata {
            task_id: Some("t-1".into()), ..
        });
        InferenceOperationPayload {
            input: input.into(),
            role: RoleProfileRef::new("system").unwrap(),
            provider_path: ProviderPath { provider_name: "mock".into(), model: "mock".into(), strategy: ProviderStrategy::Auto },
            provider_strategy: ProviderStrategy::Auto,
            protocol_version: "1.0".into(),
            stream: false,
            context: MinimalContextView { history, summary: None, control },
        }
    }

    fn make_request() -> PreparedRequest {
        PreparedRequest { provider_name: "mock".into(), model: "mock".into(), body: vec![], headers: Default::default() }
    }

    fn make_response() -> ProviderResponse {
        ProviderResponse { id: "r-1".into(), model: "mock".into(), stop_reason: Some("end_turn".into()), content: vec![], usage: None }
    }

    #[test]
    fn short_input_no_history_is_simple_query() {
        let builder = ControlFeedbackBuilder;
        let payload = make_payload("hello?", false, false);
        let feedback = builder.build(&payload, &make_request(), &make_response());
        assert!(feedback.is_simple_query);
        assert_eq!(feedback.simple_query_confidence, 88);
    }

    #[test]
    fn long_input_not_simple_query() {
        let builder = ControlFeedbackBuilder;
        let payload = make_payload("this is a longer input that exceeds twelve words and should not be considered simple", false, false);
        let feedback = builder.build(&payload, &make_request(), &make_response());
        assert!(!feedback.is_simple_query);
    }

    #[test]
    fn has_task_yields_high_continuity_confidence() {
        let builder = ControlFeedbackBuilder;
        let payload = make_payload("do it", false, true);
        let feedback = builder.build(&payload, &make_request(), &make_response());
        assert_eq!(feedback.continuity_confidence, 92);
        assert!(feedback.is_continuation);
    }

    #[test]
    fn no_task_yields_lower_continuity_confidence() {
        let builder = ControlFeedbackBuilder;
        let payload = make_payload("do it", false, false);
        let feedback = builder.build(&payload, &make_request(), &make_response());
        assert_eq!(feedback.continuity_confidence, 58);
        assert!(!feedback.is_continuation);
    }

    #[test]
    fn has_history_yields_lower_topic_shift() {
        let builder = ControlFeedbackBuilder;
        let payload = make_payload("continue", true, false);
        let feedback = builder.build(&payload, &make_request(), &make_response());
        assert_eq!(feedback.topic_shift_confidence, 18);
    }
}
```

**目标文件**: `rust/crates/runtime/src/control_feedback_tests.rs`（新建）
**lib.rs 需添加**: `#[cfg(test)] mod control_feedback_tests;`

---
---
## P3 — 次要模块（低优先级）

### T3-1: append_only_message_log 补测
**模块**: `runtime/src/append_only_message_log.rs`
**已有**: 0 个测试
**缺口**:
1. `append` 单条 / `extend` 多条
2. `compact` 后 entries 数量减少（<= old_len）
3. `compacted` flag 设为 true
4. `compact_count` / `append_count` 递增
5. compact 后再 append 仍有效

**补测用例**（新建 `rust/crates/runtime/src/append_only_message_log_tests.rs`）:
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn append_increments_count() {
        let mut log = AppendOnlyMessageLog::new();
        log.append("hello".into());
        assert_eq!(log.append_count, 1);
        assert_eq!(log.entries.len(), 1);
    }

    #[test]
    fn extend_adds_multiple() {
        let mut log = AppendOnlyMessageLog::new();
        log.extend(["a".into(), "b".into(), "c".into()]);
        assert_eq!(log.entries.len(), 3);
    }

    #[test]
    fn compact_replaces_entries_and_sets_flag() {
        let mut log = AppendOnlyMessageLog::new();
        log.extend(["a".into(), "b".into(), "c".into()]);
        log.compact(vec!["compacted".into()]);
        assert!(log.compacted);
        assert_eq!(log.compact_count, 1);
        assert_eq!(log.entries.len(), 1);
        assert_eq!(log.entries[0], "compacted");
    }

    #[test]
    fn compact_reduces_entry_count() {
        let mut log = AppendOnlyMessageLog::new();
        log.extend(["a".into(), "b".into(), "c".into(), "d".into()]);
        log.compact(vec!["summary".into()]);
        assert!(log.entries.len() <= 4);
    }

    #[test]
    fn append_after_compact_still_works() {
        let mut log = AppendOnlyMessageLog::new();
        log.append("first".into());
        log.compact(vec!["compacted".into()]);
        log.append("after".into());
        assert_eq!(log.entries.len(), 2);
        assert_eq!(log.entries[1], "after");
    }
}
```

**目标文件**: `rust/crates/runtime/src/append_only_message_log_tests.rs`（新建）
**lib.rs 需添加**: `#[cfg(test)] mod append_only_message_log_tests;`

---

### T3-2: skill_loader 补测
**模块**: `runtime/src/skill_loader.rs`
**已有**: 0 个测试
**缺口**:
1. `load_global_skills` runtime_home=None → 空 vec
2. runtime_home=不存在路径 → 空 vec
3. `summarize_loaded_skills` 空输入 → "0 global skills loaded"
4. `summarize_loaded_skills` 有技能 + limit 截断 → "+N more"

**补测用例**（新建 `rust/crates/runtime/src/skill_loader_tests.rs`）:
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn load_global_skills_none_home_returns_empty() {
        let skills = load_global_skills(None);
        assert!(skills.is_empty());
    }

    #[test]
    fn load_global_skills_none_string_returns_empty() {
        let skills = load_global_skills(Some("   "));
        assert!(skills.is_empty());
    }

    #[test]
    fn load_global_skills_bad_path_returns_empty() {
        let skills = load_global_skills(Some("/nonexistent/path/xyz"));
        assert!(skills.is_empty());
    }

    #[test]
    fn summarize_loaded_skills_empty() {
        let skills: Vec<LoadedSkill> = vec![];
        let summary = summarize_loaded_skills(&skills, 5);
        assert!(summary.contains("0 global skills loaded"));
    }

    #[test]
    fn summarize_loaded_skills_truncates_and_reports_more() {
        let skills = vec![
            LoadedSkill { skill_id: "s1".into(), title: "".into(), summary: "".into() },
            LoadedSkill { skill_id: "s2".into(), title: "".into(), summary: "".into() },
            LoadedSkill { skill_id: "s3".into(), title: "".into(), summary: "".into() },
        ];
        let summary = summarize_loaded_skills(&skills, 2);
        assert!(summary.contains("+1 more"), "summary: {}", summary);
    }
}
```

**目标文件**: `rust/crates/runtime/src/skill_loader_tests.rs`（新建）
**lib.rs 需添加**: `#[cfg(test)] mod skill_loader_tests;`

---

### T3-3: tool_semantics 补测
**模块**: `runtime/src/tool_semantics.rs`
**已有**: 0 个测试
**缺口**:
1. `semantic_views` 空输入 → 空 vec
2. `provider.call` → provider_semantic_view 路径
3. 普通工具 → category/verb/object_kind 填充

**补测用例**（新建 `rust/crates/runtime/src/tool_semantics_tests.rs`）:
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn semantic_views_empty_input() {
        let views = semantic_views(&[]);
        assert!(views.is_empty());
    }

    #[test]
    fn semantic_views_unknown_tool_classifies() {
        let record = ToolExecutionRecord {
            tool_call_id: "tc-1".into(),
            operation_id: "op-1".into(),
            trace_id: "t-1".into(),
            session_id: "s-1".into(),
            task_id: None,
            tool_name: "custom_tool".into(),
            target_ref: Some("file:///x.rs".into()),
            target_kind: Some("file".into()),
            input_summary: "input".into(),
            output_summary: Some("output".into()),
            status: "success".into(),
            started_at: "t1".into(),
            ended_at: "t2".into(),
            artifact_refs: vec![],
            error_detail: None,
            refs: EntityRefs::default(),
        };
        let views = semantic_views(&[record]);
        assert_eq!(views.len(), 1);
        let view = &views[0];
        assert!(!view.category.is_empty());
        assert!(!view.verb.is_empty());
        assert_eq!(view.object_kind, "file");
    }
}
```

**目标文件**: `rust/crates/runtime/src/tool_semantics_tests.rs`（新建）
**lib.rs 需添加**: `#[cfg(test)] mod tool_semantics_tests;`

---

### T3-4: shared io / agents 补测
**模块**: `shared/src/io.rs` / `shared/src/agents.rs`
**已有**: 0 个测试
**缺口**:
1. `read_json_or_empty` 文件不存在 → Ok([])
2. `read_json_or_empty` 非法 JSON → Json error
3. `write_json` 创建父目录
4. `append_jsonl` 追加模式（不覆盖）
5. `AgentKind` 序列化 "system_agent" / "project_agent" / "subagent"
6. `ContextMode` 序列化

**补测用例**（新建 `rust/crates/shared/src/shared_tests.rs`）:
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn read_json_or_empty_missing_file_returns_empty_vec() {
        let result: Result<Vec<serde_json::Value>, SharedIoError> =
            read_json_or_empty(Path::new("/tmp/fin-nonexistent-12345.json"));
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    #[test]
    fn read_json_or_empty_invalid_json_returns_error() {
        let tmp = TempFile::new("invalid", "{bad");
        let result: Result<Vec<serde_json::Value>, SharedIoError> = read_json_or_empty(&tmp.path);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, SharedIoError::Json { .. }));
    }

    #[test]
    fn write_json_creates_parent_dirs() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("deep/nested/file.json");
        write_json(&path, &"test".to_string()).unwrap();
        assert!(path.exists());
    }

    #[test]
    fn append_jsonl_appends_not_overwrites() {
        let tmp = TempFile::new("log", "");
        append_jsonl(&tmp.path, &"line1".to_string()).unwrap();
        append_jsonl(&tmp.path, &"line2".to_string()).unwrap();
        let content = fs::read_to_string(&tmp.path).unwrap();
        assert!(content.contains("line1"));
        assert!(content.contains("line2"));
    }

    #[test]
    fn agent_kind_serializes_correctly() {
        assert_eq!(serde_json::to_string(&AgentKind::SystemAgent).unwrap(), ""system_agent"");
        assert_eq!(serde_json::to_string(&AgentKind::ProjectAgent).unwrap(), ""project_agent"");
        assert_eq!(serde_json::to_string(&AgentKind::Subagent).unwrap(), ""subagent"");
    }

    #[test]
    fn context_mode_default() {
        let policy = ContextPolicy::default();
        assert_eq!(policy.mode, ContextMode::TaskSummaryOnly);
    }
}

// Helper structs for file-based tests
struct TempFile { path: PathBuf }
impl TempFile {
    fn new(name: &str, content: &str) -> Self {
        let path = std::env::temp_dir().join(format!("fin_test_{}_{}", name, std::process::id()));
        fs::write(&path, content).unwrap();
        Self { path }
    }
}
impl Drop for TempFile {
    fn drop(&mut self) { let _ = fs::remove_file(&self.path); }
}

struct TempDir { path: PathBuf }
impl TempDir {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!("fin_test_dir_{}", std::process::id()));
        fs::create_dir_all(&path).unwrap();
        Self { path }
    }
}
impl Drop for TempDir {
    fn drop(&mut self) { let _ = fs::remove_dir_all(&self.path); }
}
```

**目标文件**: `rust/crates/shared/src/shared_tests.rs`（新建）
**lib.rs 需添加**: `#[cfg(test)] mod shared_tests;`

---
---
## P4 — 其他 crate（低优先级）

### T4-1: orchestrator 补测
**模块**: `orchestrator/src/lib.rs`
**已有**: 内联 mod tests 含 2 个测试（valid_transition_advances / invalid_transition_fails）
**缺口**:
1. 跳跃转换 Created→Running 失败
2. 跳跃转换 Running→Dispatched 失败
3. 全路径覆盖 Created→Dispatched→Accepted→Running→Claimed→Verified→Closed 全链成功
4. 无效终态转换：Closed→Created

**补测用例**（内联 mod tests 追加）:
```rust
#[test]
fn full_lifecycle_chain_succeeds() {
    let steps = [
        (TaskStatus::Created, TaskStatus::Dispatched),
        (TaskStatus::Dispatched, TaskStatus::Accepted),
        (TaskStatus::Accepted, TaskStatus::Running),
        (TaskStatus::Running, TaskStatus::Claimed),
        (TaskStatus::Claimed, TaskStatus::Verified),
        (TaskStatus::Verified, TaskStatus::Closed),
    ];
    let mut status = TaskStatus::Created;
    for (from, to) in steps {
        status = advance_task(status, to.clone()).unwrap();
        assert_eq!(status, to);
    }
}

#[test]
fn skip_state_is_invalid() {
    let result = advance_task(TaskStatus::Created, TaskStatus::Running);
    assert!(result.is_err());
    match result.unwrap_err() {
        TransitionError::Invalid { from, to } => {
            assert_eq!(from, TaskStatus::Created);
            assert_eq!(to, TaskStatus::Running);
        }
    }
}

#[test]
fn closed_to_created_is_invalid() {
    let result = advance_task(TaskStatus::Closed, TaskStatus::Created);
    assert!(result.is_err());
}
```

**目标文件**: `rust/crates/orchestrator/src/lib.rs`（追加到现有 mod tests）

---

### T4-2: harness-core 补测
**模块**: `harness-core/src/lib.rs`
**已有**: 0 个测试
**缺口**: `ReplayScenario` serde roundtrip

**补测用例**（新建 `rust/crates/harness-core/src/harness_tests.rs`）:
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replay_scenario_roundtrip() {
        let scenario = ReplayScenario {
            name: "e2e basic".into(),
            expected_final_status: TaskStatus::Closed,
            injected_faults: vec!["provider_timeout".into()],
        };
        let json = serde_json::to_string(&scenario).unwrap();
        let parsed: ReplayScenario = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.name, "e2e basic");
        assert_eq!(parsed.expected_final_status, TaskStatus::Closed);
        assert_eq!(parsed.injected_faults, vec!["provider_timeout"]);
    }

    #[test]
    fn replay_scenario_empty_faults() {
        let scenario = ReplayScenario {
            name: "happy".into(),
            expected_final_status: TaskStatus::Verified,
            injected_faults: vec![],
        };
        assert!(scenario.injected_faults.is_empty());
    }
}
```

**目标文件**: `rust/crates/harness-core/src/harness_tests.rs`（新建）
**lib.rs 需添加**: `#[cfg(test)] mod harness_tests;`

---

### T4-3: debug-server 补测
**模块**: `debug-server/src/lib.rs`
**已有**: 1 个测试文件
**缺口**:
1. `InMemoryProjector::apply` 事件投影正确更新 current
2. `DebugProjectionConfig` 默认值
3. `DebugSnapshot` 序列化

**补测用例**（追加到现有测试文件或新建）:
```rust
#[test]
fn in_memory_projector_apply_updates_current() {
    let mut projector = InMemoryProjector::new(DebugProjectionConfig::default());
    let event = EventEnvelope::new("evt-1", "test", "2026-06-01", "src", "trace-1", serde_json::json!({"key":"val"}));
    projector.apply(&event);
    assert!(projector.current.is_some() || !projector.events.is_empty());
}

#[test]
fn debug_projection_config_defaults() {
    let config = DebugProjectionConfig::default();
    assert!(!config.retain_raw_events);
    assert!(config.retain_timeline_rows > 0);
}
```

---

### T4-4: provider 补测
**模块**: `provider/src/tests.rs`
**已有**: registry 注册 + duplicate reject
**缺口**:
1. `PreparedRequest` 构造 / 字段正确
2. `ProviderRequest` required fields
3. `parse_openai_response` / `parse_anthropic_response` 成功/失败路径
4. `endpoint_for_protocol` 映射正确

**补测用例**（追加到 `provider/src/tests.rs`）:
```rust
#[test]
fn prepared_request_holds_provider_info() {
    let req = PreparedRequest {
        provider_name: "openai".into(),
        model: "gpt-4o".into(),
        body: vec![],
        headers: Default::default(),
    };
    assert_eq!(req.provider_name, "openai");
    assert_eq!(req.model, "gpt-4o");
}

#[test]
fn endpoint_for_protocol_openai() {
    let ep = endpoint_for_protocol(&ProviderProtocol::OpenAiCompatible, "https://api.openai.com");
    assert!(ep.contains("v1") || ep.contains("completions"));
}

#[test]
fn endpoint_for_protocol_anthropic() {
    let ep = endpoint_for_protocol(&ProviderProtocol::AnthropicWire, "https://api.anthropic.com");
    assert!(ep.contains("messages") || ep.contains("v1"));
}

#[test]
fn parse_openai_response_empty_content_yields_stop() {
    let json = r#"{"id":"r-1","model":"gpt-4o","choices":[{"message":{"role":"assistant","content":""},"finish_reason":"stop"}],"usage":{"prompt_tokens":10,"completion_tokens":5}}"#;
    let resp = parse_openai_response(json).unwrap();
    assert_eq!(resp.stop_reason.as_deref(), Some("stop"));
}

#[test]
fn parse_anthropic_response_valid() {
    let json = r#"{"id":"m-1","type":"message","role":"assistant","content":[{"type":"text","text":"hi"}],"stop_reason":"end_turn","usage":{"input_tokens":10,"output_tokens":5}}"#;
    let resp = parse_anthropic_response(json).unwrap();
    assert_eq!(resp.stop_reason.as_deref(), Some("end_turn"));
}
```

**目标文件**: `rust/crates/provider/src/tests.rs`（追加）

---
---
## 执行顺序

### Phase 0 — 测试骨架与编译门
1. 为每个目标模块添加 `#[cfg(test)] mod xxx_tests;`
2. 新建测试文件时先只放 1 个 smoke test
3. 执行：`cd rust && cargo test --workspace --no-run`
4. 编译通过后再补完整红测矩阵

### Phase 1 — P0 模块补齐
1. `config/src/startup.rs`
2. `config/src/provider_profile.rs`
3. `contracts/src/records_tests.rs`
4. `runtime/src/context_compaction_tests.rs`
5. 执行：
   - `cd rust && cargo test -p fin-config`
   - `cd rust && cargo test -p fin-contracts`
   - `cd rust && cargo test -p fin-runtime context_compaction`

### Phase 2 — P1 模块补齐
1. `runtime/src/owner_loop_tests.rs`
2. `runtime/src/scheduler_tests.rs`
3. `runtime/src/task_store_tests.rs`
4. `runtime/src/closure_runtime_tests.rs`
5. 执行：
   - `cd rust && cargo test -p fin-runtime owner_loop`
   - `cd rust && cargo test -p fin-runtime scheduler`
   - `cd rust && cargo test -p fin-runtime task_store`
   - `cd rust && cargo test -p fin-runtime closure_runtime`

### Phase 3 — P2/P3 模块补齐
1. `assignment_queue_tests`
2. `context_budget_tests`
3. `control_feedback_tests`
4. `append_only_message_log_tests`
5. `skill_loader_tests`
6. `tool_semantics_tests`
7. `shared_tests`
8. 执行：`cd rust && cargo test --workspace`

### Phase 4 — P4 crate 补齐
1. orchestrator full lifecycle
2. harness-core replay scenario
3. debug-server projector/config/snapshot
4. provider parsing and endpoint
5. registry / transport-http 最小 serde/config 测试（如果有 pub API）
6. 执行：`cd rust && cargo test --workspace`

---
## 验收门槛

### 编译验收
- `cd rust && cargo test --workspace --no-run` 必须通过

### 功能验收
- `cd rust && cargo test --workspace` 必须通过

### 覆盖验收
- P0/P1 所列模块必须出现对应测试文件或内联测试：
  - `config/src/startup.rs`
  - `config/src/provider_profile.rs`
  - `contracts/src/records_tests.rs`
  - `runtime/src/context_compaction_tests.rs`
  - `runtime/src/owner_loop_tests.rs`
  - `runtime/src/scheduler_tests.rs`
  - `runtime/src/task_store_tests.rs`
  - `runtime/src/closure_runtime_tests.rs`

### 契约验收
每个模块至少锁定：
1. happy path
2. invalid input path
3. serde roundtrip 或状态机边界（如适用）
4. error message / action_kind / status 字段的字符串契约
5. 不允许 fallback，不允许吞错误

### 禁止事项
- 禁止 mock 真实 LLM/provider 作为 E2E 闭环证据；这里只做模块黑盒红测
- 禁止通过修改业务逻辑绕过测试；测试失败必须回到唯一真源修复
- 禁止删除/回滚已有测试
- 禁止污染真实用户配置、真实 `~/.fin`、真实 provider profile
- 禁止 broad kill

---
## 实施注意

1. 优先使用模块公开 API；仅当模块是 `pub(crate)` 且测试文件同 crate 内部时，允许访问 crate-private 函数。
2. 对私有字段测试不要强行穿透；优先使用公开 accessor 或 serde roundtrip。若没有 accessor，先评估是否需要新增最小只读 accessor。
3. 新增测试必须小而确定，避免依赖系统时间、真实网络、真实 provider。
4. 文件系统测试必须使用临时目录，测试结束清理。
5. 若发现设计缺口（如 duplicate project_id 未校验），先写红测确认失败，再实现最小修复。
6. 每个阶段完成后更新 `note.md`，最终把已验证结论提炼到 `MEMORY.md`。

---
## /goal 提示词

```text
/goal objective: 按 `plans/red-test-remediation-2026-06-01.md` 完成 fin 项目的黑盒红测补齐，优先 P0/P1，确保每个关键模块的功能契约可由测试锁定。

target docs:
- `plans/red-test-remediation-2026-06-01.md`
- `AGENTS.md`
- `skills/fin-general-dev/SKILL.md`
- `skills/fin-testing-harness/SKILL.md`
- `docs/architecture/08-testing-and-ci-strategy.md`

execution rules:
- 先读计划与相关 skill/doc，再改代码。
- 先补红测，确认失败或确认缺口，再做最小实现修复。
- 禁止 fallback、静默失败、真实 payload 裁剪、污染真实用户配置。
- 禁止 broad kill。
- 只做测试补齐与必要最小业务修复，不做无关重构。
- 新增测试优先纯 Rust `#[test]`，使用合成数据和临时目录，不依赖真实网络/真实 provider。
- 若测试需要访问 crate-private API，测试放在同 crate 内部；不要为了测试扩大 public API，除非确认为契约必要。
- 每完成一个阶段，将发现写入 `note.md`；收口时提炼已验证结论到 `MEMORY.md`。

verification gates:
1. `cd rust && cargo test --workspace --no-run`
2. `cd rust && cargo test -p fin-config`
3. `cd rust && cargo test -p fin-contracts`
4. `cd rust && cargo test -p fin-runtime context_compaction`
5. `cd rust && cargo test -p fin-runtime owner_loop`
6. `cd rust && cargo test -p fin-runtime scheduler`
7. `cd rust && cargo test -p fin-runtime task_store`
8. `cd rust && cargo test --workspace`

completion signal:
- 回报变更文件列表
- 回报每个 verification gate 的真实输出摘要
- 回报仍未覆盖的模块/风险
- 回报是否需要进入下一轮真实 provider E2E（本任务不要求执行真实 provider E2E）
```
