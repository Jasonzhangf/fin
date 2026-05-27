# Android Agent 推理链路当次验收（2026-05-27）

## 1. 安装与启动
- 设备：`100.127.23.27:1234`
- 安装命令：
  - `adb -s 100.127.23.27:1234 install -r android-client/app/build/outputs/apk/debug/app-debug.apk`
- 结果：`Success`

## 1.1 本轮修复（用户阻塞问题）
- 问题：对话框无法上滑滚动（会被自动吸底拉回）。
- 唯一根因：`mobile-shell.html` 自动吸底策略与 WebView 手势滚动竞争，且旧逻辑含 `window.scrollTo(...)` 干扰。
- 修复：
  - 新增 `S.userTouchScrolling`；
  - `scrollToBottom(force)` 仅在 `!inputFocused && !userScrolledUp && !userTouchScrolling` 下自动吸底；
  - 移除 `window.scrollTo(0,document.body.scrollHeight)`；
  - `setupScrollTracking()` 增加 touchstart/touchmove/touchend/touchcancel。
- 回归门禁同步：
  - `android-client/app/src/test/java/com/fin/client/MobileShellLayoutContractTest.kt`
  - `android-client/scripts/smoke/layout-focus-contract-smoke.mjs`

## 2. 当次真机日志切片（干净）
- 清空日志后重启 app：
  - `run-as com.fin.client sh -c ': > files/logs/connection-events.log'`
- 证据文件：
  - `reports/android-device-e2e/20260527-current/connection-events-current-v3.log`
- 关键事实（同一文件内可见）：
  - `turn.item.started` + `turn.item.completed`（正常链路）
  - `turn.item.failed`（失败链路，包含 failed item）
  - `turn.completed` / `turn.rendered`
  - WS/runtime/provider 三层状态分离可见。

## 2.1 新增当次证据（23:50）
- 日志：
  - `reports/android-device-e2e/20260527-current/connection-events-goal-235009.log`
- 截图：
  - `reports/android-device-e2e/20260527-current/screen-goal-235009.png`

## 3. 真机截图
- `reports/android-device-e2e/20260527-current/screen-normal-turn-v3.png`
- `reports/android-device-e2e/20260527-current/screen-failed-turn-v3.png`

## 4. 回归矩阵
- 命令：`./scripts/regression/run_android_client_matrix.sh`
- 结果：`[android-matrix] all passed`
- 包含：
  - `unit_test`
  - `assemble_debug`
  - `ws_event_contract_smoke`
  - `layout_focus_contract_smoke`
  - `turn_channel_e2e`
  - `projection_contract_check`
- 本轮最新输出：
  - `/tmp/android-matrix-latest.log`
  - 关键统计：`item_started=16`，`item_terminal=16`，`failed=5`，`rendered=4`

## 5. turn-channel E2E 本轮结果
- 证据：`reports/android-mvp-logs/turn-channel-e2e.log`
- 关键检查：
  - `item_started_present=true`
  - `item_terminal_present=true`
  - `each_item_started_has_terminal=true`
  - `failed_items_keep_error_summary=true`
  - `turn_started_present=true`
  - `turn_completed_present=true`
  - `item_started_has_label=true`
  - `item_started_has_title=true`
  - `item_started_has_purpose=true`
  - 语义示例：`Provider Call` / `Ran` / `Reasoning Stop`，不再出现 `tool/unknown` 占位。

## 6. provider live smoke（mimo）
- 命令：
  - `./scripts/run-real-provider-smoke.sh android-mimo-goal-20260527`
- 结果：`EXIT:0`
- 收据：
  - `/Users/fanzhang/.fin/harness/runs/test-android-mimo-goal-20260527/provider-live-smoke-report.json`
- 关键信息：
  - `provider_name=mini27`
  - `model=MiniMax-M2.7`
  - `protocol=OpenAiCompatible`
  - `turn_count=3`

## 7. 当前未闭合风险
1. 通过 `finAutoSend` 注入文本时，带空格 payload 在 `adb am start --es` 上存在被切分风险；建议后续统一为 base64 extra 或 native debug API，避免注入文本变形。
2. `turn.item.delta` 在本轮样例中不稳定（started/completed/failed 已完整）；若你要求“推理中更细粒度增量文本持续刷新”，需要继续在 runtime 端补稳定 delta 产出并加合同测试。
