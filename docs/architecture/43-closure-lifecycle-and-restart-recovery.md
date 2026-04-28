# 43 Closure Lifecycle And Restart Recovery

本文档冻结 fin 的推理 closure 生命周期、后续工作触发、以及 daemon 重启恢复的正确流程。

目标：

1. 定义 Session / Ledger / Session View 三层模型
2. 证明 execution checkpoint 机制是设计错误
3. 定义正确的 passive-trigger 恢复流程
4. 作为后续回归测试的基准文档

---

## 1. 核心结论

推理是 **live** 的。一次 closure 从 run_closure() 开始到返回 ClosureRun 结束，整个过程在内存中完成，不跨进程、不跨 daemon 生命周期。

closure 是一次性的：一旦 run_closure() 返回，这个推理 session 就结束了。不可能在 daemon 重启后恢复一个已经结束的 closure。

因此：

- **execution checkpoint 是设计错误**：它试图在 closure 结束后记录从哪一步恢复，但重启后旧推理 session 已死，runtime target 已不存在。
- **正确做法**：closure 结束时如果还有后续工作，framework enqueue 一个 pending_input，走标准的 pending -> scheduler -> 新推理 session 路径。
- wait.remind 已经证明了这个模式的正确性：提醒写入 reminders/pending.json，到期后 inject 为 pending_input，新推理 session 处理。

---

## 2. Session / Ledger / Session View 三层模型

session 数据分三层，数据流向一致，不存在错位。

| 层 | 描述 | 写入时机 |
|---|---|---|
| Memory State（内存态） | closure 过程中的活跃数据，存在于 run_closure() 调用栈内 | 每轮 round 追加 |
| Ledger（流水账） | append-only，只追加不覆盖 | 每轮 round flush |
| Session View（动态视图） | 从 ledger 生成 | closure 结束后 rebuild |

### 2.1 Memory State（内存态）

closure 过程中的活跃数据，存在于 run_closure() 的调用栈内：

- tool_records: Vec - 每轮 round 追加
- round_records: Vec - 每轮 round 追加
- step_records: Vec - 每轮 round 追加
- provider_request_records: Vec - 每轮 round 追加
- provider_response_records: Vec - 每轮 round 追加
- assistant_response_text: String - 每轮 round 更新

数据来源：provider HTTP 响应 + tool 执行结果。和 ledger 数据来源完全一致。

### 2.2 Ledger（流水账）

append-only，只追加不覆盖。每轮 round 完成后立刻 flush 到这些文件：

| 文件 | 内容 | 写入时机 |
|------|------|---------|
| conversation/messages.json | user/assistant 交互 | closure 结束后 |
| tools/recent.json | tool 调用记录 | 每轮 round |
| rounds/recent.json | round 记录 | 每轮 round |
| steps/recent.json | step 记录 | 每轮 round |
| provider/recent_requests.json | provider 请求 | 每轮 round |
| provider/recent_responses.json | provider 响应 | 每轮 round |

作用：
1. context rebuild 的数据源
2. crash recovery 的数据源

### 2.3 Session View（动态视图）

从 ledger 生成，closure 结束后统一 rebuild：

| 文件 | 内容 | 生成方式 |
|------|------|---------|
| context/recent_contexts.json | context 快照 | closure 结束后写入 |
| digests/recent_digests.json | 历史摘要 | closure 结束后写入 |
| reasoning/recent_reasoning_views.json | 推理视图 | closure 结束后写入 |
| tools/latest.json | 最新工具调用 | closure 结束后写入 |

用途：推理前 context assembly、channel 展示。必须和 ledger ��持一致。

### 2.4 一致性规则

内存态 subset ledger subset session view

写入顺序（每轮 round）：
1. 内存态追加（Vec.push）
2. ledger 追加（file append）
3. session view 更新（file write，closure 结束后统一 rebuild）

不一致场景：crash 发生在步骤 2 之后、步骤 3 之前 -> 重启后 session view 落后于 ledger -> 解决：rebuild session view from ledger（启动时自动执行）

### 2.5 Context Rebuild 路径

新推理 session 开始时：
1. 从 ledger 读取 recent records（messages、tools、rounds）
2. build_round_context() 使用这些 records 组装 MinimalContextView
3. 推理开始

不需要 checkpoint，不需要 resume_input。ledger 是完整真相，session view 从 ledger 生成。

### 2.6 每轮 Flush 时机

run_closure() 内部每完成一个 round：

1. execute_round_with_contract_retries() 返回 RoundExecution
2. record_round() 追加到内存 Vec
3. flush_round_to_ledger() 写入 session 文件（新增）

未完成的 round 不写入 ledger。重启后 ledger 里只有已完成的 rounds。

---

## 3. Closure 生命周期（单次推理）

Daemon Supervisor Cycle：

1. inject_due_reminders() -> 到期 reminder -> enqueue pending_input
2. run_scheduler_tick() -> 发现 pending_input -> 决定 run_next
3. run_binding_turn() -> mark_running() -> run_demo_request()
   -> M1Runtime::run_closure() [同步阻塞]
      run_closure() 内部流程：
      for round 1..N:
        build_round_context() -> 组装 context（从 ledger 读取，内存态更新）
        execute_round_with_contract_retries() -> provider.execute_prepared() [HTTP 调 provider]
        -> ModelOutputParser::parse()
        -> tool_dispatch::execute_model_tools()
        record_round() -> 追加到内存 Vec
        flush_round_to_ledger() [每轮写盘] -> append to session ledger files
      return ClosureRun
4. finalize_after_run() -> 写 execution_state = idle -> 清 execution_lease
5. persist_runtime_demo() -> rebuild session view（从 ledger）
6. 返回 -> supervisor cycle 结束

---

## 4. Closure 结束的四种结果

run_closure() 结束时，control_exit_channel 和 dispatched_tools 决定了结果：

| 条件 | 结果 | 后续动作 |
|------|------|---------|
| yield_requested | wait.remind 场景 | reminder 已写入 reminders/pending.json，等 reminder 到期 |
| stop_requested + completed_with_evidence | 任务完成 | 无需后续 |
| stop_requested + simple_chat_done | 简单聊天 | 无需后续 |
| stop_requested + blocked_requires_user_action | 需要用户介入 | 无需 framework 动作 |
| tool_calls 非空 + round < max | 继续下一轮 round | 仍在同一 closure 内 |
| tool_calls 非空 + round >= max | auto_tool_round_limit_reached | 需要 enqueue pending_input |
| tool_calls 为空 + !stop | 正常结束 | 无需后续 |

---

## 5. 旧 Checkpoint 机制（错误设计）

### 5.1 为什么这是错误的

1. **推理 session 已死**：daemon 重启后，旧的 run_closure() 调用栈早已不存在
2. **resume_input 不等于原始 context**：丢失了完整的 context assembly、tool history、reasoning views
3. **与 pending_input 重复**：wait.remind 已经证明了正确路径
4. **引入复杂状态机**：resume_checkpoint_ready 等概念全是多余的
5. **daemon 被 kill 时 checkpoint 残留**：重启后会错误地尝试恢复
6. **与 ledger 模型冲突**：有了 ledger 后只需要知道还有没有后续工作

---

## 6. 正确设计：Pending Input 是唯一触发路径

closure 结束后的所有后续工作，必须通过 pending_input 触发。

| 场景 | 动作 |
|------|------|
| wait.remind | reminder 写入 reminders/pending.json，到期后 inject pending_input |
| round 耗尽 | enqueue pending_input (source=framework.closure.round_limit) |
| daemon 被 kill | 前面 rounds 已在 ledger，重启后 rebuild + enqueue pending_input |
| daemon 重启 + orphaned running | 标记 failed + enqueue pending_input (source=framework.daemon.restart_recovery) |
| 正常完成 | 无后续工作，干净结束 |

### 6.1 新增的 pending_input 来源

| 来源 | 触发条件 | 当前状态 |
|------|---------|---------|
| framework.startup.self_check | daemon 启动，有 unfinished work | 已实现 |
| system.agent.reminder:* | reminder 到期 | 已实现 |
| framework.closure.round_limit | closure round 耗尽但还有 tool calls | 需新增 |
| framework.daemon.restart_recovery | daemon 重启发现 orphaned running state | 需新增 |

---

## 7. 完整生命周期图（重启场景）

Daemon 启动：

1. reconcile_headless_daemon_state() -> 检查旧 daemon pid 是否存活
2. refresh_startup_control_plane() -> materialize startup topology
3. inject_due_reminders() -> 到期 reminder -> enqueue pending_input
4. run_headless_cycle() -> scheduler 决策：
   - running + pid 死了 -> 标记 failed + enqueue pending_input (restart_recovery)
   - idle + 有 pending_input -> run_next_pending -> 触发新推理
   - idle + 无 pending_input -> stay_idle

---

## 8. Pending Input 全生命周期

来源 -> pending_input queue -> scheduler tick -> run_binding_turn()
-> run_closure()（全新推理 session，从 ledger rebuild context）-> closure 结束 -> 可能 enqueue 新 pending_input

---

## 9. 需要删除的代码

| 文件 | 内容 | 原因 |
|------|------|------|
| closure_runtime_checkpoint.rs | build_resume_checkpoint() | checkpoint 产生点 |
| execution_checkpoint.rs | 整个文件 | checkpoint 持久化/消费 |
| closure_runtime.rs | resume_checkpoint 调用 | 调用点 |
| closure_runtime_finalize.rs | resume_checkpoint 字段传递 | 传递链 |
| lib.rs | ClosureRun.resume_checkpoint 字段 | struct 字段 |
| control_plane.rs | resume_checkpoint_ready 相关计算 | 状态计算 |
| scheduler.rs | resume_checkpoint 优先级分支 | 决策分支 |
| session_materializer.rs | persist_execution_checkpoint() | 持久化 |

### 需要修改的字段

| 结构体 | 字段 | 操作 |
|--------|------|------|
| ExecutionStateRecord | resume_checkpoint_ready | 删除 |
| ExecutionStateRecord | resume_checkpoint_id | 删除 |
| ExecutionStateRecord | resume_from_step_id | 删除 |
| ClosureRun | resume_checkpoint | 删除 |

---

## 10. 需要新增的代码

### 10.0 每轮 flush 到 ledger

新增 flush_round_to_ledger() 函数，在 record_round() 之后调用。需要传入 runtime_home 和 session_dir。

### 10.1 closure round limit -> pending_input

在 finalize_after_run() 之后，如果 round 耗尽，enqueue pending_input。

### 10.2 daemon restart recovery -> pending_input

在 orphan recovery 路径中，直接标记 failed + enqueue pending_input。

---

## 11. 回归测试矩阵

| 场景 | 验证点 |
|------|--------|
| 正�� closure 完成 | execution_state = idle，无 checkpoint 残留 |
| wait.remind -> 到期 -> 新推理 | reminder -> pending_input -> scheduler -> 新 closure |
| closure round 耗尽 | enqueue pending_input (round_limit) |
| daemon 重启 + orphaned running | 标记 failed + enqueue pending_input |
| daemon 重启 + idle | 无动作，stay_idle |
| daemon 被 kill + closure 进行中 | 前面的 rounds 已在 ledger，重启后 rebuild session view |
| ledger 和 session view 一致性 | closure 结束后 session view == ledger 生成的结果 |

---

## 12. 与其它文档的关系

- 42-event-driven-collaboration-and-trigger-model.md
- 36-daemon-supervisor-startup-contract.md
- 39-agent-presence-and-resume-model.md
- 40-attached-control-plane-cycle.md
- 24-session-render-truth-and-reasoning-input-assembly.md
