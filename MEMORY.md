# MEMORY — Long-Term Memory

## Project Overview
- fin 的执行真源在 Rust runtime；Web 只做观察与调试。
- 基础事实模型固定为 `operation -> event -> projection/debug view`。
- 当前 M1 优先级：最小可闭合推理 + 最小可观测 Web debug，而不是复杂自治。

## Key Decisions
- [2026-04-17] Provider debug 通过结构化 event 暴露 sanitized `user_agent` 与 `request_headers`；Web/Projection 只能消费，不复制 provider 语义。
- [2026-04-17] Context snapshot 采用 bounded write：`runtime/current/current_context.json` 只保留最新，session 内 `context/recent_contexts.json` 只保留最近窗口（当前 8 条）。禁止每轮散写无界快照。
- [2026-04-17] 生命周期控制优先：M1 `web-debug` 保持前台阻塞命令，不引入 detached daemon / orphan process。
- [2026-04-17] Web debug 不允许前端轮询刷屏；当前真规则为 SSE/watch 推送驱动 + 按需 fetch artifacts。
- [2026-04-17] 当前 `fin` 的 demo/transcript/web-debug 时间统一使用本地时区时间戳，不再使用固定 UTC `Z` 字符串。
- [2026-04-17] 消息级 debug 以 `operation_id` 为唯一 focus 锚点；Web 在消息选中后左侧展示该轮 request/response/事件链，右侧展示 context snapshot + digest。
- [2026-04-17] Web debug 的主信息架构已切换为 dashboard：顶部 `Selected Request` 摘要 + `Provider / Context / System / Operation & Event` 四卡片，不再以旧 tab inspector 作为主入口。
- [2026-04-17] dashboard 首页只放 digest 级 summary cards，并要求卡片尺寸一致；完整可折叠结构放到下方 detail pane，避免首页直接塞满细节。
- [2026-04-17] Web debug 右侧当前形态为瀑布流 digest cards + modal detail；modal 支持 ESC 关闭，且 UI 主要内容消费优先来自 session artifacts（session messages / session events / recent contexts / recent digests）。
- [2026-04-17] channel 渲染真源冻结为 session artifacts，而不是 runtime current projection：先写 session，再让 WebUI / QQBot / 其他 channel 渲染。
- [2026-04-17] 下一步 agent 推理模块要按 block 装配：Control / RolePrompt / ToolCatalog / History / KnowledgeArtifact / ProjectContext / CurrentInput；这些由框架先构建，再交给模型推理。
- [2026-04-17] 聊天 UI 规则固定：只有 user 在右侧；assistant 与其他 agent 类消息都在左侧；focus 高亮使用蓝/绿而不是红色警示风格。
- [2026-04-17] 真实 provider 回归必须使用隔离 test runtime home 与 test session namespace，不能污染正常 `~/.fin`。
- [2026-04-17] 正式 build/安装/提升必须走统一 build flow；自动回归与自动 build version bump 应挂在统一入口，而不是裸 `cargo build`。
- [2026-04-17] 产品 build version 与 Cargo crate semver 分层；未来预留 core version + module version map 的模块化升级路径。

- [2026-04-17] install-dev 已实现自动 build version：隔离验证中已确认 `0.1.0001 -> 0.1.0002` 自动递增，并维护 `current/previous`。
- [2026-04-17] 网络型 provider 回归允许显式 bounded retry（当前 3 次），但 retry 过程必须写日志，不能静默 fallback。
- [2026-04-18] provider 请求路径现在固定为显式 reqwest blocking client builder（带 connect/total timeout）；若发送失败，错误信息必须至少带 `stage + attempt + endpoint + timeout/connect/request/body/decode flags + source chain`，避免 live 问题只剩模糊的 `error sending request`。

- [2026-04-17] CLI 已补齐 `build-dev / promote / rollback` 入口；`install-dev` 暂保留为兼容别名。
- [2026-04-17] `transcript-demo` 已形成真实多轮闭环：recent digest/context 不仅落盘，还会编入 provider 实际请求；Web 可通过 `recent_contexts.json` 观察每轮 context 结构。
- [2026-04-17] `ContextViewBuilder` 已实际接入 `demo / transcript / web-debug`，当前 rich context blocks 为 `control / role_prompt / tools / history / knowledge / project / current_input`；它们会进入 provider rendered input，并落到 `current_context.json + recent_contexts.json`，供 Web debug 同步观察。
- [2026-04-17] rich context 第二轮增强已落地：`tools.disabled_tools / tools.hard_guards / project.project_root / project.relative_selected_paths / project.focus_summary` 已作为结构化字段进入 session artifacts 和 Web debug。
- [2026-04-17] rich context 第三轮结构增强已落地：`role_prompt.current_prompt_summary / prompt_history / prompt_lineage` 与 `project.primary_project / active_projects / projects` 已进入 session artifacts 和 Web debug。
- [2026-04-17] Prompt System Block 第一版已落地：`role_prompt.prompt_modules / output_contract` 已进入 session artifacts 和 Web debug。
- [2026-04-17] Tool Prompt Spec 第一版已落地：`tools.tool_selection_policy` 与 richer framework tool spec（`purpose / when_to_use / when_not_to_use / input_schema_summary / output_schema_summary / side_effects / example_uses`）已进入 session artifacts 和 Web debug。
- [2026-04-17] schema 演进需要兼容旧 session artifacts；新增字段默认必须 `serde(default)`，重命名字段需保留 alias（例如 `current_prompt_summary <- prompt_summary`），否则真实 web-debug 会在旧数据上炸掉。
- [2026-04-17] Prompt System 设计真源已冻结为四层：`Stable Core Prompt + Role Prompt Modules + Session/Task Overlay + Turn Context Envelope`；role 内容与 model overlays 的真源写入 `docs/architecture/26-role-prompt-family-and-model-overlays.md`，本地执行适配写入 `skills/fin-prompt-system/SKILL.md`。
- [2026-04-17] Stable Core Prompt 第一轮设计已冻结：结构参考 Hermes，文本写法参考 Codex，但真相语义完全服从 fin 的 session artifact / event / framework ownership 架构；对应文档为 `docs/architecture/27-stable-core-prompt.md` 与 `docs/prompts/01-stable-core-prompt-v1.md`。
- [2026-04-17] Role Prompt 文本第一轮已冻结：system / project / worker / reviewer baseline 文本在 `docs/prompts/02-role-baselines-v1.md`，GPT/Codex overlay 文本在 `docs/prompts/03-gpt-codex-overlay-v1.md`；当前 prompt 真源已覆盖 stable core、role baselines、GPT/Codex overlay 三层。

- [2026-04-17] `~/.fin/skills` 现在是全局 skills 运行时真源目录；runtime 会扫描 `~/.fin/skills/*/SKILL.md`，并把已加载 skills 注入 role prompt 结构化投影，而不是只靠文件存在但不被系统消费。
- [2026-04-18] 最小推理闭环新增 `ControlFeedback`：runtime 在 closure 结束后发 `control.feedback_recorded` 事件，并将同一份 feedback 写入 `runtime/current/current_control_feedback.json`、`sessions/.../control/latest.json`、`ExecutionNote.control_feedback`、`DigestRecord.control_feedback`；Web inspector 只消费这些真源，不再自己猜 continuity/topic/simple-query。
- [2026-04-18] `ControlFeedback` 解析器已升级为“模型输出优先 + runtime fallback”：支持从 `<fin_user_response>` 与 `<fin_control_feedback>` 解析；若 control block JSON 不含 fin 认可字段，则必须拒绝并回退到 `runtime_heuristic`，不能把错 shape 的 JSON 误判为有效 control feedback。
- [2026-04-18] `ControlFeedback` 解析失败路径已升级为 mask 白名单提取：若真实 provider 输出包含 fin 认可 key 但 value type 不合法（例如 `1.0` / `"0.18"` / `"true"`），runtime 会只提取白名单字段并做受控 coercion，产出 `origin=model_output_contract_masked`；未知字段必须丢弃，不能污染真源。
- [2026-04-18] 若真实 provider 在 structured output 上持续“语义正确但 schema 不精确”，prompt 侧优先采用“双重暴露”：既在 `role_prompt.output_contract` 放规则，也在 `ModelInputAssembler` 末尾重复注入 mandatory final answer format + exact example + forbidden shapes（`0.98/1.0`、字符串布尔值、extra keys）；不要先放宽 runtime truth 判定。
- [2026-04-18] conversation 前台与 debug 后台的关系已再次冻结：它们共享同一份 session render truth，只是 richness level 不同；后续 UI 应分为 Minimal / Rich / Full Trace，Rich mode 不是 debug 特例，而是正常会话增强层。
- [2026-04-18] “渲染 reasoning” 在 fin 中只允许通过结构化 reasoning view 暴露，不等于暴露原始 CoT；当前近似真源仍是 `ControlFeedback + ExecutionNote + DigestRecord`，后续再独立为 `ReasoningViewRecord`。
- [2026-04-18] 工具渲染当前只有 `ToolSnapshot { tool_name, status, summary }`，不足以支持有意义 UI；后续需升级为独立 `ToolExecutionRecord`，至少表达用途、操作对象、结果、side effect。
- [2026-04-18] 当前 M1 尚不支持真正 pause/resume、pending input queue、interrupt、同 session 并行推理；并行能力应先经历 `pending-first -> cross-task/cross-worker -> cross-process/cross-network` 三阶段，而不是直接在当前 closure loop 上强开并发。
- [2026-04-18] 有一个必须保留的并行特例：用户在任务进行中询问“当前状态”属于 `status_probe` 类请求。它必须显式标记为 non-interrupting parallel inquiry，走 side path，从最近 `ProgressBlock + ExecutionNote + ControlFeedback + tool state` 组装状态回复；不能打断主推理，也不能生成新的 digest closure。
- [2026-04-18] `status_probe` 的第一版实现先落在 `web-debug` handler，而不是 runtime 主闭环：支持显式 `input_kind=status_probe` 与 `/status` 前缀，当前直接读取 `last_run + progress/latest + notes/latest + control/latest` 组装回复，并返回 `response_kind=status_probe`、`freshness`、`progress/note/control_feedback`；对应测试已验证不会新增 closure/digest，也不会改写 session messages。
- [2026-04-18] 若 live `web-debug` 出现“源码测试全绿但 4040 真实行为仍像旧逻辑”的现象，先判定 stale process / stale binary，而不是先继续猜 HTTP 层：本次 `POST /api/chat/send` 超时就是如此。判据是非法 body 仍能即时返回 400，说明 HTTP 解析活着；随后重编译并精确重启当前 `4040` 进程后，`/status` 立即恢复正常。
- [2026-04-18] `status_probe` 的真实 hand-check 验收标准已冻结：`POST /api/chat/send {"message":"/status"}` 必须返回 `response_kind=status_probe`、`events_count=0`，且 session `messages.json` 与 `recent_digests.json` hash 不变，证明没有走 provider / closure / digest 写入路径。
- [2026-04-18] 推理历史的“完整 turn 真源”不能只靠 `messages + digests + progress` 拼出来；最少还必须有：1) assembled prompt / rendered_input，2) provider raw output，3) 独立工具执行记录，4) 独立 reasoning 摘要记录。当前已新增 `ToolExecutionRecord`、`ReasoningViewRecord`、`ClosureTraceRecord` 并落盘到 session artifacts。
- [2026-04-18] `ControlFeedback` 当前的自动 hook 范围已冻结为：解析/掩码提取 -> fallback 合并 -> 写 `control/latest.json` + `control.feedback_recorded` event + 注入 note/digest/status_probe。它还没有升级到“自动 session 切换 / task creation / interrupt / pending input queue”这类真正控制面动作。
- [2026-04-18] `web-debug` turn id 不能基于 bounded recent digest 数量推导，否则窗口滚动后会重复旧 `operation_id`；单调 turn counter 现在以 `last_run.turn_index` / `operation_id` suffix 作为真源并写回 `last_run.json`。
- [2026-04-18] 真实 `4040` provider 普通消息已再次验证：返回 `LIVE-4040-RETRY-CHECK`，并确认当前 session 从 `op-cli-demo-0001` 单调推进到 `op-cli-demo-0002` / `turn_index=2`；对应 reasoning/tool/closure artifacts 与 `rendered_input` 均已落盘。
- [2026-04-18] Web trace 当前的真源接法已冻结：右侧 detail/modal 不直接拼 runtime current 快照，而是按 `operation_id` 从 session artifacts 绑定 `context + digest + reasoning view + tool records + closure trace`；若 UI 改了但 `app.js` 未出现新字段消费路径，先判主 binary / bundle 未刷新。
- [2026-04-18] Web trace 的可读性规则继续冻结：对 `Tool Activity`、`Closure Trace`、`Provider Raw Output` 优先做语义化分区渲染（purpose/target/input/output/side effects、rendered prompt、provider structured blocks），而不是直接丢裸 JSON 树；树形展开只作为保底通用 fallback。
- [2026-04-18] normal conversation 与 debug 共用 session render truth 的规则继续落地：聊天主界面可以有 `Minimal / Rich / Full Trace` 三档 richness，但它只改变显示丰富度，不引入第二套数据链；reasoning/control/tool/provider trace 仍必须从 `FocusTurn(operation_id -> session artifacts)` 派生。
- [2026-04-18] debug-server 新增前端模块文件时，除了让 TS 编译产出，还必须同步在 `rust/crates/debug-server/src/web_app.rs` 暴露对应 `/*.js` 路由；否则浏览器会因模块 404 直接停在初始 loading 画面，看起来像“页面没显示”，但真因是前端模块链断裂。
- [2026-04-18] conversation Rich 模式的显示原则继续收敛：优先用紧凑的 control meters + tool semantic snippets 展示关键状态，把 reasoning/control/tool 压成可扫读块；详细 provider/control/rendered prompt 仍留给 `Full Trace` 展开。
- [2026-04-18] 聊天区 step-card 的 Status 规则再收紧：主对话区只展示极简 control 摘要，`continuity/topic_shift` 视为同一维度并在 UI 合并成单个 `topic %`，`simple_query` 只显示单个 `simple %`；详细 control block 只通过点击卡片后的小模态框查看。
- [2026-04-18] 会话区排版继续冻结：`reasoning` 作为独立单行 summary 显示；assistant 正常视觉重心放在正文与工具卡；provider/control 不再拆成显眼卡片，而是合并成一条低对比 `meta row`，只露最重要摘要，点击后再进小模态框看完整细节。

## Patterns & Learnings
- 大的 CLI/核心入口在行为稳定后要尽快拆回模块边界；门禁白名单只能临时存在，验证通过后应回收。
- Provider 请求调试信息进入 event 时必须脱敏；允许暴露 header 名称和安全值，禁止泄漏 API key/token/cookie。
- 需要直观 debug 时，优先把字段做成结构化 projection + current artifact，而不是靠 grep 巨量日志。
- 多轮闭环调试优先看 `current_context.json + recent_contexts.json + last_run.json` 的组合，而不是猜模型是否记住历史。
- snapshot 设计先考虑 CPU / 内存 / IO 边界，再决定写入粒度；latest overwrite + bounded recent window 是当前默认模式。
- Web debug 的实例标识必须单一且稳定；当前项目只允许对外汇报/验证/说明 `4040`，历史测试端口不得混入当前状态沟通，否则会破坏实例、bundle 与截图证据的一致性。
- 若 prompt/output contract 或 `include_str!` 打包的 live binary 有变更，真实验证前必须显式重启当前 `4040` 的 `web-debug` 进程，否则会继续命中旧 prompt/旧 bundle。

## Technical Context
- Projection 当前已包含：`latest_provider_activity` / `latest_provider_user_agent` / `latest_provider_header_names`。
- Runtime 当前已落盘：`current_context.json`、`recent_contexts.json`、`last_run.json`、`current_projection.json`、`latest_events.jsonl`。
- Web debug 当前展示：Selected Request Summary + Provider / Context / System / Operation & Event dashboard。
- 当前 Web Context 卡与 modal detail 已按 context blocks 分区；首页显示 digest，细节通过 modal 看 structured tree。

## Verified Evidence
- [2026-04-17] `cargo test --workspace` 通过。
- [2026-04-17] 使用真实 anthropic-compatible provider 的隔离 `runtime-demo` 返回 `OK`，并验证了新的 context/projection/provider debug artifacts。
- [2026-04-17] 使用真实 `web-debug` 会话在 `http://127.0.0.1:4041` 发送测试请求后，`~/.fin/runtime/current/current_context.json` 已验证包含 rich context blocks，测试标记 `ctx-demo-2`。
- [2026-04-17] 再次用真实 `web-debug` 会话验证，`project_root` 已修正为 git repo 根 `/Users/fanzhang/Documents/github/fin`，增强字段测试标记 `ctx-demo-4`。
- [2026-04-17] 再次用真实 `web-debug` 会话验证，prompt history 与 projects 结构已落盘并可由 `http://127.0.0.1:4041` 观察，测试标记 `ctx-demo-5`。
- [2026-04-17] 再次用真实 `web-debug` 会话验证，prompt system block 与 tool prompt spec 已落盘并可由 `http://127.0.0.1:4041` 观察，测试标记 `ctx-demo-6`。
- [2026-04-17] Prompt/Web 可见性问题先查三段真源：`current_context.json` 是否有字段、`/inspector.js` 是否带新 bundle、当前 `web-debug` 旧进程是否已重启；静态资源走 `include_str!` 时，不重启 server 就不会切到新 UI。
- [2026-04-18] Web debug 端口纪律收紧：当前有效端口只允许引用 `4040`；`4041` 视为历史测试端口，后续汇报/验证/截图说明一律不得混用，否则会误导用户判断实例与 bundle 是否一致。
- [2026-04-18] 真实 `web-debug` 4040 provider 回归再次验证：第一次请求网络失败，按 bounded retry 第二次成功；provider 返回 exact 两段式 structured output，`current_control_feedback.json` 与 `model.output_parsed` 事件显示 `origin=model_output_contract_v1`、`control_feedback_salvaged=false`。
- [2026-04-18] 真实 `web-debug` 4040 再次手工验证：`POST /api/chat/send {"message":"/status"}` 返回 `response_kind=status_probe`、`freshness=recent`、`events_count=0`，并且 `messages.json` 与 `recent_digests.json` hash 保持不变，确认它是框架 side-path，不会新增 closure/digest。
- [2026-04-18] 使用本地静态 provider 的真实 probe 已验证新增 artifacts：`last_run.json` 现在包含 `operation_id/trace_id/turn_index/current_reasoning_view_path/current_closure_trace_path/session_recent_reasoning_path/session_recent_tool_records_path/session_recent_closures_path`；对应 session 下已实际生成 `tools/recent_tool_records.json`、`reasoning/recent_reasoning_views.json`、`closures/recent_closures.json`，且 `ClosureTraceRecord` 持有完整 `rendered_input`。
- [2026-04-18] 仅有“把 reasoning/tool records 落盘”还不够；若下一轮 context 仍只喂 `recent_messages + recent_digests`，模型就吃不到推理摘要与工具历史。当前已补到 `ContextViewBuilder` / `ModelInputAssembler`：recent reasoning summaries 与 recent tool activity 会进入下一轮 assembled prompt。
