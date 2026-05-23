# Context Compression / Prompt Cache 审计与重构方案 — 2026-05-23

## 0. 审计目标

Jason 要求重新审视 `fin` 的 context/history build、context review、压缩算法与 prompt cache 命中关系，重点覆盖多轮绘画/多轮任务：

1. 上下文排序必须利于缓存命中：**完全不变 → 极慢变化 → 慢变化 → 快变化 → 最新工具调用/用户输入最后**。
2. 系统提示词、Developer Prompt、Skills、绘画/任务记录应尽可能保持不变，并以 append-only / diff 的方式逐渐增加。
3. `context history build` 与 `context review / compression` 需要合一审视，不应平时每轮无意义重建导致缓存失效。
4. 只有达到上下文压缩阈值才触发压缩；平时只追加当前 turn 所需的最小增量。
5. 对比 `~/code/codex` 的上下文压缩方式，决定采用传统 summary compact，还是与 `fin` 现有 digest/context 机制组合。

本文是**审计与待审批修改方案**，本轮不直接改实现。

---

## 1. 结论摘要

`fin` 当前不是“压缩阈值驱动的历史管理”，而是“每轮从近期 artifacts 重新装配一个大文本 prompt”：

- `web_debug_turns.rs` 每轮读取 `conversation/messages.json`、recent digests、recent reasoning views、recent tool records，并塞进 `SessionRequest`。
- `ContextViewBuilder` 每轮用这些 recent artifacts 重新构造 `MinimalContextView`。
- `ModelInputAssembler` 每轮把 `role_prompt/tools/project/peer/history/current request` 渲染成一个大字符串。
- `/compact` 目前只是本地命令式 rebuild snapshot，不是 Codex 那种 token-usage threshold 触发的 history rewrite。
- provider request/response schema 当前没有 token usage / prompt cache key 字段，无法做“达到阈值才 compact”的在线闭环。

因此当前方案对 prompt cache 不友好：稳定规则和慢变上下文会在每轮与动态 history 混合重排；`mandatory final answer format/example` 这类稳定块还被放在当前请求之后，破坏稳定前缀最大化。

推荐方案不是完全照搬传统大模型 summary，也不是保留当前每轮 rebuild，而是采用 **Codex-style token-threshold compaction + fin digest/context artifacts 组合**：

1. 建立 `ContextBudgetManager / ContextAssemblyPlanner` 作为唯一真源。
2. 正常 turn：只 append 当前用户输入、必要 tool tail、少量 settings diff；不重建完整 history。
3. 达到 token/context 阈值：触发 compact/review，把旧 history 物理替换为 `summary + selected recent user/task/artifacts + current tail`。
4. fin 的 Digest/Reasoning/Tool artifacts 继续作为 compact 输入与可观测证据，但不再每轮全量注入 prompt。

---

## 2. 当前 fin 链路证据

### 2.1 Web/debug 每轮读取近期历史

文件：`rust/crates/cli/src/web_debug_turns.rs`

关键证据：

- L200-L213：每轮读取 `session_messages_path`，格式化为 `role: content` 后放入 `recent_messages`。
- L230-L239：每轮读取 `recent_reasoning_views` 与 `recent_tool_records`。
- L292-L317：这些 recent artifacts 被完整传入 `run_session_request(...)`。

风险：普通 turn 也随着 messages/tools 增长改变 prompt 中段，不是阈值触发 compact。

### 2.2 ContextViewBuilder 每轮 rebuild MinimalContextView

文件：`rust/crates/runtime/src/context_view.rs`

关键证据：

- L16-L31：`ContextAssemblyInput` 接收 `recent_messages/recent_digests/recent_reasoning_views/recent_tool_records`。
- L37-L98：`build()` 每轮从 input 构建新的 `MinimalContextView`。
- L77：`role_prompt` 由 `build_role_prompt_block(worker, &recent_digests, &input)` 生成。
- L79-L84：`HistoryBlock` 直接包含 recent messages/digests/reasoning。
- L99-L103：recent tool records 被渲染到 `history.recent_tool_activity`。
- L104-L110：dynamic tool catalog 也读取 recent tool records。

风险：role prompt、history、tools 在同一 build 中混合动态输入。尤其 tool catalog 依赖 recent tool records，会让本应偏稳定的 tools 块随工具历史变化。

### 2.3 ModelInputAssembler 当前输出顺序不利于稳定前缀

文件：`rust/crates/runtime/src/model_input_assembler.rs`

当前顺序：

1. L14-L25 Agent prompt + Structured output contract
2. L26-L38 Context summary + Continuity tail
3. L39-L70 Tools / framework capabilities / disabled / policy
4. L72-L83 Project / Peer scope
5. L84-L103 Current interaction ledger / reasoning / tool execution history
6. L104-L114 Current request / Request envelope / attachments
7. L115-L123 Mandatory final answer format/example

风险点：

- `Mandatory final answer format/example` 是稳定规则，却在当前请求之后，无法贡献稳定前缀。
- `Context summary/continuity_tail` 在 tools/project 前，会随着 digest 变化影响后续大段稳定块缓存。
- history/tool activity 在 current request 前是合理的“快变尾部”，但当前会把 bounded recent window 全量搬入 prompt。

### 2.4 Persist 层是 bounded recent window，不是 compacted history

文件：`rust/crates/runtime/src/session_materializer.rs`

关键证据：

- L278-L293：context snapshots append 后 `trim_head(recent_context_limit)`。
- L296-L305：digests append 后 `trim_head(recent_digest_limit)`。
- L308-L323：reasoning views append 后 `trim_head(recent_reasoning_limit)`。
- L326-L341：tool records extend 后 `trim_head(recent_tool_record_limit)`。
- L388-L427：conversation messages append user/assistant 后 `trim_head(session_message_limit)`。

配置默认值在 `rust/crates/config/src/lib.rs`：

- L128-L139：`recent_context_limit=8`、`recent_digest_limit=8`、`recent_reasoning_limit=16`、`recent_tool_record_limit=32`、`session_message_limit=128`。

风险：这是“保留最近 N 条”，不是“达到阈值后生成 summary 并替换旧 history”。trim head 会丢旧信息，但不会形成对模型可读且稳定的压缩摘要基线。

### 2.5 `/compact` 只是 slash command rebuild，不是自动阈值 compact

文件：`rust/crates/cli/src/session_commands.rs`

关键证据：

- L522：`handle_compact` 入口。
- L588-L608：读取 recent artifacts 后调用 `ContextViewBuilder.build(...)`。
- L622-L648：写 `runtime/current/current_context.json` 与 `context/rebuild-index.json`。
- L649-L655：追加 notice message “/compact context rebuilt from recent session artifacts”。

风险：这不是在线 token usage 驱动的 history rewrite，也不会保证 compact 后请求形态变成稳定 summary + current tail。

### 2.6 Provider 层缺少 token usage 与 prompt cache key

文件：`rust/crates/provider/src/lib.rs`

关键证据：

- L47-L52：`ProviderRequest` 只有 `input/rendered_input/override_model`。
- L66-L74：`ProviderResponse` 只有 provider/model/output/response_id/stop/status。
- L87-L104：`prepare_request` 未包含 prompt cache key / conversation thread key。

风险：无法可靠实现“达到阈值才压缩”，也无法对 provider 的 prompt cache 行为进行归因。

### 2.7 多轮 auto-tool follow-up 也在变更 context 中段

文件：`rust/crates/runtime/src/round_context.rs`

关键证据：

- L20-L35：每个 follow-up round clone base context 后把 previous assistant/tool activity 写回 history。
- L40-L60：round > 1 时追加 summary/continuity_tail。
- L78-L84：每轮重建 dynamic tool catalog。

风险：单个 turn 内工具 follow-up 会让 summary/tools/history 重排；最新工具结果应尽量位于尾部，而不是影响前缀区块。

---

## 3. Codex 对比基线

### 3.1 Prompt cache key 固定到 thread

文件：`/Users/fanzhang/code/codex/codex-rs/core/src/client.rs`

- L713-L726：`prompt_cache_key = Some(self.state.thread_id.to_string())` 注入 Responses API request。

意义：同一 thread 使用稳定 cache key，前缀稳定性才有收益。

### 3.2 Codex 不是每轮 compact，而是 token-threshold compact

文件：`/Users/fanzhang/code/codex/codex-rs/core/src/session/turn.rs`

- L150-L159：turn 开始前运行 `run_pre_sampling_compact(...)`。
- L721-L740：读取 total token usage 与 model `auto_compact_token_limit()`。
- L740 后：只有 `total_usage_tokens >= auto_compact_limit` 才触发 auto compact。
- L450-L510 附近：sampling 后如果 `token_limit_reached && needs_follow_up`，才 mid-turn compact。

意义：普通 turn 不重写 history，只在阈值或模型返回 context limit 风险时 compact。

### 3.3 Codex 有 reference_context_item / diff baseline

文件：`/Users/fanzhang/code/codex/codex-rs/core/src/context_manager/history.rs`

- L35-L38：history oldest-first，rewrite 会 bump history_version。
- L40-L50：`reference_context_item` 是 settings diff baseline。
- L98-L106：`record_items` 按 oldest-to-newest append API-visible items。

文件：`/Users/fanzhang/code/codex/codex-rs/core/src/session/mod.rs`

- L2585-L2688：`build_initial_context()` 组装 full initial context。
- L2813-L2825：注释说明 reference snapshot 缺失时注入 full initial context，否则只 settings diff。
- L2826-L2856：`record_context_updates_and_set_reference_context_item()` 正常 turn 只追加必要 context items，并推进 baseline。

意义：稳定系统/Developer/Skills 不需要每轮重新渲染；平时只追加 diff。

### 3.4 Codex compact 是 history replacement

文件：`/Users/fanzhang/code/codex/codex-rs/core/src/compact.rs`

- L170-L185：compact task clone history，并把 compact input 作为 turn input。
- L194-L204：compact 请求用当前 history for_prompt + base instructions。
- L223-L230：compact 时若 context window exceeded，移除 oldest history item，明确保留后缀 recent messages。

测试：`/Users/fanzhang/code/codex/codex-rs/core/tests/suite/compact.rs`

- `summarize_context_three_requests...` 验证 compact 后请求只保留 summary + user history + new user message，并保持 baseline developer instructions。
- `multiple_auto_compact_per_task_runs_after_token_limit_hit` 验证 auto compact 是 token limit 触发，不是每轮发生。

意义：compact 是少数时刻的历史 rewrite，不是普通 turn 的重装配。

---

## 4. 差异表

| 维度 | fin 当前 | Codex 基线 | 风险 |
|---|---|---|---|
| Cache key | provider schema 未见 prompt cache key | thread_id 作为 prompt_cache_key | cache 行为不可控/不可观测 |
| 普通 turn | 每轮读取 recent artifacts 并 rebuild 大字符串 | append input + context diff | 稳定前缀容易失效 |
| Context baseline | 无 `reference_context_item` 等价物 | full context once，后续 settings diff | 系统/Developer/Skills 重复渲染 |
| 压缩触发 | `/compact` 手工 rebuild；无 token usage threshold | token usage / context limit 触发 | 平时和压缩边界混淆 |
| History 结构 | bounded recent JSON windows + trim_head | oldest-first append history，compact/rollback 才 rewrite | 丢旧信息但不形成 summary truth |
| Prompt 顺序 | summary/continuity/history 插在稳定块附近；稳定 final format 在请求后 | stable instructions / context items / history / user tail | 稳定前缀最大化不足 |
| Tool catalog | dynamic tools 读取 recent_tool_records | tools 作为 request tools/spec；工具输出是 history item | 工具历史影响工具说明区 |
| 多轮工具 | follow-up 更新 summary/continuity/tools | function call output 作为后缀进入下一 sampling | 最新工具结果污染慢变区 |

---

## 5. 目标上下文分层顺序

目标顺序应固定为：

1. **Immutable prefix（完全不变）**
   - stable core system / developer / output contract
   - protocol invariants
   - tool schema/static capabilities（不含 recent tool history）
   - mandatory answer format/example

2. **Rarely changing（极慢变化）**
   - role baseline
   - loaded skills index / skill manifest hash
   - project root / workspace identity / policy
   - drawing task static constraints（画风、尺寸、禁区等）

3. **Slow changing（慢变化）**
   - session baseline / task state diff
   - compacted summary
   - selected durable artifacts / drawing record index
   - settings diff from previous turn

4. **Growing append-only history（追加增长）**
   - user/assistant visible messages oldest → newest
   - selected recent tool results
   - drawing iterations / image references as stable records

5. **Volatile tail（快变尾部）**
   - current turn tool call outputs
   - current request envelope / attachments
   - latest user input

约束：前 1-2 层尽量不随 turn 改变；第 3 层只在 compact 或 settings change 改变；第 4 层 append-only；第 5 层每轮变化且必须在最后。

---

## 6. 待审批修改点

### A. 新增 ContextBudgetManager / ContextAssemblyPlanner（推荐）

职责：统一 context review、history build、compression trigger。

建议落点：

- `rust/crates/runtime/src/context_budget.rs`
- `rust/crates/runtime/src/context_assembly_plan.rs`
- contracts 增加 `ContextAssemblyPlanRecord` / `ContextBudgetSnapshot`

核心行为：

1. 读取 provider token usage / estimated tokens。
2. 判断是否需要：normal append、settings diff、pre-turn compact、mid-turn compact。
3. 产出可持久化 plan：sections、order、stable_hash、dynamic_tail_hash、trigger_reason。
4. 所有 prompt assembly 只能消费该 plan，禁止各入口自己拼 recent artifacts。

### B. Provider schema 增加 usage + prompt_cache_key

建议落点：

- `rust/crates/provider/src/lib.rs`
- provider HTTP adapters / tests
- runtime provider request/response records

新增字段：

- request: `prompt_cache_key: Option<String>`，默认 session/thread id。
- response: `usage: Option<TokenUsage>`，包含 prompt/completion/total/cached/reasoning（provider 不支持则显式 None）。

验收：无 token usage 时不能宣称阈值 compact 已闭环，只能走估算分支并记录 evidence weakness。

### C. 重排 ModelInputAssembler 稳定前缀

建议修改：

1. `Agent prompt / Structured output contract / Mandatory final answer format/example` 移到最前。
2. 静态 tool catalog 与动态 tool history 分离。
3. `Context summary/continuity_tail` 移到慢变层，不放在会影响 tools/static rules 的位置。
4. `Current request + attachments + latest user input` 保持最后。

### D. 引入 ContextBaseline / reference_context_item 等价物

建议：

- session 级保存 `context/baseline.json`。
- 第一个真实 turn 或 compact 后写 full baseline。
- 后续 turn 只计算 settings/project/skill/tool schema diff。
- baseline 缺失时才 full reinject。

### E. 改造 compact：从 rebuild snapshot 改为 history replacement

建议：

- `/compact` 与 auto compact 共用同一 compact engine。
- compact 输出写入：`conversation/compacted_history.json` 或等价 history replacement。
- 保留 selected recent user messages、drawing records、current tail。
- compact digest 成为慢变层 summary truth。
- 旧 `/compact` rebuild snapshot 若无用途，应物理移除或降级为 diagnostic，不允许继续冒充压缩实现。

### F. 多轮绘画记录专用策略

建议：

- 绘画记录不是每轮完整重述，而是 append-only `drawing_iteration_records`。
- 图片引用、seed、prompt、negative prompt、用户评价、修正目标形成稳定 record id。
- compact 时只压缩旧迭代的自然语言说明，保留关键 image/artifact refs 不丢失。
- 最新工具调用（生成图/编辑图/上传图/分析图）放在 tail。

---

## 7. 推荐实施顺序

1. **审计测试先行**：新增 request shape snapshot tests，固定当前问题与目标顺序。
2. **Provider usage/cache key contract**：先能观测 token/cache，再做阈值决策。
3. **Assembly plan 抽象**：让所有入口走唯一 plan。
4. **稳定前缀重排**：迁移 mandatory format/static tools/role prompt 到前缀。
5. **Baseline diff**：实现 full once + diff。
6. **Auto compact engine**：token threshold 触发 history replacement。
7. **绘画 records**：把 drawing iterations 接入 compact 输入与 stable record index。
8. **清理旧 `/compact` rebuild 语义**：删除或改名 diagnostic。

---

## 8. 验证矩阵

### Unit / snapshot

- `model_input_assembler` request shape snapshot：稳定块在前，current user input 最后。
- `context_assembly_plan` ordering test：immutable → rare → slow → append-only → volatile。
- `baseline diff` test：第二 turn 不重复 full stable context。
- `compact trigger` test：低于阈值不 compact，高于阈值 compact。
- `compact replacement` test：旧 history 被 summary replacement，不只是 trim_head。

### Runtime E2E

- 多轮普通聊天 3-5 turn：无 compact event，只 append tail/diff。
- 模拟高 token usage：触发 pre-turn compact。
- 模拟 tool follow-up + context limit：触发 mid-turn compact。
- 多轮绘画 fixture：旧图片记录保留 refs，最新工具调用在 tail。

### Evidence artifacts

- `runtime/current/current_context_assembly_plan.json`
- `sessions/.../context/baseline.json`
- `sessions/.../context/compaction-events.jsonl`
- provider request record 包含 `prompt_cache_key` 与 usage/cached token evidence。

---

## 9. 不建议方案

1. **继续每轮 rebuild recent artifacts**：不解决 cache 前缀稳定性。
2. **只调大 session_message_limit**：会更快撑满上下文，且 cache 更差。
3. **只做传统 summary，不保留 fin artifacts**：会丢绘画/image/tool refs 与可回放证据。
4. **只移动 CSS/渲染层**：context 真源在 runtime/provider，不在 UI。
5. **无 usage 就声称 auto compact**：没有阈值证据，不符合“先验证后结论”。

---

## 10. 推荐决策

采用 **Hybrid Codex-style compact**：

- Codex 负责启发：thread cache key、stable baseline、diff、token threshold compact、history replacement。
- fin 保留优势：digest/reasoning/tool artifacts、session materializer、runtime event evidence、drawing iteration records。
- 两者合并到一个 `ContextBudgetManager / ContextAssemblyPlanner`，作为 context review 与 compression 的唯一真源。
