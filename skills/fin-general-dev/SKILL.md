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
