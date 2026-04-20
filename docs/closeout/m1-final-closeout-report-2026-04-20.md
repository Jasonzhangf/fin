# M1 Final Closeout Report — 2026-04-20

本文档是 `fin` 当前 M1 阶段的最终收口报告。

目标只回答四件事：

1. **M1 是否已经完成**
2. **完成的证据是什么**
3. **当前还剩什么债务**
4. **下一步应该如何进入 M2**

它不是新的架构文档，而是本轮 closeout 的最终结论页。

---

## 1. 最终结论

截至 **2026-04-20**，`fin` 的 **M1 最小闭环已经完成**。

完成判定依据：

1. 真实 provider 可用
2. 单 agent 多轮推理闭环可用
3. 同 turn 自动 tool loop 可用
4. `/compact` rebuild 可用
5. session / turn / step / event / control plane durable truth 可追索
6. Web debug / status probe / tick / reminder / queue / interrupt 可观察
7. install/build gate 已恢复
8. `qqbot` 真实 channel 对话闭环已有 **repo 内 E2E + 真实 live receipt**

因此当前状态应视为：

> `fin` 已经从 “稳定内核” 进入 “真实最小产品闭环完成” 状态；接下来不应继续在 M1 范围内扩张，而应以冻结边界和收敛 debt 为主，并谨慎进入 M2。

---

## 2. 本轮确认的 M1 完成面

### 2.1 推理主链

当前已确认：

1. 用户输入进入 session truth
2. runtime 组装 context
3. provider 发真实请求
4. 模型输出解析为：
   - assistant response
   - control feedback
   - tool calls
5. tool execution 结果进入 durable truth
6. 同一 turn 内自动 tool roundtrip 可继续推进
7. 最终 closure / digest / reasoning / provider artifacts / tool records 全部落盘

这条链已经不是 demo，而是有 receipt 与回归矩阵支撑的最小主链。

### 2.2 可观测性与 durable truth

当前已确认：

1. session `conversation/messages.json` 是会话渲染真源
2. `recent_contexts / recent_digests / recent_reasoning_views / recent_tool_records` 可重建上下文视图
3. `turns / steps / provider / routing / closures` 可重建推理历史
4. Web 只消费 artifacts，不补 runtime 真相
5. provider debug 已做脱敏

### 2.3 控制面

当前已确认：

1. `RoutingDecisionRecord`
2. `RoutingActionRecord`
3. `ExecutionStateRecord`
4. `PendingInputRecord`
5. `InterruptedSegmentRecord`
6. `SegmentMergeRecord`
7. `SchedulerDecisionRecord`
8. `SchedulerTickRecord`
9. `SupervisorCycleRecord`
10. `SupervisorHeartbeatRecord`
11. `DaemonStateRecord`
12. `DaemonRecoveryActionRecord`

这些记录已经足以支撑 M1 所需的继续/排队/提醒/巡检/观察闭环。

### 2.4 `qqbot` 真实 channel 闭环

当前已确认：

1. `message.ingest`
2. `target -> session` restore / binding
3. 输入进入 session truth
4. 走 runtime inference
5. 回复进入 session truth
6. `message.emit` 回到真实 `qqbot`
7. 异步 periodic delivery 也能从 session truth 派送

这意味着 `qqbot` 已经不是 “peer skeleton”，而是 **M1 所要求的真实 channel gateway 最小闭环**。

---

## 3. 关键证据

## 3.1 provider real smoke

真实 provider receipt：

- `~/.fin/harness/runs/test-live-provider-20260420-2155/provider-live-smoke-report.json`

关键事实：

- `provider_name = ali-coding-plan`
- `protocol = anthropic-wire`
- `model = qwen3.6-plus`
- `turn_count = 3`
- `control_feedback_origin = model_output_contract_v1`
- `reasoning_stop_present = true`

结论：

- 默认真实 provider 主链可用

## 3.2 install / build receipts

标准 receipt index：

- `~/.fin/harness/reports/0.1.0001/receipt-index.json`

已确认：

- build/install gate 已恢复
- receipt index 已稳定生成
- install smoke 已进入标准 receipt 流

## 3.3 mainline receipts

已有主链 receipts：

- `history_context`
- `auto_tool_roundtrip`
- `control_boundary`

当前 closeout 口径中，这三类都已收下，并已进入 `docs/closeout/` 文档体系。

## 3.4 qqbot repo 内 E2E

repo 内已补：

- `process_inbound_message(...)` 集成 E2E

已验证：

1. inbound -> ack
2. runtime inference
3. session truth 落盘
4. final reply emit
5. existing target restore 继续旧 session

## 3.5 qqbot real live receipt

真实 live receipt：

- `~/.fin/harness/runs/qqbot-live-receipt-20260420-real/qqbot-live-receipt.json`

本轮实际目标：

- `target = qqbot:c2c:F6A6F19355D0D62EEC06277EB445B51F`

关键字段：

- `status = passed`
- `session_id = session-test-install-0-1-0001`
- `task_id = task-test-install-0-1-0001`
- `ack_notice_present = true`
- `session_reply_present = true`
- `provider_request_present = true`
- `provider_response_present = true`
- `latest_reply_preview = OK`

结论：

- `qqbot` 真实输入输出闭环已有真实 runtime receipt，不再只是 repo 内测试结论

---

## 4. 回归与验证结果

当前已跑过并通过：

```bash
cargo test --workspace
```

```bash
cargo test -p fin-cli
```

此外还实际执行过：

```bash
cargo run -p fin-cli -- qqbot-live-receipt ~/.fin/config/user.toml qqbot:c2c:F6A6F19355D0D62EEC06277EB445B51F qqbot-live-receipt-20260420-real
```

结果：

- receipt 成功生成
- `status = passed`

---

## 5. 当前仍然存在的债务

这些不再阻塞 M1，但必须被明确记录：

### 5.1 长文件拆分债务

`scripts/check-code-line-limit.py` 仍会报若干 >500 行文件。

这属于：

- 结构性模块拆分债务
- 不是 M1 功能闭环 blocker
- 应在 M2 入口阶段或稳定化阶段逐步拆分

### 5.2 always-on / daemon 强化

当前 `qqbot` 已有最小可用闭环，但以下仍属于后续增强：

1. detached daemon 级常驻监督
2. 更强 crash-restart / lease / recovery 体系
3. 更细的 lifecycle receipts

### 5.3 真多 agent 执行面

当前 system/project/worker 的骨架已具备方向，但还没进入：

1. 真 project agent 执行链
2. 跨机器 peer 协作
3. distributed mailbox / eventbus

这些统一进入 M2。

---

## 6. M2 入口建议

当前最合理的 M2 入口不是继续加大而全的新概念，而是按以下顺序进入：

1. **先做结构性 debt 收敛**
   - 大文件拆分
   - 稳定 contract / truth ownership
2. **再做 always-on / daemon 生命周期强化**
3. **再进入 project agent / worker 执行面**
4. **最后才做 distributed peers / cross-machine**

原则：

> 先把当前 M1 已有真源守住，再把系统从“最小闭环”推进到“可长期运行的多 agent 系统”。

---

## 7. 最终判断

如果只用一句话概括当前状态：

> `fin` 的 M1 已经完成：它现在拥有真实 provider、真实 session truth、真实 channel gateway、真实 receipts，以及可回归的单 agent runtime 内核；下一步不再是补 M1，而是以冻结边界为前提，谨慎进入 M2。
