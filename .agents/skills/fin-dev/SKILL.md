---
name: fin-dev
summary: Android 客户端开发强制测试回环（先测后合入）
---

# fin-dev skill（强制）

## 0. 目标
在 Android 客户端任何功能合入前，必须完成：
1) 功能实现
2) 基础测试回环（单元+契约+端到端）
3) 本地验证
4) 证据落盘

无证据=未完成=禁止合入。

## 1. 功能拆分与测试回环（每个功能都要有）
对每个功能建立以下固定回环：

### F1 连接层（扫码/WS/状态机）
- Unit
  - 状态机 happy path
  - 4类异常态转移（auth_failed/endpoint_unreachable/stale_token/protocol_mismatch）
- Contract
  - 握手 envelope schema
  - subscribe topic schema
- E2E
  - mock ws server 下完整链路：idle->...->healthy
- Evidence
  - reports/android-mvp-logs/connection-*.log
  - 页面截图 connection

### F2 Session 管理（列表/过滤/切换/恢复）
- Unit
  - session 选择与 currentSession 绑定
  - 重连恢复不串台
- Contract
  - session.list/session.bound 事件字段完整性
- E2E
  - reconnect 后同 session_id
- Evidence
  - session-recovery.log
  - sessions 页面截图

### F3 输入管理（队列/去重/补发）
- Unit
  - dedupeKey 唯一性
  - 状态机 draft->queued->consumed->rendered->delivered
- Contract
  - session.user_input payload 字段
- E2E
  - duplicate skipped
  - reconnect/restart 不重复补发
- Evidence
  - input-dedupe.log
  - conversation 页面截图

### F4 Turn 渲染（Compact/Debug）
- Unit
  - turn card 渲染字段齐全
- Contract
  - turn.rendered 事件字段映射
- E2E
  - user/assistant/control/tool/closure 全显示
- Evidence
  - turn 渲染日志 + 截图

### F5 状态可观测（worker/project/daemon）
- Unit
  - runtime payload 映射
- Contract
  - runtime.workers/runtime.projects/runtime.daemon 字段
- E2E
  - 三类状态均可见
- Evidence
  - runtime 日志 + runtime 页面截图

### F6 反刷屏门禁
- Unit
  - 同 signature dedupe
  - idle no-progress skip
  - binding mismatch cleanup
- E2E
  - 不重复投递验证
- Evidence
  - anti-spam.log

### F7 更新系统（update-dist + latest.json + app内检查）
- Unit
  - latest.json schema
- E2E
  - 生成 APK + latest.json
  - app 检查更新读取 latest.json
- Evidence
  - update-dist/*
  - 更新页面截图

## 2. 合入门禁（必须全部通过）
每次改动提交前执行：
1) `./scripts/android-mvp/run_connection_matrix.py`
2) `./scripts/android-mvp/run_session_input_matrix.py`
3) `./scripts/android-mvp/capture_shell_screenshots.py`
4) `cd android-client && ./gradlew :app:assembleDebug`
5) `cd android-client && ./scripts/build-and-publish.sh`

通过标准：
- 日志文件全部生成且无失败关键字
- 截图文件生成
- APK 与 latest.json 生成

## 3. 证据落盘标准
- 日志：`reports/android-mvp-logs/*.log`
- 截图：`reports/android-mvp-screenshots/*.png`
- 验收索引：`reports/android-mvp-validation.md`
- 发布物：`android-client/update-dist/`

## 4. 反模式（禁止）
- 只有按钮/占位无真实事件链
- 只编译不跑 e2e
- 没有日志和截图就宣称完成
- Android 另起双真源语义


## 5. Jason 新增强制规则（2026-05-16）
- 每次执行都必须记录 success/failure 到 `note.md`（包括证据路径与下一步动作）。
- 跨设备客户端默认是“远程 WS 一等公民”：daemon 在本机、客户端在局域网，通过 Tailscale 连通；禁止把 `127.0.0.1` 本地化思路当主路径。
- 任何“可用性”结论必须先有远程握手/订阅真实日志，再有真机页面证据；缺一即 FAIL。
- Android 网络排查固定四段：`host_tcp -> device_shell_tcp -> app_uid_shell_tcp -> app_ws_handshake`；前三段通过但第四段失败时，必须归类为 `webview_or_app_runtime_ws_path_issue`，禁止误判为 daemon 不可达。

## 6. 界面修改与调试流程（统一收敛到 fin-dev）

### 6.1 UI 修改硬约束（手机用户态优先）
- 用户态默认只显示可读信息；调试原始 JSON 必须受 debug 开关控制，默认关闭。
- 设置入口必须可达、可回退；禁止出现“能进入不能退出”的面板流程。
- 任何 UI 结构调整后，必须同步更新 `scripts/android-mvp/capture_shell_screenshots.py`，确保截图门禁可复现。
- `+` 菜单、发送按钮、输入区交互必须有明确收起/可发送行为；禁止只做视觉占位。

### 6.2 推理渲染约束（Normal/Debug 双通道）
- 单一 turnStore 真源：仅从 `turn.rendered` 入库，禁止本地占位 turn（例如“思考中”）。
- Normal 通道：面向人类可读摘要（assistant 回复 + 可读的工具/错误 timeline）。
- Debug 通道：只在开关开启时显示 raw debug 结构（tool/error/control/closure）。
- debug 开关不得触发重连、重发、重绑；仅允许渲染层分叉。

### 6.3 连接稳定性与错误流程
- 必须实现自动重连（指数回退），并区分“可重试错误”和“终止错误”：
  - 终止：`auth_failed` / `protocol_mismatch` / `stale_token` / `no_profile`
  - 重试：`endpoint_unreachable` / `closed` / runtime exception
- 连接状态必须结构化记录到连接事件日志，便于回放和定位。

### 6.4 真机验收最小回环（每次 UI/连接改动后必跑）
1. `cd android-client && ./gradlew :app:assembleDebug`
2. `adb -s <device> install -r app/build/outputs/apk/debug/app-debug.apk`
3. `python3 scripts/android-mvp/capture_shell_screenshots.py`
4. `python3 scripts/android-mvp/run_turn_channel_e2e.py`
5. 必须落盘：
   - `reports/android-mvp-logs/turn-channel-*.log`
   - `reports/android-mvp-screenshots/turn-*.png`
   - `reports/android-mvp-screenshots/device-*.png`（至少一张真机图）

## 7. 本地 skills 收敛规则（避免分散）
- 当前项目本地 skill 统一收敛到：`.agents/skills/fin-dev/SKILL.md`。
- 新增经验默认写入 fin-dev，不再新建平行本地 skill（除非用户明确要求拆分）。
- 若发现旧 skill 与 fin-dev 重复，优先合并到 fin-dev 并删除重复入口，避免多真源分散。
