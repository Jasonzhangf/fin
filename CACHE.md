# CACHE — Short-Term Memory

## Current Task
- Task: 收敛 provider debug + bounded context snapshot + Web 可观测基础
- Status: in_progress

## Recent Decisions
- [2026-04-17] build/回归/安装/提升要收敛到统一 build flow
- [2026-04-17] build version 与 Cargo semver 分层；未来预留 core/module version topology
- [2026-04-17] current context 只写 `runtime/current/current_context.json` 最新覆盖版
- [2026-04-17] session recent context 只写 `context/recent_contexts.json` bounded window（8）
- [2026-04-17] M1 不引入 detached debug daemon，避免 orphan process
- [2026-04-17] Web debug 刷新从前端轮询改为 SSE/watch 推送驱动；前端只在初始加载、手动刷新、发送消息后主动请求数据
- [2026-04-17] fin 当前 runtime / transcript / web-debug 生成的时间戳统一落本地时区格式（例如 `2026-04-17T21:00:15+08:00`）
- [2026-04-17] 聊天消息现在以 `operation_id` 为 focus 锚点；点击消息后在 focus pane 左侧看 request/response/事件链，右侧看 context/digest
- [2026-04-17] Web debug 信息架构改成 dashboard：顶部 selected request summary + provider/context/system/operation&event 四卡片；去掉旧 tab inspector 主模式
- [2026-04-17] 聊天视觉规则固定：user 右侧；assistant/其他 agent 左侧；focus 用蓝/绿高亮；点击消息不应强制滚回底部
- [2026-04-17] dashboard 再收敛为“两层”：首层四张等尺寸 digest summary cards；点击 card 后在下方 detail pane 看完整折叠结构，避免首页卡片过高且信息过散
- [2026-04-17] Web debug 当前右侧再收敛为瀑布流 digest cards + modal detail；ESC 可关闭 modal，UI 渲染消费优先改为 session artifacts（messages/events/context/digest）而不是 runtime current 快照
- [2026-04-17] `ContextViewBuilder` 已在 demo/transcript/web-debug 三条入口接入；当前 context blocks 为 `control / role_prompt / tools / history / knowledge / project / current_input`，并已在 Web Context 卡 + modal detail 可见
- [2026-04-17] context 第二轮增强已接入：`tools.disabled_tools / tools.hard_guards / project.project_root / project.relative_selected_paths / project.focus_summary`；Web digest 与 modal 可见
- [2026-04-17] context 第三轮结构增强已接入：`role_prompt.current_prompt_summary / prompt_history / prompt_lineage`，以及 `project.primary_project / active_projects / projects`；Web digest 与 modal 可见
- [2026-04-17] 为兼容旧 session artifacts，`current_prompt_summary` 兼容反序列化旧字段 `prompt_summary`，新增 prompt/project 字段均加了 `serde(default)`
- [2026-04-17] Prompt System Block 第一版已接入：`prompt_modules + output_contract`；Tool Prompt Spec 第一版已接入：`tool_selection_policy + richer framework tool spec(purpose/when_to_use/when_not_to_use/input/output/side_effects/examples)`
- [2026-04-17] Prompt System 设计真源已冻结为四层：`Stable Core + Role Modules + Session Overlay + Turn Envelope`；下一层 role family baseline / model overlays 已写入 `docs/architecture/26-role-prompt-family-and-model-overlays.md` 并同步进入本地 `fin-prompt-system` skill
- [2026-04-17] Stable Core Prompt 第一轮设计已落盘：结构参考 Hermes、写法参考 Codex、语义服从 fin 框架真相边界；设计真源写入 `docs/architecture/27-stable-core-prompt.md`，第一版 prompt blocks 文本写入 `docs/prompts/01-stable-core-prompt-v1.md`
- [2026-04-17] Role Prompt 文本层继续推进：四类 role baseline 第一版写入 `docs/prompts/02-role-baselines-v1.md`，GPT/Codex overlay 第一版写入 `docs/prompts/03-gpt-codex-overlay-v1.md`；下一步可直接进入 Rust assembler skeleton
- [2026-04-18] `ControlFeedback` 第一版已接入 closure 链路：新增 `control.feedback_recorded` event、`runtime/current/current_control_feedback.json`、`sessions/.../control/latest.json`，并同步进入 note/digest/projection/Web inspector。
- [2026-04-18] `ControlFeedback` 第二轮已升级为模型输出解析：`assistant_response_text` 与 provider raw output 分离；`model.output_parsed` 事件新增 `contract_detected` / `control_feedback_parsed`；错误 shape 的 control block 现在严格 fallback 到 `runtime_heuristic`。
- [2026-04-18] `ControlFeedback` 第三轮已接入 parse-failure salvage：`model.output_parsed` 新增 `control_feedback_salvaged`；真实 provider 在错 type JSON 下现在会走 `model_output_contract_masked`，保留白名单字段并丢弃未知字段。
- [2026-04-18] 真实 qwen provider 的 exact schema 命中已通过“`role_prompt.output_contract` + 推理末尾 mandatory final answer format”双重暴露提升；关键强化点是把禁止样式显式写出来：`0.98/1.0` 小数 confidence、字符串布尔值、extra keys。
- [2026-04-18] conversation UI 的下一层不再只叫 debug：已冻结为 Minimal / Rich / Full Trace 三档 richness；normal conversation 与 debug 共用 session render truth，Rich mode 后续要承载 reasoning summary、control block、tool semantic cards。
- [2026-04-18] conversation Rich step cards 继续收紧：Status 只显示紧凑 control chips；`continuity/topic_shift` 在聊天区合并为单个 `topic` 数字，`simple_query` 只保留单个 `simple` 数字，详情改为点击小模态框查看，避免占用主对话区空间。
- [2026-04-18] conversation 排版第四轮继续收紧：`reasoning` 必须单独一条显示；主 assistant 区以消息正文 + tool cards 为主；`control + provider` 合并为单条低强调 meta row，默认仅展示最重要摘要，细节只进点击模态框。
- [2026-04-18] 当前 M1 真相已再确认：尚不支持真正 pause/resume、pending input queue、interrupt、同 session 并行推理；这些先冻结为架构/contract，不提前做表面交互。
- [2026-04-18] “任务进行中询问状态”已冻结为特殊并行请求 `status_probe`：必须带特殊标记、non-interrupting、优先从最新 `ProgressBlock + ExecutionNote + ControlFeedback + ToolExecutionRecord` 组装回复，不打断主推理，也不生成新 digest。
- [2026-04-18] `status_probe` 最小 skeleton 已落地到 `web-debug` chat handler：支持显式 `input_kind=status_probe` 与 `/status` 前缀；当前走 side-path 直接读取最新 `progress/latest.json + notes/latest.json + control/latest.json + last_run.json` 组装回复，不调用 provider、不落新 closure。
- [2026-04-18] live `4040` 的 `POST /api/chat/send` 超时已确认不是 HTTP parser 问题：非法 body 会立即返回 400；卡住只发生在合法消息路径。重编译并精确重启当前 `web-debug` 进程后，`/status` 已恢复为正常 `status_probe` 200 响应，说明当时真问题是 live 进程仍在跑旧 binary / 旧 handler，而不是当前源码路径本身坏掉。
- [2026-04-18] provider 客户端现已固定为显式 `reqwest::blocking::Client::builder()`，统一加 connect/total timeout、reqwest 错误分类（timeout/connect/request/body/decode）与 3 次 bounded retry；失败消息必须带 stage/attempt/endpoint/source-chain，不能只剩 `error sending request`。
- [2026-04-18] Web trace 第二轮已接上 session truth：新增 `/api/recent_reasoning_views.json`、`/api/recent_tool_records.json`、`/api/recent_closures.json`；前端 focus turn 已按 `operation_id` 绑定 `ReasoningViewRecord` / `ToolExecutionRecord[]` / `ClosureTraceRecord`，modal 可看 provider raw output、rendered prompt、reasoning、tool activity、closure trace。
- [2026-04-18] Web trace 第三轮已做语义化渲染：`Provider Raw Output` 会拆 `fin_user_response` / `fin_control_feedback`；`Tool Activity` 改为语义卡片（purpose/target/input/output/side effects/artifacts）；`Closure Trace` 改为 user/assistant/rendered prompt/provider blocks 分区显示，不再只靠裸树渲染。
- [2026-04-18] normal conversation 的 Richness 第一版已接入主聊天区：header 提供 `Minimal / Rich / Full Trace` 切换；assistant turn 在 `Rich` 下显示 reasoning/control/tool 摘要，在 `Full Trace` 下追加 provider structured blocks 与 rendered prompt。仍然全部从 session artifacts + focus turn 真源渲染。

## Completed Evidence
- transcript-demo real provider closure passed; turn-3 returned BANANA-42 from prior context
- CLI build/versioning split completed; main.rs removed from line-limit whitelist
- real `build-dev` isolated run passed
- real manual staged -> `promote 0.1.0099` passed
- install-dev isolated run passed with auto build versions `0.1.0001 -> 0.1.0002`
- current/previous symlink update verified
- `cargo test --workspace` passed
- isolated real provider runtime-demo passed with `OK`
- projection / current_context / recent_contexts artifacts verified in isolated runtime home
- 真实 `web-debug` 4041 会话已验证：`current_context.json` 与 `recent_contexts.json` 带 rich context blocks，测试标记 `ctx-demo-2`
- 真实 `web-debug` 4041 会话再次验证：增强后的 tool/project block 已落入 `current_context.json`，测试标记 `ctx-demo-4`
- 真实 `web-debug` 4041 会话已验证：prompt history 与 projects 结构已落入 `current_context.json`，测试标记 `ctx-demo-5`
- 真实 `web-debug` 4041 会话已验证：prompt system block 与 tool prompt spec 已落入 `current_context.json`，测试标记 `ctx-demo-6`

- [2026-04-17] `~/.fin/skills` 已纳入 runtime home 布局；runtime 已支持扫描 `~/.fin/skills/*/SKILL.md` 并把 loaded global skills 投影到 role prompt lineage/module，供 system prompt 设计继续使用

## Next Steps
- 继续细化 `ContextViewBuilder` 的 block 内容：knowledge candidates 与 project scope file focus 先做真字段而不是占位摘要
- 再把 transcript/demo 闭环收进 installed smoke 或 harness 脚本
- 把 `ControlFeedback` 从 runtime heuristic 升级为真正的 model control block 解析与 contract
- 继续扩大真实 provider 验证样本，确认 `model_output_contract_v1` 命中是否稳定，而不只是单轮成功
- 把 Rich mode 所需的 `ReasoningViewRecord` / `ToolExecutionRecord` 从文档推进到 contracts/runtime skeleton
- 先设计 pending input / interrupt 最小状态机，再讨论并行推理
- 把 `status_probe` 从 web-debug side-path 推进到更通用的 runtime/message contract，而不是只留在 debug handler
- 若需要用户手工验证，直接在当前输入框发送 `/status`，应返回非中断状态摘要而非触发 provider 推理
- [2026-04-17] 若 Web debug 改了 include_str 静态 JS 或 context schema，必须重启正在监听的 `web-debug` 进程并刷新到最新 session turn；否则页面会继续显示旧 bundle/旧 artifact，表象像“context 组件没接上”。
- [2026-04-18] WebUI 端口只报 `4040`；不得再把历史测试端口 `4041` 混进当前状态汇报。
- [2026-04-18] 真实 `4040` 已再次手工验证：`POST /api/chat/send {"message":"/status"}` 返回 `response_kind=status_probe`、`events_count=0`、`messages_unchanged=true`、`digests_unchanged=true`；说明它确实走框架 side-path，而不是 provider / closure 路径。
- [2026-04-18] 推理完整化第一轮已落地：新增 `ToolExecutionRecord`、`ReasoningViewRecord`、`ClosureTraceRecord`，并在每次正常 closure 后落到 `sessions/.../tools/recent_tool_records.json`、`reasoning/recent_reasoning_views.json`、`closures/recent_closures.json`，同时更新 `runtime/current/current_reasoning_view.json` 与 `current_closure_trace.json`。
- [2026-04-18] `inference.started` 事件现在附带 `rendered_input`，`ClosureTraceRecord` 也持久化完整 assembled prompt / provider raw output / user input / assistant response，便于后续 Web 做完整 turn 级可视化。
- [2026-04-18] `web-debug` turn id 生成已修正为基于 `last_run.turn_index` / `operation_id` 后缀单调递增，不再用 bounded digest window 的长度推导；此前 `op-cli-demo-0009` 被重复复用的问题已修补。
- [2026-04-18] 推理上下文继续补全：`ContextViewBuilder` 与 `ModelInputAssembler` 现在会把 recent reasoning summaries 与 recent tool activity 编入下一轮 context，而不是只有 `recent_messages + recent_digests`。
- [2026-04-18] 真实 provider 再验证通过：隔离 `runtime-demo` 返回 `PROVIDER-RETRY-CHECK`；真实 `4040` 普通消息返回 `LIVE-4040-RETRY-CHECK`，并确认 `last_run.json` 已前进到 `operation_id=op-cli-demo-0002`、`turn_index=2`，reasoning/tool/closure artifacts 全部已写盘。
- [2026-04-18] 真实 `4040` 再次验证：普通消息返回 `LIVE-TRACE-CARDS-OK`；`op-cli-demo-0003` 已写入 `recent_reasoning_views` / `recent_tool_records` / `recent_closures`，且 `/app.js` 已包含 `recentReasoningViews/recentToolRecords/recentClosures` 新消费路径，说明 live binary 与前端 bundle 都已切到新版本。
- [2026-04-18] 真实 `4040` 再验证：`/inspector.js` 已包含 `Provider Raw Output / Reasoning View / Tool Activity / Closure Trace / renderInspectorSection`；普通消息最终返回 `TRACE-SEMANTIC-UI`，推进到 `digest-op-cli-demo-0004`。
- [2026-04-18] 真实 `4040` 再验证：`/chat.js` 已包含 richness 切换与 `renderRichPanel`；首页 HTML 已出现 `Minimal / Rich / Full Trace` 三按钮；普通消息返回 `TRACE-RICH-CONVO`，推进到 `digest-op-cli-demo-0005`。
- [2026-04-18] 当前这次“页面只停在 loading / not loaded”的真因已确认：不是数据接口坏掉，而是新增 `section_renderers.js` 后忘了把它暴露到 `web_app.rs`，导致浏览器模块加载 404，整个前端脚本链不执行。修复后 `section_renderers.js` 在 live `4040` 返回 200，普通消息继续闭合到 `digest-op-cli-demo-0006`。
- [2026-04-18] conversation Rich 模式继续收紧：主聊天区 assistant 卡片已增加 control 百分比指标条（continuity/topic shift/simple query）和更语义化的 tool snippet（purpose/target/input/output/side effects），减少纯文本堆叠。
