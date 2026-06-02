---
name: fin-testing-harness
description: Testing and harness workflow for fin. Use for replay design, fault injection, and CI validation planning.
---

# fin Testing + Harness Skill

## 1) Intent

用于确定测试层级、回放方式、故障注入矩阵以及 CI 最小门禁。

## 2) Canonical sources

1. `docs/architecture/07-harness-replay-fault-injection.md`
2. `docs/architecture/08-testing-and-ci-strategy.md`
3. `docs/architecture/15-install-build-regression-flow.md`
4. 相关 replay / fixture 文件

## 3) Workflow

1. 先确定改动影响的层级
2. 先按固定顺序选最小测试集：`provider config -> provider slice -> runtime builder -> recording/projection -> debug/install`
3. 先决定证据写入 `~/.fin/harness/reports/` 与 `~/.fin/logs/regression/` 的落点
4. 测试必须使用隔离的 test `user.toml`、test runtime home、test session namespace，禁止污染正常 `~/.fin/sessions/*`
5. 若跨 runtime / transport 边界，必须补 harness 场景
6. 若修复可复现故障，优先沉淀 replay 场景
7. 正式 build 的回归必须能被统一 build flow 自动调用，不能依赖人工拼接命令


## 3.1) Module test baseline（路由到全局 skill）

模块级测试闭环的通用分层统一遵循全局 `coding-principals` 中的：

- Unit
- Function / Contract
- Orchestration Regression
- Installed-binary / Runtime Smoke

本地只补 `fin` 的落点要求：

- 回归报告进入 `~/.fin/harness/reports/`
- 回归日志进入 `~/.fin/logs/regression/`
- 影响 runtime / provider / debug 链路的改动，优先补 replay / harness smoke
- 多轮上下文闭环优先用 `transcript-demo` 场景固定复现；不要只看单轮 `runtime-demo` 就宣称 context 生效
- provider 真实/半真实测试默认使用 `~/.rcc/provider/ali-coding-plan/config.v2.json` 生成 test `user.toml`
- 默认 provider/model 冻结为 `ali-coding-plan` / `qwen3.6-plus`，协议为 `anthropic-wire`
- 测试 session 必须带 `test-` 命名空间，并写入 `~/.fin/harness/runs/<run-id>/...` 的隔离目录
- mainline receipt 若需要真实 `auto_tool_roundtrip` 样本，优先用 `fin mainline-demo <user.toml>` 生成 deterministic 3-turn + 2-round tool-loop session；不要拿单轮 tool dispatch 冒充多轮 roundtrip
- mainline receipt 若需要 stronger `control_boundary` 样本，优先用 `fin control-boundary-demo <user.toml>` 生成真实 `pause -> queue -> resume-run -> wait_external -> reminder_fired -> supervisor_heartbeat_due/stale_lease` 场景；不要只靠空 heartbeat/daemon state 就宣称 control-plane 够强
- 真实 provider 多轮 smoke 优先用 `scripts/run-real-provider-smoke.sh [test-run-id]`（内部调用 `fin provider-live-smoke <user.toml>`）；receipt 必须落到 `~/.fin/harness/runs/<run-id>/provider-live-smoke-report.json`，并检查 `control_feedback_origin != runtime_heuristic`，同时记录 `reasoning_stop_present`，避免把 heuristic 观察态误判为真实闭环
- 真实 provider/live E2E 不得在 transcript/prompt 中硬编码“必须先调用某工具再调用某工具”；live 场景只定义目标、上下文、可用工具与验收条件。若要固定工具序列，请改用 contract/replay 测试，不要冒充真实模型自治闭环
- 真实 provider/live E2E 的 user turn 必须保持“用户纯度”：只能写真实用户会说的自然语言请求，禁止把 `reasoning.stop`、tool call、control block、closure、framework 指令或任何内部控制语义写进模拟用户输入
- live E2E 的工具使用指导不得写进 user turn；模型应只从 tool catalog / tool prompt spec 理解“何时用、如何用、不要何时用”这些规则
- live E2E 不得把 framework control plane 语义暴露成可见输入层：既不能写进 user turn，也不要把 runtime 内部控制实现伪装成 agent 自述；测试只验证外部行为与落盘事实，不测试“agent 是否知道 framework 内部机制”
- 任何 formalize / routing / dispatch / pause / resume / scheduler 激活类验收，都必须显式核对 trigger source：**只能**来自用户刚性命令或模型已落盘的 control-block 指标；若是 framework 从自然语言、阈值、wrapper shortcut 自己猜出来的，测试应直接判失败
- live E2E 的主 oracle 是 persisted timeline truth（round / step / tool receipt / provider request/response / closure），不是结果文案是否“漂亮”；先验收流程推进与框架约束，再看内容质量
- 文字 channel/gateway 的标准回归必须显式覆盖三层：**内部推理闭环 -> gateway 即时响应/短路闭环 -> 真实业务通道闭环**；只验证 session message delivery 不够，因为 `system_notice` / queue notice / routing prompt 这类即时响应可能不会 materialize 到 session truth
- 任何 QQ/channel/runtime 文本链路修复后，**先跑标准回归顺序**：L1 内部推理/状态卡/turn truth 自闭环 -> L2 gateway/QQ 模拟输入闭环 -> L3 真机业务通道闭环；前两层没过，不得让 Jason 先做人肉验证
- 修完 user-visible channel 文案/卡片/notice 后，回归必须至少覆盖三类坏例子：`空 assistant/tool-only round`、`旧失败残留到新 waiting/ack 卡片`、`gateway 补发 no_new_messages 或重复 activity card`；否则不算完成标准回归
- 若 QQ 文本链路进入 debug/dev mode，回归还必须显式检查：`每个 execution agent 各自有状态行`、`工具摘要列表按结构化 fingerprint 去重`、`heartbeat 仍保留并行 agent visibility`；否则用户无法判断真实执行路径
- 若 execution agent 已有名字池 / `agent_id=device.agent_name` 真源，debug/dev 报告断言必须优先检查输出的是简短 `agent_name`，而不是 `System Worker ...` / `Worker project/...` 这类长 title
- 若名字显示规则涉及多设备/外部 peer，回归还必须覆盖：`单 device -> 只显示 agent_name`、`多 device -> 显示 device.agent_name`、`外部 peer -> 显示 peer.alias/label`
- 若本轮修复涉及“工具摘要里的人名/目标名”，不要只测 QQ renderer；还必须补一条 runtime tool test，确认 `target_ref/input_summary/output_summary` 已优先落简短 agent 名称，否则前台只是临时洗字，真源仍脏
- 若本轮修复涉及 QQ dev/debug 的**工具摘要压缩**，renderer 回归必须至少固定五类人机可读摘要：`agent.assign`、`mailbox.send`、`mailbox.poll`、`daemon.ensure_peer`、`update_plan`；否则用户仍看不出执行路径
- 若 `mailbox.poll` 在当前 round 里只是 `messages=0, remaining=0` 的空查询，compact/dev 卡片回归必须断言它**不出现在用户可见摘要里**；零结果 poll 不是进度
- 若 `project.task.list` 被用于前台披露任务池，回归必须断言：**首次披露详细 task ids、同一 task 集不重复细节、task 集变化后才重新披露细节**；不要把 `limit=10` 之类实现参数暴露给 QQ 用户
- 若本轮还改了 QQ debug tool summary 的**摘要保护策略**（例如避免把截断 JSON 直接发给用户），必须补一条 `spawn_activity_delivery_loop -> outbound-debug-card.jsonl` 隔离 E2E，检查真实 outbound 文本里是可读摘要而不是原始 JSON 残片
- 跑完 QQ/gateway 模拟闭环后，不要只报 `cargo test passed`；要额外翻保留下来的隔离 runtime home 真相：`runtime/peers/qqbot/outbound*.jsonl`、`runtime/peers/qqbot/events.jsonl`、`sessions/*/conversation/messages.json`、`harness/runs/*/qqbot-live-receipt.json`，确认实际发出的 ack/reply/card 内容与 receipt 判定一致
- 若主业务 `qqbot-live-receipt` 在修复后仍因**重启前最后一次真实 inbound** 的旧坏卡而失败，先补一条**隔离 runtime 的 simulated qqbot inbound -> live receipt gate** 自动回归，确认新链路已干净；不要把“历史最后一条坏卡还没被新真实 inbound 覆盖”误判成当前代码仍失败
- 当前 `project_runtime_resume` / `assignment_runtime_resume` 的标准回归真相是 **observe-only + explicit_trigger_required**；若老测试还期待 attached/headless control-plane 自动把 project/worker turn 真跑起来，应先更新测试断言，不要把“框架更被动了”误报成回归失败
- 若修复涉及“强制派发/managed execution”，标准回归必须显式检查**visible toolset 是否已切换**：进入 dispatch-required 状态后，system/owner 看到的是派发/审查工具而不是 direct repo-write 工具；若仍保留原完整工具集而只是 prompt 文案变化，不算通过
- 对“强制派发”回归，至少覆盖两类坏例子：`toolset 未切换导致 system 继续自干`、`toolset 已切换但 framework 直接替模型 dispatch`；只有“模型看到受限工具集后自己选择派发”才符合当前设计边界
- 若修复涉及 daemon restart / checkpoint resume，必须补一条 **orphaned running -> recover -> resume** 回归：旧 `running` state 只有在 execution lease pid 仍活着时才算真运行；否则测试应断言 daemon 会先恢复到可 resume truth，而不是永久 `wait_running`
- live E2E 里“多 round”本身不是问题；只有当多轮之间没有真实任务推进（无新证据、无产物改进、无失败后修正、无验证增量）时，才可判为 `空转`
- 若使用 `scripts/run-real-provider-smoke.sh` 跑真实 provider transcript，默认会在退出时清掉 `session-test-*`；因此需要 postmortem 时，必须把检查重点放在 `provider-live-smoke-report.json`、`runtime/current/*`、tool receipts 与 transcript 产物文件，而不是等脚本结束后再去找已被清理的 session 目录
- live E2E 的异常归因默认按三层做：timeline/receipt 缺失、hidden control 泄漏、步骤顺序断裂 => `framework gap`；truth 完整但模型仍不合理选工具/不用 receipt/空转 => `model or prompt gap`；若两边证据都不够，则结论必须是 `insufficient truth`，先补 observability
- 若 live transcript 的交付物是文档/文件/补丁，验收时必须显式检查**实际内容**是否满足用户要求；不要因为 round 数偏多就先下负结论
- 真实 provider + 工具链 E2E 默认按“只读工具 -> 隔离 scratch 写入 -> 仓库真实写入”三级递进；不要一上来就拿复杂多 turn 读写混合场景判断工具链是否可用
- live transcript 若要求模型产出文件，输出路径必须按 `run_id` 隔离；固定路径会把上一次 run 的残留文件带入下一次归因，污染“是否真的执行工具/是否真的写入产物”的判断
- live transcript 的 `session_id/task_id` 也必须按 `__RUN_ID__` 渲染到 `session-test-* / task-test-*`；否则 cleanup 脚本无法统一回收，测试残留会长期挂在隔离 runtime home 里
- live transcript 若验收条件包含“repo/workspace 内目标文件真实存在且非空”，不要只看 `provider-live-smoke-report.json` 的 `artifact_paths`；还必须显式检查 transcript 指定目标路径，因为 repo 内写入产物不一定被回填到 receipt artifact 列表
- 因为 current history / tool receipt 默认按真值全量回注，live E2E 的 `exec_command` 必须显式约束输出体积（精确命令、`head`/`sed`/`rg` 限幅）；不要让模型自由跑 `find ... | xargs head` 这类大输出命令把后续 round prompt 撑爆
- 若复杂 live E2E 已能真实闭环但产物内容明显过泛，下一步先检查 `recent_provider_requests.json` 是否真的带入了足够的 executed-tool evidence（stdout/artifact refs/snippets），不要把“证据注入太粗”误判成 provider 或 tool dispatcher 失效
- 当目标是验证 current history/full tool evidence，测试断言必须直接检查 follow-up rendered input 是否出现 authoritative receipt/full stdout/full patch arguments；不要只断言“调过工具了”
- 若在补 tool feedback 闭环覆盖，不要只拿 `exec_command` 做样本；至少额外覆盖一条 `missing_runtime_target`、一条 `missing_context_capability`、一条 `missing_argument` 的 follow-up rendered-input 断言，证明模型确实看到了可纠正的失败语义
- Rust 测试文件若因 line-limit 被拆成父模块 + 子模块，不要默认 `cargo test <旧模块名>` 能覆盖父/子模块全部测试；至少补跑显式父模块测试名或直接跑 crate 级回归，避免 filter 只命中新子模块导致父模块漏验
- live provider/harness 禁止使用 180s/240s 这类短 wall-clock subprocess timeout 截断整条 run；最小要求是 provider waiting budget >= 15 分钟，并且 timeout 只绑定等待阶段，不绑定整条推理链墙钟
- 若 live run 失败后只剩 `start log + agent registry`，必须判定为“partial truth / diagnosability 缺口”，不能直接把问题归咎于 provider 或模型服从性
- 若真实 provider/live transcript 的首轮已经真实写出文件或跑出验证，但 `session_messages.json` 少一条 assistant、最后只报 `expected at least N session messages`，先检查 `closure_runtime` 的 `max_auto_tool_rounds` 是否把健康的多轮工具闭环截断；这类问题优先归为 framework round-cap gap，而不是空转或解析失败
- `build-mainline-receipts.py` 允许按 receipt family 指定不同 source session；当 history/context、tool-loop、control-boundary 真源不在同一 session 时，必须显式传 `--history-session-id/--tool-loop-session-id/--control-session-id`
- **标准业务入口（主 `~/.fin` / 主 QQ target / 主 system session）是受保护资源**：任何 provider/live/qqbot/startup 测试默认必须使用隔离 `runtime_home`、隔离 `user.toml`、隔离 session namespace；禁止让测试通过 `/new`、`/resume`、`/qqbot pair`、`last_run` 回绑去改写主业务入口。
- 若测试覆盖 `start` / `daemon` / `qqbot` / `system agent`，验收必须同时证明两件事：**测试入口在隔离 home 中闭环**，且**主业务入口未被污染**（主 `last_run`、主 `qqbot conversations`、主 peer state 仍指向业务 session，而不是 `test-*` / install / smoke session）。
- 启动链测试与业务链测试必须分开：测试可以创建专用 system session，但不得拿测试 session 替代主业务 system session；凡是会改写 canonical startup binding 的测试，一律只能在隔离 runtime home 执行。
- **测试业务 session 命名规则固定为 disposable namespace**：所有测试/烟雾/安装验证产生的 namespace 必须以 `test-` 开头，从而落成 `session-test-* / task-test-* / op-test-*`；`session-cli-demo` 只视为历史兼容残留，后续不允许再把无前缀 demo session 写入主 `~/.fin`。
- 每次测试结束后，测试业务 session 必须立刻清理；统一用 `python3 scripts/cleanup-test-sessions.py --runtime-home <path>`。对主 `~/.fin` 的残留排查/回收，也必须走同一个脚本，不允许人工零散删目录。

## 4) Minimal validation matrix

- L1：unit
- L2：contract
- L3：replay / harness smoke
- L4：cluster simulation
- L5：manual debug observe

## 5) Anti-patterns

- 跳过 replay 直接依赖人工排查
- 没有 raw event 就写 Web 调试结论
- 故障注入只看 UI，不看结构化事件
- 只跑 unit 就宣称模块可交付
- 用正常 `~/.fin` 家目录跑测试，导致 session / log / projection 污染
- 用短总超时把真实 provider/tool run 强行打死，然后把 timeout 误报成 runtime/tool 失败
- 还没证明“只读工具链 / scratch patch”可用，就直接拿仓库写入型复杂任务做唯一 E2E 判断
- 容忍 framework 通过自然语言 heuristic 或可见系统插话激活控制逻辑，再把它误记成“模型自治闭环”
