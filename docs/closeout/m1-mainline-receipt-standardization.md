# M1 Mainline Receipt Standardization

本文档把 M1.1 的焦点从“receipt 目录页”推进到“推理主链关键行为的真实证据”。

目标很克制，只做三件事：

1. 固定 mainline receipt family
2. 固定每类 receipt 的最小判定条件
3. 让 closeout / regression 能直接引用真实 session artifacts

它不做：

- 重写整套 harness
- 发明新的 runtime 语义
- 用测试想象替代真实 session 证据

---

## 1. 当前统一入口

当前 mainline receipts 固定写入：

```text
harness/reports/<build-version>/mainline-receipts.json
```

当前 schema：

```text
fin.receipt.mainline.v1
```

它是推理主链 receipt 的稳定聚合页。

---

## 2. 固定 receipt family

当前 M1.1 先冻结 3 类：

1. `history_context`
2. `auto_tool_roundtrip`
3. `control_boundary`

说明：

- `history_context`：证明多轮历史确实进入了下一轮 context，而不是只有 UI 对话在增长
- `auto_tool_roundtrip`：证明同一 operation 内发生了真正的自动工具往返，而不是只解析了一次 tool call 就停
- `control_boundary`：证明 session durable artifacts 已经记录控制面边界事实（如 heartbeat / daemon / scheduler / queue / interrupt 等），便于后续 pause/resume/queue 稳定化
  - 当前已升级为 **async control-plane receipt**，要求能看见 `wait_external -> reminder_fired -> heartbeat_due/stale_lease` 这类异步恢复链，而不只是 pause/queue/resume 的静态残留物

---

## 3. 固定字段

顶层固定字段：

- `schema_version`
- `generated_at`
- `build_version`
- `runtime_home`
- `report_dir`
- `source_session_id`
- `source_session_path`
- `receipts[]`

每个 receipt entry 固定字段：

- `receipt_id`
- `kind`
- `status`
- `source_session_id`
- `source_session_path`
- `supporting_paths[]`
- `summary`

状态仍然只允许：

- `passed`
- `failed`
- `missing`

---

## 4. 各类 receipt 的最小判定条件

## 4.1 history_context

原始真源：

- `conversation/messages.json`
- `context/recent_contexts.json`
- `steps/recent_steps.json`
- `rounds/recent_rounds.json`

最小通过条件：

1. 至少存在 2 个 context records
2. 第一条 continuity tail 为空
3. 后续至少一条 continuity tail 非空
4. 至少有 2 次 operation / round 能相互对应

这类 receipt 要回答：

- continuity 是否真正增长
- 最新 context 带进了哪些最近消息
- 这是不是 runtime 装配的事实，而不是 UI 假象

---

## 4.2 auto_tool_roundtrip

原始真源：

- `rounds/recent_rounds.json`
- `steps/recent_steps.json`
- `events/stream.jsonl`
- `tools/recent_tool_records.json`

最小通过条件：

1. 至少一个 operation 出现 `round_count > 1`
2. 或者存在 `reasoning.auto_tool_roundtrip_completed` 事件

如果当前 closeout run 没有真实样本，必须返回 `missing`，不能拿单轮 tool dispatch 冒充通过。

---

## 4.3 control_boundary

原始真源候选：

- `control/execution_state.json`
- `control/pause_checkpoint.json`
- `control/scheduler/latest.json`
- `control/scheduler/latest_tick.json`
- `control/supervisor/latest.json`
- `control/supervisor/latest_heartbeat.json`
- `control/supervisor/recent_heartbeats.json`
- `control/daemon/latest_state.json`
- `control/daemon/latest_recovery_action.json`
- `queue/pending_inputs.json`
- `interrupts/recent_segments.json`
- `interrupts/recent_merges.json`
- `runtime/current/current_execution_state.json`

最小通过条件：

1. 至少存在一组 durable control-plane artifacts
2. 摘要里必须明确哪些 control families 实际存在
3. 若 queue / interrupt 当前为空，必须如实反映为空，而不是补结论
4. stronger async receipt 至少要能证明以下事实中的关键子集已经真实发生：
   - `waiting_external`
   - `reminder_scheduled`
   - `reminder_fired`
   - `supervisor_heartbeat_due`
   - `stale_lease`

这类 receipt 当前要回答：

- 该 session 是否已经有 durable control-plane evidence
- 当前 evidence 来自哪些 families
- 是否真实发生过 pause / queue / resume / scheduler drive / segment merge
- 是否真实发生过 `wait_external -> reminder_fired -> heartbeat_due/stale_lease` 的异步控制面推进
- 当前异步控制面还缺什么

---

## 5. 当前边界与策略

当前这一步仍然保持收口策略：

1. 先把真实 receipt family 的 schema 与生成入口固定
2. 先产出有真实样本的 `history_context`
3. `auto_tool_roundtrip` 没样本时就老实标 `missing`
4. `control_boundary` 先收当前 session 已有 durable control artifacts，并持续提升到 async control-plane 级别

不要为了“看起来完整”而伪造 tool-loop 或 pause/queue 证据。

---

## 6. 当前生成方式

生成脚本固定为：

```bash
scripts/build-mainline-receipts.py \
  --runtime-home <runtime-home> \
  --report-dir <.../harness/reports/<build-version>> \
  --build-version <build-version> \
  --session-id <session-id>
```

若不同 receipt family 需要不同 source session，可额外指定：

- `--history-session-id`
- `--tool-loop-session-id`
- `--control-session-id`

未指定时，默认都回退到 `--session-id`。

输出：

```text
harness/reports/<build-version>/mainline-receipts.json
```

---

## 7. 当前最小演进方向

下一步若继续推进，顺序固定为：

1. 继续补 queue / interrupt / pending / resume + async wait/reminder/heartbeat 的强 control-boundary 样本
2. 再把 mainline receipt 纳入统一 receipt index
3. 最后再考虑更多 installed-binary / control-plane receipt family

也就是说：

> 先把推理主链的真实证据落下来，再谈更大的 harness 统一化。
