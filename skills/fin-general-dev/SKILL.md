---
name: fin-general-dev
description: Project-local default development workflow for fin. Use for feature work, refactors, and bug fixes before touching repo code.
---

# fin General Dev Skill

## 1) Intent

这是 fin 的默认开发入口 skill。

它只回答四件事：
1. 任务属于哪一层
2. 哪个文件/模块是真源
3. 最小正确改动点在哪
4. 最小验证矩阵是什么

## 2) Canonical sources

1. `AGENTS.md`
2. `docs/architecture/02-layer-boundaries.md`
3. 对应子系统 architecture 文档
4. 相关 crate / 模块源码

## 3) Workflow

1. 先读 `AGENTS.md`
2. 先找 owning layer
3. 再找唯一真源
4. 先定义最小验证，再实现
5. 只改 owning layer，避免跨层复制逻辑
6. 代码文件默认必须 `< 500` 行；仅允许极少数白名单文件豁免，并通过门禁脚本检查
7. 如新增稳定规律，更新 docs 或本地 skill

## 3.1) fin project-specific sink

- 本 skill 只沉淀 `fin` 项目特有规则，不重复跨项目通用方法论。
- `fin` 当前应沉淀到本地的内容包括：
  - session / context / tentative session
  - task / topic / routing / confidence
  - digest / rebuild / continuity
  - control block / progress block / execution note / knowledge artifact
  - project leader / orchestrator / heartbeat / recovery
  - source card / user activity card / text channel delivery / tool semantic render
- 这些内容进入本地 `skills/` 与 `docs/architecture/`，不回灌到全局 skill。


## 3.2) Module delivery baseline（路由到全局 skill）

模块级通用骨架不再在本地重复展开，统一遵循全局 `coding-principals`：

- Shared Functions + Blocks + Orchestration
- Operation + Event + Projection
- Module Debug Baseline
- Module Test Baseline
- Evidence-first Delivery

本地 skill 只补 `fin` 自己的附加要求：

- 结构化事件默认要能落到 `~/.fin` 对应证据目录
- 模块设计要兼容 session / workdir / runtime home 的分层
- Web 只能做观察层，不能补 runtime 真相
- 若模块影响多 agent / runtime / debug 链路，必须补 replay 或等价回放证据
- context / debug snapshot 默认采用 bounded-write；禁止无界散写与每轮碎片化落盘
- 生命周期设计默认前台可控；未获批准不引入 detached daemon / orphan process
- 正式 build / install / promote 统一走 build-versioning flow；不要把裸 `cargo build` 当成交付闭环
- 非白名单代码文件必须遵守 500 行上限；门禁脚本真源为 `scripts/check-code-line-limit.py`
- 对“工具分发/控制”类模块默认使用“薄编排 + 功能切片”拆分：入口文件只做路由，执行逻辑按能力拆到独立文件，避免 dispatcher 再次长成单体文件
- 新增共享视图真源（如 activity cards / tool semantics）时，先做后端 contract + builder，再让 Web/QQ 只接读取；不要先在某个 UI 组件里硬写展示对象
- 文字 channel 若不支持编辑，默认策略是“compact redraw + persisted delivery state”；不要在 adapter 里临时拼零碎 delta 行冒充统一卡更新
- channel gateway 的正常回复与异步补充回复都必须从 session truth 发出；禁止直接拿 handler 返回文本绕过 session/message ledger，否则会丢上下文恢复与重复发送判定真源
- progress / activity card 的语义层禁止回显用户提示词、provider base URL 等内部输入细节；`provider.call` 只允许显示模型标识，且 recent actions 优先展示真实工具动作，provider.call 只在没有其他动作时兜底显示
- Web debug 实例纪律必须单一；当前对外只允许引用 `4040`，禁止把历史测试端口混进当前验证与汇报
- Android / WebUI 若宣称“已连 daemon”，必须先证明 `:4040` 监听者就是 always-on headless daemon（例如 lease/state + 4040 连通性 + 非 web-debug 临时前台）；未完成该 ownership 证明前，不得把临时 web-debug 验证当成 daemon 验证
- `ControlFeedback` / continuity / topic-shift / simple-query 判断必须由 runtime 产出单一事实，并进入 event + session artifact + note/digest；Web 只消费，不得二次推断
- framework-owned resume/checkpoint prompt 允许进入 turn/step/provider/debug truth，但禁止写入用户可见 conversation；pause/resume 真相由 execution checkpoint + consumed event 维护，不允许 UI/adapter 伪造恢复语义
- detached/headless always-on 的生命周期真相必须落在 framework-owned pid/lease/state/recovery artifacts；前台请求链只能消费或唤起，不能冒充 daemon supervision 真源
- 模型输出的结构化 block 必须做 schema 识别；错 shape 的 JSON 只能当 raw provider output 观察，不能冒充有效 control feedback
- 若模型输出的 control block 只是在 value type 上不合法但包含白名单 key，runtime 应做 mask 白名单提取与受控 coercion；未知字段一律丢弃，不允许半结构化脏数据进入 control truth
- 新增或改造关键流水线/数据源时，必须先对齐 `docs/architecture/44-pipeline-unique-type-and-error-chain.md`：按 `<Domain><Direction><NN><Node>` 建唯一类型，显式连接请求链/响应链/错误链；默认禁止中间插节点，新增能力优先进既有节点内部 block / validator / parser
- normal conversation 与 debug 共享同一份 session render truth；所谓 richer UI 是 richness level 的差异，不允许做两套前后端真源
- `Minimal / Rich / Full Trace` 这类 UI richness 开关只能控制展示层次，不能切换事实来源；所有 richness 都必须基于同一份 `operation_id -> session artifacts` 绑定结果
- 用户在任务进行中询问“当前状态”的请求应视为特殊并行 inquiry；必须带显式标记，走 non-interrupting side path，从最新 progress/note/control/tool state 组装回复，不能打断主推理
- 同一 turn 的自动工具编排必须有轮次上限与结构化限幅事件（如 `reasoning.auto_tool_roundtrip_limit_reached`），防止模型重复工具调用导致无界循环
- tool catalog 必须与 runtime dispatcher 同步维护：真可调用工具进入 `model_tools`，未接线但设计上存在的工具族进入 `disabled_tools/planned`；禁止出现“文档说有、模型上下文里完全看不到”的工具真相漂移
- 项目进入 M1 收口模式后，默认优先做 `scope freeze -> regression matrix -> blocker fix -> receipts`；非阻塞的新框架/新模块需求先登记到 closeout/backlog 文档，不直接实现
- 启动配置若区分 user/system 两层，真正可编辑的 system-only 字段（如 startup topology）必须从 `~/.fin/config/system.toml` 读回再生效；不能只生成模板却运行时永远忽略
- owner-loop 若已进入 managed task path，后续状态推进必须优先使用 `project.task.create/claim/submit/review` 这组 task-system truth；不要再靠自由文本或 note 冒充 dispatch/review 真相
- tentative session 的 runtime truth 必须允许 `session_id` 已存在而 `task_id` 仍为空；restore/materializer/last_run 路径都不得在 formalize 之前合成伪 `task_id`，formalize 后的 `topic_thread_id` 也不得被后续 last_run 覆写丢失

## 4) Minimal validation matrix

- L1 Unit：纯函数、状态机、schema
- L2 Contract：HTTP / WS / envelope
- L3 Harness：回放与故障注入
- L4 Cluster：多 worker / 多 node
- L5 Manual Debug：Web 调试后台观察

## 5) Anti-patterns

- 没确认 owning layer 就直接改
- 在 web 层补逻辑掩盖 runtime 真相
- 只改 docs 不补验证
- 只跑 unit 就宣称多 agent 行为完成
- 模块实现没有 operation/event/debug/test 骨架

## 6) Android 每轮更新标准流程（强制）

每次 Android 客户端改动后，必须执行并留证据：

1. **构建与升级包产出（必做）**
   - `cd android-client && ./gradlew :app:assembleDebug`
   - `cd android-client && ./scripts/build-and-publish.sh`
   - 必须产出：
     - `android-client/update-dist/fin-latest-debug.apk`
     - `android-client/update-dist/latest.json`

2. **真机安装（必做）**
   - 使用指定 adb 测试机安装最新 APK
   - 记录安装日志到 `reports/android-mvp-logs/` 或 `reports/session-kb-logs/`

3. **在线测试（必做）**
   - 客户端内执行“检查更新 -> 下载 -> 安装”链路，至少跑一轮在线更新检查
   - 同时验证核心对话链路可连接 daemon 并收发一次真实推理
   - 记录日志 + 截图

4. **验收索引（必做）**
   - 将本轮 PASS/FAIL 与证据路径写入验收 md
   - 无在线测试证据，不得宣称完成

5. **前端语义约束（必做）**
   - 用户端禁止直接输入/编辑 JSON 配置。
   - 前端只提供语义化交互入口（模式、模型、effort、选项等）。
   - 后端负责生成/写入唯一语义解析 block（JSON 真源）。
   - 禁止前端实现第二套业务 JSON 解析与拼装真相。
## 3.3) Agent-first dispatch 原则（新增）

### 核心原则
**dispatch 必须是 agent 驱动的，不是 framework 预编的**：
1. **framework 只被动触发**：框架的触发点只能是 control block 满足、或显式条件达成；不允许框架自行合成业务语义。
2. **framework 提供工具 + 限制**：框架只暴露 dispatch tool、限制边界，不预设何时派发、派什么任务。
3. **agent 决定何时派发**：system agent 推理后决定是否调用 dispatch tool；project agent 推理后决定是否向 system 回报。
4. **唯一真源**：业务 payload（任务内容、任务摘要、结论）由模型生成，框架只执行传输和状态推进。

### 正确流程
```
user request
  → system agent 推理
    → system agent 调用 dispatch tool（如 agent.assign）
      → framework 执行 send mailbox 到 project agent
        → project agent 接收并执行
          → project agent 回报进度（progress 事件）
          → project agent 回报结果（result 事件）
        → system agent 接收结果
          → system agent 决定下一步（继续派发 / 总结 / 等待）
```

### 错误模式（反模式）
- harness 硬编码 `task.summary` 作为 dispatch payload 真源
- system self-mailbox loopback 冒充 dispatch
- framework 在 agent 推理前预先合成派发行为
- project agent 结果由 harness 注入 summary，不经模型
- 缺少 progress 事件的持续回报机制
- project agent 静默结束，无状态上报

### 验证要求
dispatch 相关功能必须验证：
1. system agent 确实调用了 dispatch tool（tool call artifact 存在）
2. dispatch payload 是模型生成的内容，不是 harness 硬编码
3. project agent 收到真实任务文本，不只是 task_id
4. project agent 执行过程中有 progress 事件链
5. project agent 完成后有结构化 result 回报
6. system agent 收到结果后有后续推理（不是 harness 直接收口）

### 最小 harness 职责
```
- ingress: 把 user request 写入 system session ledger
- transport: 执行 agent 之间的 mailbox send（由 tool call 触发）
- observe: 等待并记录 progress / result 事件
- report: 把执行结果写入 evidence directory
```
harness **不得**：
- 预设 dispatch 时机
- 合成 dispatch payload 内容
- 跳过 project agent 直接给 system 注入 summary
- 伪造 agent reasoning trace

## 7) Agent dispatch / harness 边界（新增）

当任务涉及 `system agent -> project agent` 协作时，默认规则：

1. framework 只提供工具、transport、权限边界、事件记录；不替 agent 做任务决策。
2. 是否 dispatch，必须由 agent 自己在推理中决定；不得由 harness 预先替它派发。
3. dispatch payload 的业务语义（任务描述、目标、交付物）必须由模型生成；harness 只负责传输。
4. framework 只能在 control block 或明确条件达成后被动触发；禁止“为跑通而主动帮模型做决定”。
5. 若当前 agent 不能跨 cwd / 权限边界执行，正确做法是暴露 dispatch tool 给它，而不是让 framework 偷偷越权执行。
6. project agent 的 subagent / 内部执行细节默认对其他 agent 不可见；跨 agent 只交换 mailbox progress/result truth。

### 这一类任务的最小验证新增要求
- 证明 user request 先进入 system agent，而不是 harness 直送 project
- 证明 dispatch task_description 来源于模型/tool call，而不是 harness 硬编码
- 证明 project progress/result 通过 mailbox/ledger 回到 system
- 证明 system 基于 project result 继续推进或收口，而不是 harness 代替收尾

### 反模式
- harness 预生成 dispatch task_summary
- harness 决定是否 dispatch
- system self-loopback 冒充真实 agent 通信
- framework 直接 synthesize system summary / final answer
