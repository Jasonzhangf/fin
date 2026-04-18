# 29 Multi-turn History Model

本文档冻结 `fin` 的完整多轮历史记录方案。

目标：

- 吸收 `codex`、`finger`、`hermes-agent` 的有效做法
- 冻结 `session / history / digest / context rebuild` 的职责分层
- 明确多 worker / 同 workdir 下的共享边界
- 给后续 `TurnRecord / StepLedger / multi-step inference loop` 提供真源

非目标：

- 当前不展开 wire shape
- 当前不展开具体 crate/file 落点
- 当前不直接进入 pause/resume / parallel inference 实现细节

---

## 1. 参考结论

## 1.1 Finger 的可取部分

`finger` 提供了三个关键方向：

1. `session` 是 durable substrate，而不是 prompt 本身
2. raw ledger 与 compact memory 必须分层
3. 多个 worker/runtime 可以保持各自原始 ledger，再通过共享知识层发生协作

因此，`fin` 不应把“原始历史、压缩记忆、当前上下文”混成同一份 history 文件。

## 1.2 Codex 的可取部分

`codex` 明确区分：

- persistent history：跨 session、轻量、兼容优先
- local history：当前 session、rich working state

`fin` 应继承这条原则：

- durable cross-session knowledge 轻量且稳定
- current session working state 可以 richer，但不能直接提升为长期共享知识

## 1.3 Hermes-agent 的可取部分

`hermes-agent` 在 context compression 上最有价值的点是：

1. 压缩结果必须结构化
2. summary 应支持 iterative update，而不是每次重做
3. 必须保护最近 tail
4. 不得破坏 tool-call / tool-result 对

这意味着 `fin` 的 compression / rebuild 必须由 framework 控制，而不是交给模型自由发挥。

---

## 2. Canonical layered model

`fin` 的多轮历史系统冻结为六层。

## 2.1 Operation / Event Ledger Layer

职责：

- 记录 runtime fact truth
- append-only 保存 operation 与 event
- 作为 replay / audit / debug / causality 的真源

典型内容：

- input accepted
- provider started/completed/failed
- tool started/completed/failed
- progress updated
- control feedback recorded
- dispatch / mailbox / heartbeat 事件

规则：

- ledger 永不作为“直接 prompt history”
- ledger 不被压缩替代
- debug 结论必须优先回看 ledger / event chain

## 2.2 Turn / Closure Record Layer

职责：

- 把一次完整 closure 作为 durable 多轮单元保存
- 成为会话连续性的标准中间层

定义：

- `turn`：用户输入驱动的一次完整推理闭环
- `step`：turn 内部的 reasoning / provider / tool 子步骤
- `closure`：正确停止的 turn
- interrupted execution 不形成独立 closure

每个 `TurnRecord` 最少应绑定：

- session / task / topic / worker / operation / trace 标识
- user input
- assistant visible output
- control feedback
- progress summary
- execution note refs
- context snapshot ref
- reasoning view refs
- tool execution refs
- provider request/response refs
- digest ref

## 2.3 Progress / Execution Note Layer

职责：

- 承载运行中的中层沉淀

分层：

- `ProgressBlock`
  - 高频、实时、偏监控
  - phase / blocker / next step / health / tool snapshots
- `ExecutionNote`
  - 持续积累、偏稳定沉淀
  - control block 的重要内容、plan changes、lessons、handoff facts

规则：

- execution note 持续生成，不等 closure 结束
- execution note 是 digest 的输入之一，但不是 digest 的替身

## 2.4 Digest / Compact Memory Layer

职责：

- 承担 continuity 与 rebuild 的压缩材料
- 不替代 raw truth

分型：

- `ClosureDigest`
- `TaskDigest`
- `TopicDigest`
- `SessionDigest`（用于会话选择/恢复概览）

规则：

- 每个有效 closure 必须生成 `ClosureDigest`
- 中断不单独生成 closure digest
- `TaskDigest` / `TopicDigest` 走 iterative update

## 2.5 Knowledge Artifact Layer

职责：

- 保存已验证、可复用、长期有价值的知识沉淀
- 作为跨 worker / 跨 session 的主共享层

进入条件：

- 已验证
- 可复用
- 不依赖瞬时局部上下文才成立

规则：

- 共享优先通过 knowledge artifact，而不是共享原始 session history
- skill 不是 canonical history store；skill 只承载流程和适配

## 2.6 Working Context Layer

职责：

- 作为单次推理的动态装配视图

规则：

- context 不是 session
- context 不是 ledger 原文
- context 是 framework 根据当前目标与最近连续性构建的 ephemeral view

---

## 3. Session and context relationship

## 3.1 Session

`Session` 是 durable substrate，持有：

- ledger
- turn records
- progress / notes
- digests
- artifacts
- control / routing state
- rebuild index

## 3.2 Context

`Context` 是一次推理调用的临时装配结果。

它来自：

- 当前 session 状态
- task / topic / dispatch 状态
- recent runjournal / recent turn tail
- collab deltas
- retrieved knowledge artifacts
- current input

## 3.3 Core rule

冻结规则：

> Session = durable substrate  
> Context = one inference-time assembled view

特别规则：

- `RunJournal` 最新 slice 必须是 context 中最重要的连续推理 history 之一
- `session` 内的材料不会无脑全部进入 context
- 进入 context 的内容必须经过 scope、relevance、continuity、budget 四重选择

---

## 4. Recommended ContextView composition order

上下文顺序冻结为“静态在前，动态在后，连续历史靠后”。

## 4.1 Stable prompt head

1. stable core prompt
2. role prompt
3. loaded skills / coding principles
4. tool catalog
5. output contract

## 4.2 Control and routing head

6. session/task/topic identity
7. task list / topic list
8. current routing hints
9. dispatch / progress summary
10. latest control state

## 4.3 Project and collaboration head

11. project context
12. active projects / project list
13. cwd / selected paths / runtime env
14. relevant collab deltas

## 4.4 Knowledge rebuild head

15. retrieved knowledge artifacts
16. task digests
17. topic digests
18. session digest summary

## 4.5 Recent continuity tail

19. recent closure digests
20. recent execution notes
21. recent reasoning summaries
22. recent tool activities
23. recent visible messages / recent turn tail

## 4.6 Current input

24. current user input / agent input / status probe input

---

## 5. Compression and rebuild policy

## 5.1 Compression ownership

compression 由 framework 控制，不由模型自由定义策略。

## 5.2 Never compress raw ledger

原始 event / ledger 永不被“摘要替换”。

允许压缩的层：

- turn layer
- digest layer
- rebuild material layer

## 5.3 Preservation rules

压缩必须保护：

1. 最近 `N` 个有效 closures
2. 未完成的 tool-call / tool-result 对
3. 最新 control state
4. 当前 active task/topic 绑定
5. recent runjournal + execution note tail

## 5.4 Iterative summary update

`TaskDigest` / `TopicDigest` 应采用 iterative update：

- 读取上一版 digest
- 合并新 closure digest / execution notes / control facts
- 更新 in-progress / done / risks / next-step

禁止每次从零扫描全部历史做大总结。

## 5.5 Rebuild triggers

以下情况触发 context rebuild：

- token pressure 超阈值
- topic shift / task switch
- session revive
- worker handoff
- long-gap resume

## 5.6 Rebuild inputs

rebuild 应组合：

- relevant task/topic digests
- retrieved knowledge artifacts
- recent continuity tail
- latest runjournal / execution note tail

---

## 6. Multi-worker sharing under one workdir

## 6.1 Non-sharing rule

同一 workdir 下多个 worker **不直接共享原始 session history**。

原因：

- 容易污染连续性
- 难以追踪 provenance
- debug 与压缩边界都会变脏

## 6.2 What is shared

推荐共享四类内容：

1. collab deltas
2. approved digests
3. promoted knowledge artifacts
4. task/project shared state

## 6.3 Retrieval scopes

共享范围冻结为：

- `worker_local`
- `session_local`
- `task_shared`
- `workdir_shared`
- `repo_shared`
- `global`

## 6.4 Promotion flow

共享提升流程为：

`RunJournal / CollabSpace -> candidate artifact -> classify -> verify/grade -> assign scope -> publish`

---

## 7. Session layout implications

未来 session 目录建议向以下职责结构演进：

```text
session/
  ledger/
    operations.jsonl
    events.jsonl
    step-ledger.jsonl
    worker-ledgers/

  conversation/
    messages.json
    turns/
    recent_turns.json

  progress/
  notes/
  control/
  context/
    recent_contexts.json
    rebuild-index.json

  digests/
    closure-digests.jsonl
    task-digests.jsonl
    topic-digests.jsonl
    recent_digests.json

  reasoning/
  tools/
  closures/
  collab/
  tasks/
  topics/
  artifacts/
    candidates/
    knowledge/
    verified/
```

说明：

- 当前 M1 已经有 `messages / recent_contexts / recent_digests / reasoning / tools / closures`
- 下一步不是推翻重做，而是在当前 artifacts 之上补出 canonical `TurnRecord / StepLedger / Digest family / Rebuild index`

---

## 8. Implementation priority after architecture freeze

推荐顺序：

1. 先补 canonical `TurnRecord / ClosureRecord`
2. 再补 turn 内部的 `StepLedger`
3. 再把 digest family 拆成 `closure / task / topic`
4. 再补 `retrieval scope + rebuild index`
5. 最后再升级到真正 multi-step inference loop

---

## 9. Final canonical sentence

`fin` 的多轮历史记录真模型冻结为：

- raw truth lives in ledger
- closed-loop conversation units live in turn records
- ongoing process state lives in progress / execution notes
- continuity and rebuild live in digest families
- cross-worker sharing lives in knowledge artifacts + retrieval scope
- model input always comes from a framework-built `ContextView`, never directly from raw history
