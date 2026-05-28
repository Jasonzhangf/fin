# Android 推理链路 2026-05-28 审计 closeout

## 验收结论

按当前 objective 本轮审计结论为：已具备可运行的 Android 推理链路，且 turn 过程事件可被结构化消费，已通过本地回归矩阵与真机证据采集。

## 完成状态（对照 objective 关键条款）

1. 本地 runtime/provider/tool 推理链路真实跑通  
   - 真实 provider live smoke 通过：`reports/regression/20260528-goal-audit/live-provider-smoke.log`  
   - Android turn-channel E2E 通过：`reports/android-mvp-logs/turn-channel-e2e.log`

2. turn 过程消息被结构化消费，而不只是最终文案  
   - `turn.item.started/completed/failed` 已形成配对链路  
   - E2E 检查：`item_started_present=true`、`item_terminal_present=true`、`each_item_started_has_terminal=true`

3. Android 只作为控制界面壳，不拥有业务推理语义  
   - `android-client/app/src/main/assets/mobile-shell.html` 仅消费 `turn.item.*` 与 session/runtime/health 事件  
   - 遗留 event consumer 已移除，回归合同已固定

4. 后端事件是唯一真源，UI 不猜语义  
   - 后端 `rust/crates/debug-server/src/mobile_ws.rs` 统一产出 mobile item lifecycle  
   - 缺字段在投影层暴露 `schema_error`，不使用 `tool/unknown` 语义占位

5. 每次 build 都能运行回归矩阵，验证推理链路  
   - 本轮已全量执行：`./scripts/regression/run_android_client_matrix.sh`  
   - 最新结果：`reports/regression/20260528-goal-audit/android-matrix.log` 显示 `all passed`

## 关键证据索引

- Rust 目标测试：
  - `reports/regression/20260528-goal-audit/cargo-fin-debug-server-mobile_ws.log`
  - `reports/regression/20260528-goal-audit/cargo-agent-control.log`
- Android unit+assemble：
  - `reports/regression/20260528-goal-audit/android-gradle-unit-assemble.log`
- 回归矩阵：
  - `reports/regression/20260528-goal-audit/android-matrix.log`
- Live provider smoke：
  - `reports/regression/20260528-goal-audit/live-provider-smoke.log`
  - `/Users/fanzhang/.fin/harness/runs/test-android-mimo-goal-20260528-audit/provider-live-smoke-report.json`
- Turn-channel E2E 结果：
  - `reports/android-mvp-logs/turn-channel-e2e.log`
- 真机证据：
  - `reports/android-device-e2e/20260528-current/connection-events-002237.log`
  - `reports/android-device-e2e/20260528-current/screen-002237.png`

## 当前剩余风险

1. `turn.item.delta` 虽已接入，但本轮尚未稳定形成“推理中高频增量刷新”的完整可视化闭环；已不影响“推理链路可用”的主目标，但影响“推理过程实时渲染密度”。
2. 真机滚动与会话体验已修复，但仍建议在更多 Android WebView 机型上做一轮手工巡检。
