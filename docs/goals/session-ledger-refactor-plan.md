# Session Ledger Refactor Plan

## 1. 目标与验收标准

目标：把 fin 当前 session / ledger 管理升级为 ledger-first 架构：一个 ledger root 下一个唯一事实 ledger identity，基于 timeline 管理多 track；session 和 knowledge 都是 ledger track / projection，不再是分散 artifacts 的多真源。

验收标准：

1. 每个 session/project runtime 都有可定位的 `ledger/ledger.json` 与 `ledger/timeline/index.jsonl`。
2. 所有新 turn 写入同一个 ledger timeline，并同步写入分 track 文件。
3. `session.detail` 可追溯每个 turn 的完整累加过程。
4. `session.snapshot` 可从 detail/timeline 重建，包含用户输入、assistant summary、重要工具合集、artifact refs。
5. `knowledge` track 可追加 project-shared 知识，并带 evidence refs。
6. 现有 UI / QQBot / Android 仍可工作；旧 `conversation/messages.json`、recent/latest 文件只作为兼容 projection 存在。
7. 本地 query/curation 工具能按 session/task/agent/time/track 查询 ledger，校验 snapshot 与 knowledge evidence。
8. 所有关键状态推进有测试覆盖；无 fallback、无静默吞错、无第二事实源。

## 2. 范围与边界

### In Scope

- Rust runtime ledger 数据模型与 writer。
- `SessionMaterializer` 双写：现有 artifacts + 新 ledger tracks。
- ledger timeline seq 分配与 append-only 写入。
- session detail / snapshot track 生成。
- knowledge track 的最小 append API 与 evidence 校验。
- CLI 本地查询/整理命令或 tool function。
- 测试覆盖：单 turn、多 turn、多 track 顺序、snapshot rebuild、knowledge evidence、旧 projection 兼容。
- 文档更新：contract / architecture / migration notes。

### Out of Scope

- 立即移除所有旧 artifacts。
- UI 全面改读 ledger snapshot。
- 跨设备 ledger replication。
- 复杂 CRDT / conflict merge。
- cloud sync / remote database。
- 把 raw ledger 直接塞进 prompt。

## 3. 设计原则

1. Ledger 是唯一事实源；projection 可重建。
2. 一个 ledger root 只有一个 ledger identity；track 分文件，不分事实源。
3. timeline 统一排序：`seq` 单调递增，`ts` 只辅助展示。
4. record append-only；变更用 correction/supersedes，不原地改事实。
5. session 是 ledger 的 track/view；不是 ledger 外的 truth。
6. knowledge 是 project-shared track，必须带 evidence refs。
7. UI/channel 只消费 projection/snapshot；不解释 ledger 语义。
8. 本地工具负责查询、重建、整理、校验。
9. 禁止 fallback/降级/静默吞错；错误必须结构化暴露。

## 4. 目标目录结构

```text
<runtime_home>/ledgers/<ledger_id>/
  ledger.json
  timeline/index.jsonl
  tracks/
    session.detail.jsonl
    session.snapshot.jsonl
    events.jsonl
    turns.jsonl
    steps.jsonl
    tools.jsonl
    provider.jsonl
    control.jsonl
    knowledge.jsonl
  snapshots/current_session.json
  indexes/by-session.json
  indexes/by-task.json
  indexes/by-agent.json
  indexes/by-time.json
  indexes/knowledge-index.json
```

兼容期可在 session 目录下保留旧 artifacts：

```text
sessions/<year>/<month>/<session_id>/conversation/messages.json
sessions/<year>/<month>/<session_id>/turns/recent_turns.json
sessions/<year>/<month>/<session_id>/steps/recent_steps.json
...
```

但这些文件必须被定义为 projection / compatibility artifacts。

## 5. 技术方案

### 5.1 新增 contract types

候选文件：

- `rust/crates/contracts/src/records.rs`
- `rust/crates/contracts/src/lib.rs`

新增：

- `LedgerRecordEnvelope`
- `LedgerTrackKind`
- `LedgerRefs`
- `LedgerIdentityRecord`
- `SessionDetailRecord`
- `SessionSnapshotRecord`
- `KnowledgeLedgerRecord`
- `LedgerTimelineIndexRecord`

关键字段：

- `ledger_id`
- `seq`
- `ts`
- `track`
- `record_id`
- `record_kind`
- `refs`
- `payload`
- `caused_by`
- `supersedes`

### 5.2 新增 runtime ledger store

候选文件：

- `rust/crates/runtime/src/ledger_store.rs`
- `rust/crates/runtime/src/ledger_records.rs`
- `rust/crates/runtime/src/ledger_query.rs`

职责：

- 初始化 ledger root。
- 分配下一个 `seq`。
- 原子 append timeline + track record。
- 写 current snapshot / indexes。
- 校验 record refs。
- 查询 timeline by filters。

写入规则：

1. append track record 成功后 append timeline index。
2. 若任一写入失败，返回结构化 error，不吞。
3. 不做多路径 fallback；ledger root path 缺失就创建，格式损坏就报错。

### 5.3 SessionMaterializer 集成

候选文件：

- `rust/crates/runtime/src/session_materializer.rs`
- `rust/crates/runtime/src/session_record_journal.rs`
- `rust/crates/runtime/src/session_materializer_events.rs`

集成点：

- 非 hidden turn：从 `ClosureRun` 生成 ledger records。
- hidden framework source：写 control track，不污染 `session.snapshot`，必要时写 `session.detail` 的 framework/internal record。
- 保留旧 artifacts 写入，但标记为 projection。

映射：

- `run.events` -> `events` track。
- `run.turn_record` -> `turns` track。
- `run.step_records` -> `steps` track。
- `run.tool_records` -> `tools` track。
- provider request/response + round -> `provider` track。
- control feedback / routing action / checkpoint -> `control` track。
- full turn aggregation -> `session.detail` track。
- visible summary -> `session.snapshot` track。

### 5.4 Knowledge track

候选文件：

- `rust/crates/runtime/src/knowledge_ledger.rs`
- `rust/crates/cli/src/session_commands.rs` 或新 `ledger_commands.rs`

来源：

- digest summary 中被验证的 stable conclusion。
- model output learning/control block。
- tool execution verified result。
- 用户显式“记住”的 project scope fact。

最小实现：

- `append_knowledge_record(input)`。
- `knowledge_id` 稳定生成。
- `evidence_refs` 必填且必须能在 ledger timeline 或 session detail 中解析。
- `scope=project` 默认同 project 可检索。
- `supersedes` 只追加，不覆盖旧 record。

### 5.5 Snapshot rebuild

候选文件：

- `rust/crates/runtime/src/ledger_snapshot.rs`
- `rust/crates/cli/src/ledger_commands.rs`

能力：

- 从 `session.detail` + timeline rebuild `session.snapshot`。
- 对比当前 snapshot projection。
- 生成 drift report。
- 失败时指出缺失 record/ref，不自动补假数据。

### 5.6 CLI / local tool 查询

候选命令建议：

```bash
fin ledger query <user.toml> --session <id> --track session.detail
fin ledger query <user.toml> --task <id> --since <ts> --until <ts>
fin ledger snapshot rebuild <user.toml> --session <id>
fin ledger knowledge list <user.toml> --project <id>
fin ledger knowledge add <user.toml> --project <id> --statement <text> --evidence <ref>
fin ledger check <user.toml> --session <id>
```

实现可先做内部 Rust API + tests，再接 CLI。

## 6. 风险与规避

### 风险 1：双写导致事实分叉

规避：ledger 是唯一事实；旧 artifacts 明确为 projection。测试必须验证 projection 可由 ledger rebuild。

### 风险 2：seq 并发冲突

规避：M1 先使用单进程文件锁或 atomic seq 文件；并发写入失败必须显式报错。后续再做跨进程锁增强。

### 风险 3：knowledge 无 evidence 变成记忆污染

规避：knowledge append API 强制 evidence refs；缺 evidence 直接拒绝。

### 风险 4：UI 误读 track 语义

规避：UI 只读 `snapshots/current_session.json` 或现有 projection，禁止直接解释 raw tracks。

### 风险 5：迁移范围过大

规避：分阶段推进，先 writer + query + tests，再替换读路径。

## 7. 测试计划

### Unit tests

- ledger init creates identity and dirs。
- seq monotonic across multiple track appends。
- append writes both track file and timeline index。
- record correction uses `supersedes` not overwrite。
- invalid ledger record rejects empty track / refs / record_id。
- knowledge append rejects missing evidence。
- knowledge append accepts valid evidence refs。
- snapshot rebuild from session.detail matches stored snapshot。

### Integration tests

- single turn materialization writes all required tracks。
- multi-turn materialization preserves timeline order across events/steps/tools/session tracks。
- hidden framework turn writes control/internal track but not user-visible snapshot。
- legacy `conversation/messages.json` matches ledger-derived session snapshot projection。
- QQBot delivery still reads compatible projection and sends new assistant/system messages。
- status probe does not mutate ledger tracks。

### Regression matrix

- `cargo test -p fin-contracts -p fin-runtime -p fin-cli`
- `cargo test -p fin-debug-server`
- targeted ledger tests with `-- --nocapture`
- `git diff --check`
- Android build only if Java/Gradle runtime available。

## 8. 实施步骤

### Phase 1：contract + store skeleton

1. Add contract records。
2. Add `LedgerStore` with init, append, query basics。
3. Unit-test seq/timeline/track append。

### Phase 2：session materializer dual-write

1. Convert `ClosureRun` into ledger track records。
2. Write session.detail/session.snapshot/events/turns/steps/tools/provider/control tracks。
3. Keep old artifacts as projection。
4. Add integration tests。

### Phase 3：knowledge track

1. Add knowledge append API。
2. Add evidence ref validation。
3. Add project-shared query index。
4. Add tests for summary/learning/control-derived knowledge candidates。

### Phase 4：local query / curation CLI

1. Add `fin ledger query`。
2. Add `fin ledger snapshot rebuild`。
3. Add `fin ledger knowledge list/add`。
4. Add `fin ledger check`。

### Phase 5：read-path migration

1. Status probe reads ledger-derived snapshot first。
2. QQBot delivery reads session snapshot projection。
3. WebUI consumes projection only。
4. Keep old files as compatibility exports until next cleanup milestone。

## 9. 文件清单

Likely new files:

- `rust/crates/runtime/src/ledger_store.rs`
- `rust/crates/runtime/src/ledger_records.rs`
- `rust/crates/runtime/src/ledger_query.rs`
- `rust/crates/runtime/src/ledger_snapshot.rs`
- `rust/crates/runtime/src/knowledge_ledger.rs`
- `rust/crates/runtime/src/ledger_store_tests.rs`
- `rust/crates/cli/src/ledger_commands.rs`

Likely modified files:

- `rust/crates/contracts/src/records.rs`
- `rust/crates/contracts/src/lib.rs`
- `rust/crates/runtime/src/lib.rs`
- `rust/crates/runtime/src/session_materializer.rs`
- `rust/crates/runtime/src/session_record_journal.rs`
- `rust/crates/cli/src/command.rs`
- `rust/crates/cli/src/cli.rs`
- `rust/crates/cli/src/status_probe.rs`
- `rust/crates/cli/src/channel_peer_conversations.rs`
- `docs/contracts/session-ledger-contract.md`
- `docs/architecture/29-multi-turn-history-model.md`

## 10. 完成定义

任务完成时必须满足：

1. 新 ledger root + timeline + tracks 可由测试生成并查询。
2. 新 turn materialization 双写 ledger 与旧 projection。
3. session snapshot 可从 session detail 重建。
4. knowledge track 支持带 evidence 的追加和查询。
5. 本地 query/check 工具可用。
6. 核心测试通过，并说明 Android 是否因环境阻塞。
7. 文档说明当前旧 artifacts 的 projection/compatibility 地位。
8. Summary 中明确唯一性：为什么 ledger store/session materializer 是唯一正确修改点。
