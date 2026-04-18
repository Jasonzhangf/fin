# 24 Session Render Truth and Reasoning Input Assembly

本文档收口两个紧邻问题：

1. WebUI / QQBot / 其他 channel 到底从哪里读，才能保证显示唯一真源
2. agent 单轮推理前，context / tools / history / prompts / project scope 到底如何装配

目标不是立刻展开全部实现，而是先冻结：

- render truth
- reasoning input truth
- framework vs model ownership
- 下一步实现顺序

---

## 1. 先冻结两层“真源”

`fin` 里必须区分两种真源，不允许混淆：

### 1.1 Runtime fact truth

运行事实真源仍然是：

```text
operation -> event
```

也就是说：

- 事件是“系统发生了什么”的事实真源
- projection 只是读优化
- channel 不允许自己发明运行事实

### 1.2 Session render truth

但**面向用户的会话渲染真源**，必须冻结为：

```text
session artifacts
```

也就是：

- `session_messages.json`
- `session_events` 对应的 session 级事件材料
- `recent_contexts.json`
- `recent_digests.json`
- 后续的 session-level channel materialized views

结论：

> 运行事实看 event；会话显示看 session artifacts。

这两层都是真源，但解决的是不同问题。

---

## 2. 为什么 channel 不能直接从 current projection 渲染

如果 WebUI / QQBot 直接从 runtime current projection 渲染，会出现几个问题：

1. **显示顺序不稳定**
   - projection 可能是最新聚合态，不保留完整会话顺序
2. **不同 channel 容易读到不同切片**
   - 某个 channel 从 projection 读
   - 另一个 channel 从 memory state 读
   - 最后用户看到的不是同一个会话
3. **重建困难**
   - session 无法从 channel 展示回溯
4. **容易出现“还没写盘就先显示”**
   - 前端或 bot 先收到临时数据
   - 后续 session 写盘再覆盖
   - 结果出现闪烁、重复、错序

所以必须冻结：

```text
先写 session
再渲染 channel
```

---

## 3. Channel render pipeline

所有用户可见 channel 统一遵循：

```text
operation accepted
  -> execution + event append
  -> session materializer updates session artifacts
  -> session revision advances
  -> channel consumers read session artifacts
  -> render
```

### 3.1 关键规则

1. WebUI 不直接消费未落盘内存消息
2. QQBot 不直接消费临时 assistant response buffer
3. channel 更新以 session revision / session write completion 为准
4. 任何 channel 看到的 message / digest / context 都应该能在 session 目录中找到对应文件证据

---

## 4. Session 目录下需要承担的 render artifacts

当前已经有：

- `conversation/messages.json`
- `events/stream.jsonl`
- `control/latest.json`
- `context/recent_contexts.json`
- `digests/recent_digests.json`

下一步建议冻结它们的职责：

### 4.1 `conversation/messages.json`

负责：

- 用户可见消息顺序
- turn/message 渲染真源
- message_id / operation_id / trace_id / closure_id 关联

用于：

- Web 对话区
- QQBot 会话文本渲染
- 后续 transcript 导出

### 4.2 `events/stream.jsonl`

负责：

- session 级事件事实流
- timeline / debug / replay 的可追踪依据

用于：

- detail modal timeline
- debug overlay
- replay / harness

### 4.3 `context/recent_contexts.json`

负责：

- 最近若干轮的 context snapshots
- 用户验证“本轮到底带了什么上下文”

用于：

- debug detail
- context rebuild 校验

### 4.4 `control/latest.json`

负责：

- 当前 closure 的 control feedback 真源
- continuity / topic shift / simple query 的最近判断
- execution note / digest 归档前的独立框架记录

用于：

- debug detail
- 后续 tentative session / routing state machine 接口对接

### 4.5 `digests/recent_digests.json`

负责：

- closure 级摘要
- continuity 重建的历史材料

用于：

- digest card
- future context rebuild

---

## 5. Session materializer 的 owning boundary

必须把“写 session artifacts”当成一个独立 framework slice，而不是 UI 顺手拼出来。

建议 owning boundary：

```text
runtime events / recording outputs
  -> session materializer
  -> session artifacts
```

它负责：

- 写 messages
- 写 context snapshots
- 写 digests
- 写 session event append
- bump session revision

它不负责：

- Web 样式
- QQBot 文案美化
- 模型提示词

结论：

> channel 只读 session materializer 的产物，不自己再拼第二份会话语义。

---

## 6. 下一步：Reasoning input assembly 的唯一真源

agent 单轮推理前，输入装配也必须冻结成一条框架-owned pipeline。

不是：

- UI 传一点
- provider 再补一点
- model 随机再靠历史猜一点

而应该是：

```text
session + task state + retrieved knowledge + tool catalog + project scope
  -> ContextViewBuilder
  -> ModelInputEnvelope
```

---

## 7. 当前 M1 已落地的 block（2026-04-17）

当前代码里，`demo / transcript / web-debug` 三条入口已经统一接入 `ContextViewBuilder`，并把以下 block 写入 `current_context.json` 与 `recent_contexts.json`：

- `control`
- `role_prompt`
- `tools`
- `history`
- `knowledge`
- `project`
- `current_input`

其中当前 M1 已补充的细化字段包括：

- `tools.disabled_tools`
- `tools.hard_guards`
- `project.project_root`
- `project.relative_selected_paths`
- `project.focus_summary`

同时，Web debug 的 `Context` 卡与 modal detail 已按这些 block 分区显示。

这意味着当前闭环已经变成：

```text
session artifacts + runtime binding
  -> ContextViewBuilder
  -> provider request(rendered_input)
  -> closure finished
  -> session materializer writes recent_contexts/messages/digests
  -> Web debug reads session artifacts
```

最小验证证据：

- `cargo test --workspace`
- `npx tsc -p rust/crates/debug-server/webui/tsconfig.json`
- 真实 `web-debug` 会话请求后，`~/.fin/runtime/current/current_context.json` 可直接看到 rich context blocks

---

## 7. 单轮推理输入的七个 block

下一步建议把推理输入拆成 7 个 block。

### A. `ControlBlock`

负责：

- 当前 task / topic / dispatch 状态
- continuity / topic-shift / simple-query 的框架判断输入
- 预算、限制、策略、是否要产 note/digest

### B. `RolePromptBlock`

负责：

- 当前 agent role 的系统提示词
- 当前 prompt 摘要
- 最近 prompt history
- prompt lineage（role baseline / project rules / runtime rules / current overrides）
- prompt modules（baseline / project / runtime / tooling / output contract）
- 行为边界
- 输出要求
- 当前项目级 rules 的已编译版本

### C. `ToolCatalogBlock`

负责：

- 当前允许的工具列表
- 每个工具的用途 / 何时使用 / 何时不要使用 / 输入 schema / 输出 schema / side effects / examples
- tool selection policy
- 本轮禁用工具或策略限制

### D. `HistoryBlock`

负责：

- 最近连续多轮的原始 user/assistant/tool 历史
- 当前 task 的最近连续窗口
- closure 之间的连续性保留

### E. `KnowledgeArtifactBlock`

负责：

- 被 retrieval 命中的 digest / artifact / note
- 历史高价值知识的压缩块

### F. `ProjectContextBlock`

负责：

- `primary_project`
- `active_projects`
- `projects`
- 当前 project / repo / cwd / selected paths
- 必要的 project scope 信息
- 当前任务相关文件区域或代码区域摘要

### G. `CurrentInputBlock`

负责：

- 用户本轮输入
- agent request / dispatch request
- 当前明确目标

---

## 8. 各 block 的装配顺序

建议冻结为：

```text
1. ControlBlock
2. RolePromptBlock
3. ToolCatalogBlock
4. HistoryBlock
5. KnowledgeArtifactBlock
6. ProjectContextBlock
7. CurrentInputBlock
```

理由：

- 先让模型知道“自己是谁、现在受什么控制”
- 再知道“能做什么工具动作”
- 再知道“刚才聊到哪”
- 再补长期知识
- 再补 project scope
- 最后给本轮新输入

这样更符合推理优先级。

---

## 9. Framework vs model ownership

### 框架必须负责

- context rebuild 触发
- history slice 选取
- retrieval 预筛选
- tool catalog 生成
- project scope 摘要输入
- session 写盘
- channel render feed

### 模型负责

- 基于这些 block 推理
- 输出 user_response / agent_messages / operation proposals
- 产 control feedback / note candidate / digest candidate

结论：

> 很多“基础状态反馈、记录、通信、构建”必须像潜意识一样由框架完成，不经过模型临时推导。

---

## 10. 下一步实现顺序

围绕刚才的冻结结论，下一步建议顺序如下：

### Step 1. `SessionMaterializer` 独立化

先冻结：

- 写哪些 session artifacts
- revision 怎么递增
- channel 从哪里读

### Step 2. `ContextViewBuilder` 独立化

先冻结：

- 从 session / task / artifacts / project scope 读什么
- 输出什么最小 `ContextView`

### Step 3. `ToolCatalogBuilder` 独立化

先冻结：

- tool list 来源
- role / task / policy 如何裁剪工具权限

### Step 4. `ModelInputAssembler` 独立化

把：

- prompt
- tools
- history
- artifacts
- project scope
- current input

组装成稳定输入 envelope。

### Step 5. `ModelOutputRecorder`

把模型输出稳定落成：

- session messages
- execution note
- digest
- event link

---

## 11. 当前非目标

以下细节暂不在本轮冻结：

- 最终 prompt 文本内容
- retrieval 排序公式
- project scope 的代码切片算法
- tool schema 的 wire shape
- channel-specific UI 微交互

这些都属于模块实现阶段再展开。
