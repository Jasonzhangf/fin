# 01 Manual Transcript + Web Debug Check

本文档给出当前 `fin` 多轮闭环 + Web 观察的最短人工测试流程。

## 1. 目标

验证三件事：

1. `transcript-demo` 能形成真实多轮闭环
2. `current_context.json` 与 `recent_contexts.json` 结构正确
3. Web debug 能把这些信息直观显示出来

## 2. 前提

- 已有可用的 `~/.fin/config/user.toml`
- 真实 provider 可用
- 当前 repo 已通过：
  - `cargo test --workspace`
  - `python3 scripts/check-code-line-limit.py`

## 3. 最短 CLI 闭环验证

在 repo 的 `rust/` 目录执行：

```bash
TMP_HOME=$(mktemp -d /tmp/fin-transcript-manual.XXXXXX)
FIN_RUNTIME_HOME_OVERRIDE="$TMP_HOME" \
cargo run -p fin-cli -- transcript-demo \
  ~/.fin/config/user.toml \
  ../fixtures/transcripts/banana-memory.json
```

预期：

- 终端输出 `transcript demo ok`
- 输出中包含：
  - `turns=3`
  - `session=session-test-real-transcript`
  - `task=task-test-real-transcript`

## 4. 重点检查文件

### 4.1 Last run

```bash
cat "$TMP_HOME/runtime/current/last_run.json"
```

重点看：

- `answer` 应该是 `BANANA-42`
- `session_recent_contexts_path` 存在
- `provider_user_agent` / `provider_header_names` 存在

### 4.2 Current context

```bash
cat "$TMP_HOME/runtime/current/current_context.json"
```

重点看：

- `input` 是第三轮输入
- `context.continuity_tail` 包含前两轮的 input/answer
- `context.summary` 存在
- `provider_path` / `role` / `protocol_version` 正确

### 4.3 Recent contexts

```bash
cat "$TMP_HOME/sessions/2026/04/session-test-real-transcript/context/recent_contexts.json"
```

重点看：

- 应有 3 条
- 第一条 `continuity_tail` 为空
- 第二条开始带上历史 continuity
- 第三条带上前两轮 continuity

## 5. Web debug 观察

启动 Web：

```bash
FIN_RUNTIME_HOME_OVERRIDE="$TMP_HOME" \
cargo run -p fin-cli -- web-debug ~/.fin/config/user.toml 4040
```

浏览器打开：

```text
http://127.0.0.1:4040
```

当前页面重点检查这些区域：

1. **Current Projection**
   - 有 session/task
   - 有 latest provider activity

2. **Provider Debug**
   - 有 `user_agent`
   - 有 `header_names`

3. **Current Context Snapshot**
   - 第三轮 input 正确
   - continuity_tail 正确

4. **Recent Context History**
   - 能看到 3 轮
   - 每轮 input / summary / continuity_tail 结构直观

5. **Last Run**
   - `answer` 是 `BANANA-42`

6. **Live Event Stream**
   - 可以看到 provider / progress / note / digest / operation completed 事件链

## 6. 如果你要给我反馈，最有价值的是这几类

1. **布局问题**
   - 哪个区域最难看
   - 哪个信息不够直观
   - 是否需要 tab 化 / 分区调整

2. **字段问题**
   - 哪些字段缺失
   - 哪些字段太噪音
   - 哪些字段需要更靠前显示

3. **闭环问题**
   - 你是否能一眼判断第三轮为什么能答对
   - 你是否能从页面看清 context 是如何构建的

4. **调试效率问题**
   - 出错时你最先想看什么
   - 当前页面有没有让你多点/多翻/多猜

## 7. 清理

测试结束后，如需删除临时目录：

```bash
rm -rf "$TMP_HOME"
```

