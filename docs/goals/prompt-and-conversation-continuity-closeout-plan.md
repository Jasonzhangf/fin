# Prompt 与连续对话渲染收口计划

## 目标

把 `fin` 当前“工具已经存在但模型不会正确使用”和“界面仍像独立问答卡片而不是连续对话线程”两个问题，一次性收口到唯一真源：

1. runtime prompt system 必须明确告诉 `system agent / project agent`：
   - 什么时候直做
   - 什么时候查看 framework 状态
   - 什么时候派发给别的 agent
   - 派发后如何持续跟进、收尾、汇报
2. WebUI / Android 必须按 session ledger / turn timeline 渲染连续会话，而不是把每轮割裂成孤立卡片。
3. headless 多 agent 派发、progress、完成、失败、断连恢复必须有真实 E2E 回归证据。

## 验收标准

### Prompt / Runtime

- `system agent` 遇到跨 `cwd` / 跨 project 工作时，不再停留在泛化建议，而会：
  - 先看 peer / presence / supervision / backlog
  - 选择目标 agent
  - 派发 bounded task
  - 等待明确结果或失败状态
  - 把结果回报给当前用户会话
- prompt 明确要求：
  - 不允许“首个工具调用后静默停止”
  - 不允许把 chat 文本当控制面真相
  - 不允许重写历史卡片
  - 必须按 append-only timeline 推进

### UI / Channel

- WebUI / Android 主对话区表现为连续聊天线程：
  - user / assistant / live progress / tool timeline 都是时间顺序追加
  - 历史消息一旦落盘，不会被后续 live tool 事件回写修改
  - project agent 进度作为 pinned / expandable 子线程展示
- channel 只连接并绑定 `system agent`；`project agent` 只以 runtime snapshot / activity 方式可见。

### Harness / E2E

- 本地双实例真实跑通：
  - `system agent` 接收用户请求
  - 识别需要 `project cwd`
  - 通过 control plane 派发给目标 `project agent`
  - `project agent` 执行并回报进度 / 工具事件 / 最终结果
  - `system agent` 收到结果后完成收尾并面向用户汇报
- 必须覆盖失败与恢复场景：
  - 连接不上
  - 中途断连
  - 恢复连接
  - 子 agent 执行失败
  - wait timeout
  - close / resume 生命周期

## 范围

### In Scope

- `rust/crates/runtime` prompt assembly / role baseline / turn rules
- `rust/crates/cli` headless multi-agent harness / ledger-first read path / lifecycle regression
- `rust/crates/debug-server` Web debug / mobile ws snapshot feed
- `android-client` 连续对话渲染、pinned agent progress、工具 timeline
- 回归脚本、build-all、local dual-instance E2E、live provider E2E receipts

### Out of Scope

- 新增第二套 UI 会话语义
- 通过 fallback/mock 冒充真实 agent 生命周期
- 把 project agent 直接暴露成用户主入口
- 用压缩 prompt / 减少 payload 冒充“稳定性提升”

## 设计原则

1. **唯一真源**
   - agent lifecycle 真源：runtime control-plane + mailbox + run status
   - 会话渲染真源：session ledger / session materializer artifacts
   - UI 只读，不补业务语义
2. **无 fallback**
   - 不保留临时文件 IPC、旧 projection 作为控制真相、UI 自己推断状态
3. **append-only timeline**
   - 历史卡片不可被 live state 回写
4. **agent-first prompt**
   - 模型看到的是 framework-routed work item，不是直接用户聊天
5. **先测试后实现**
   - 每个 gap 先写红测，再实现，再 build / install / live smoke

## 当前已确认事实

1. runtime prompt 已补了一部分 cross-cwd dispatch / continuity / no-silent-stop 规则，但还未形成完整交付闭环。
2. Web debug chat 已开始从 turn-card 向 thread render 演进，但还需要以 ledger-first read path 和 pinned project progress 收口。
3. 本地多 agent lifecycle harness 已具备 durable mailbox / run status 真源，且存在 local/live receipts。
4. ledger 写侧已经存在；读侧仍有部分 consumer 依赖旧 projection 文件路径，需要继续迁移。

## 技术方案

### A. Prompt 系统收口

文档真源：

- `docs/architecture/25-prompt-system.md`
- `docs/architecture/26-role-prompt-family-and-model-overlays.md`
- `docs/architecture/27-stable-core-prompt.md`
- `docs/architecture/29-multi-turn-history-model.md`
- `docs/architecture/24-session-render-truth-and-reasoning-input-assembly.md`

代码落点：

- `rust/crates/runtime/src/prompt_assembly.rs`
- `rust/crates/runtime/src/prompt_tests_basics.rs`

要求：

- system role 明确：
  - cross-cwd => project-management work
  - inspect `peer.list` / `peer.describe` / `agent.presence.list` / `project.supervision.list`
  - bounded dispatch + explicit wait + explicit result refs
- project role 明确：
  - project truth / review closure / worker handoff ownership
- continuity 明确：
  - 当前 turn 是连续会话中的下一步，而不是重开新问答
  - 首个工具调用后必须继续推进直到完成/失败/等待

### B. Ledger-first 连续对话渲染

代码落点：

- `rust/crates/cli/src/session_ledger_read.rs`
- `rust/crates/cli/src/session_commands.rs`
- `rust/crates/cli/src/channel_peer_conversations.rs`
- `rust/crates/cli/src/status_probe.rs`
- `rust/crates/cli/src/provider_live_smoke_report.rs`
- `rust/crates/cli/src/web_debug_support.rs`
- `rust/crates/cli/src/web_debug_turns.rs`
- `rust/crates/debug-server/src/mobile_ws.rs`
- `rust/crates/debug-server/webui/src/chat.ts`
- `rust/crates/debug-server/webui/styles.css`
- `android-client/app/src/main/assets/mobile-shell.html`

要求：

- 所有 session list / turn list / context / digest / activity feed 先读 ledger/session truth
- `conversation/messages.json`、`recent_*` 仅保留兼容投影，不再作为 authority
- UI 渲染模型改成：
  - thread
  - message bubble
  - live progress row
  - pinned child-agent detail

### C. Headless 多 agent harness 闭环

代码落点：

- `scripts/run-local-multi-agent-e2e.sh`
- `scripts/regression/run_local_regression.sh`
- `scripts/build-all.sh`
- `rust/crates/cli` 下 multi-agent harness / live smoke / web debug tests

要求：

- 默认编译回归包含 local dual-instance static E2E
- live 模式包含真实 provider 双实例 E2E
- 每次生成 receipt，落盘到 `reports/regression/...`

### D. Android / Web 真机与真页面 smoke

要求：

- Android：
  - build
  - adb install
  - 连接真实 daemon
  - 验证连续会话、输入绑定、pinned project agent、工具 timeline
- Web：
  - 打开 debug console
  - 验证连续 thread 渲染与 activity snapshot 一致

## 风险与规避

1. **旧 projection 兼容路径还被隐藏依赖**
   - 规避：先补红测，证明 `session_messages_path` 缺失时仍能工作
2. **UI thread render 与 Android shell 时间格式不一致**
   - 规避：补 contract smoke，校验 timestamp / updated_at shape
3. **真实 provider E2E 超时**
   - 规避：按 connect / provider wait / tool wait 分阶段 timeout，禁止短 wall-clock 总超时
4. **control-plane 与 UI 再次双真源**
   - 规避：所有状态只从 runtime snapshot / ledger 读；UI 不做 lifecycle 推断

## 测试计划

### 1. Prompt / Runtime 单测

- `cargo test --manifest-path rust/Cargo.toml -p fin-runtime prompt_tests -- --nocapture`
- 新增 / 维持：
  - cross-cwd dispatch guidance
  - continuity guidance
  - no-silent-stop guidance
  - append-only history guidance

### 2. CLI / Ledger / Web Debug 定向测试

- `cargo test --manifest-path rust/Cargo.toml -p fin-cli session_commands::tests -- --nocapture`
- `cargo test --manifest-path rust/Cargo.toml -p fin-cli channel_peer_conversations::tests -- --nocapture`
- `cargo test --manifest-path rust/Cargo.toml -p fin-cli local_multi_agent_lifecycle_harness_tests -- --nocapture`
- `cargo test --manifest-path rust/Cargo.toml -p fin-cli web_debug_tests -- --nocapture`
- 新增：
  - `provider_live_smoke_report` 在 `last_run.session_messages_path` 缺失时仍可读 session 真源
  - `web_debug_support` / `web_debug_turns` 不依赖旧 projection anchor

### 3. Debug Server / Mobile Contract 测试

- `cargo test --manifest-path rust/Cargo.toml -p fin-debug-server -- --nocapture`
- `node android-client/scripts/smoke/ws-event-contract-smoke.mjs`

### 4. 本地双实例 E2E

- static dual-instance harness
- live real-LLM dual-instance harness
- 覆盖：
  - dispatch
  - progress update
  - final result
  - connect failure
  - disconnect/reconnect
  - execution failure
  - timeout
  - close/resume

### 5. 安装态 / 客户端 smoke

- `scripts/build-all.sh`
- Android `adb install -r ...apk`
- Web debug console live smoke

## 实施顺序

1. 继续清掉 read-side 非 ledger-first 入口，补红测再迁移
2. 补齐 prompt continuity / managed execution / cross-agent follow-through 文案与单测
3. 收口 Web debug / Android 为连续 thread + pinned project agent progress
4. 跑本地双实例 static E2E
5. 跑本地双实例 live LLM E2E
6. 跑 Android/Web live smoke
7. 更新 closeout receipt、必要 docs、skills 精华

## 完成定义（DoD）

必须同时满足：

1. 模型在真实多 agent 请求里会正确判断并派发给目标 project agent，而不是只说建议。
2. 派发后主会话能看到连续 progress，project agent 子线程可展开，任务最终有 completed/failed 明确收尾。
3. WebUI / Android 主界面呈现连续对话线程，历史不会被 live tool 事件篡改。
4. read-side 关键消费链路不再依赖旧 projection 文件作为 authority。
5. local dual-instance static/live E2E、build、Android/Web smoke 都有 receipts / logs / artifacts。

