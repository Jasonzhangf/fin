# 14 Runtime Home Layout under `~/.fin`

本文档定义 `fin` 的统一运行时家目录：`~/.fin`。

它回答四件事：

1. 哪些数据必须进入 `~/.fin`
2. session / workdir / task / topic 如何在磁盘上分层
3. 日志、调试、harness、安装物放在哪里
4. `finger` 与 `codex` 的哪些目录思想被保留，哪些被重组

不展开：

- 最终文件字段
- wire format
- 检索排序公式
- UI 组件细节

## 1. 总原则

### 1.1 `~/.fin` 是唯一运行时家目录

`fin` 的运行时数据、配置、日志、调试证据、安装物、回归结果统一进入：

```text
~/.fin
```

这样做的目的：

- 保持运行时状态集中
- 避免 repo 内混入用户态数据
- 便于全局安装、回归、归档、恢复
- 便于 Web / CLI / harness 指向同一份证据目录

### 1.2 源码构建物与运行时家目录分离

需要明确区分两类东西：

- **源码构建物**：仍在 repo 内，例如 `rust/target/`
- **运行时家目录资产**：进入 `~/.fin`

规则：

- 编译产生的临时 target 不迁入 `~/.fin`
- 被提升为“可安装版本”的产物、回归证据、运行日志、session 数据进入 `~/.fin`

### 1.3 session history 不直接共享，价值通过 workdir scope 共享

沿用之前已经确认的原则：

- 原始 session history 保持 session-local
- 多 worker / 多 session 的价值共享不通过直接拼接原始 history
- 共享通过：
  - `CollabSpace`
  - `KnowledgeArtifactStore`
  - workdir 级索引 / 检索 scope

## 2. 顶层目录结构

建议的 `~/.fin` 顶层布局如下：

```text
~/.fin/
  config/
  skills/
  bin/
  install/
  runtime/
  sessions/
  workdirs/
  logs/
  diagnostics/
  harness/
  archive/
  tmp/
```

各目录职责：

- `config/`：配置真源
- `skills/`：全局可复用 skills 真源
- `bin/`：全局入口命令
- `install/`：版本化安装物与安装回执
- `runtime/`：活跃进程、锁、租约、心跳、当前投影
- `sessions/`：按时间分桶的 session 数据
- `workdirs/`：workdirectory 级共享域
- `logs/`：结构外文本日志
- `diagnostics/`：调试样本、崩溃、快照、trace
- `harness/`：录制、回放、故障注入、报告
- `archive/`：归档与清理后的冷数据
- `tmp/`：可清理临时目录

### 2.1 测试运行必须走隔离子树

测试产生的 runtime home、session、projection、logs 不能直接落到正常运行目录。

推荐布局：

```text
~/.fin/harness/runs/
  <run-id>/
    user.test.toml
    runtime-home/
      config/
      runtime/
      sessions/
      logs/
      diagnostics/
      harness/
```

规则：

- `run-id` 必须带 `test-` 前缀
- test `user.toml` 与正常 `user.toml` 分开
- test session 必须带 `test-` 命名空间
- 正常运行继续使用 `~/.fin/` 顶层；测试运行只允许写入 `~/.fin/harness/runs/<run-id>/runtime-home/`

## 3. `config/` 与 `skills/`：配置与全局 skills 目录

```text
~/.fin/config/
  user.toml
  system.toml
  system.template.toml

~/.fin/skills/
  <skill-id>/
    SKILL.md
    _meta.json (optional)
```

说明：

- `user.toml`
  - 用户唯一手写入口
  - 只暴露必须由用户选择的配置
- `system.toml`
  - 系统配置单文件
  - 供 runtime / provider / routing / debug / harness 统一读取
- `system.template.toml`
  - 框架自动生成或刷新模板的来源之一

约束：

- 配置文件统一支持注释，当前采用 TOML
- 不做 merge
- 只允许 `user.toml -> ConfigMapper/Normalizer -> system.toml / runtime view`

## 4. `bin/` 与 `install/`：全局安装布局

### 4.1 `bin/`

```text
~/.fin/bin/
  fin -> ../install/current/bin/fin
```

规则：

- 用户只需要一次性把 `~/.fin/bin` 加到 `PATH`
- 后续版本切换只更新 `install/current`
- 尽量不依赖 `/usr/local/bin` 这类系统级写入

### 4.2 `install/`

```text
~/.fin/install/
  current -> versions/<build-id>
  previous -> versions/<build-id>
  versions/
    <build-id>/
      bin/
      manifest.toml
      checksums.txt
  staged/
    <build-id>/
  receipts/
    <build-id>.json
  packages/
```

职责：

- `versions/`：已归档的可运行版本
- `staged/`：待回归验证的候选安装物
- `current/`：当前提升为全局默认版本的软链接
- `previous/`：最近一个可快速切换的上一版本
- `receipts/`：安装与提升回执
- `packages/`：打包文件（如后续需要 tarball / zip / pkg）

## 5. `runtime/`：活跃运行态目录

```text
~/.fin/runtime/
  locks/
  pids/
  sockets/
  leases/
  heartbeats/
  projections/
  current/
```

职责说明：

- `locks/`：单实例 / 资源互斥锁
- `pids/`：运行进程 pid 文件
- `sockets/`：本机 IPC / unix socket
- `leases/`：dispatch / task ownership lease
- `heartbeats/`：worker / node / project 心跳快照
- `projections/`：当前投影视图缓存
- `current/`：当前激活运行的简要元数据（latest-only，不做无界累积）

原则：

- `runtime/` 偏活跃态、可重建态
- 丢失后不应破坏长期知识真源
- 长期沉淀应进入 `sessions/`、`workdirs/`、`harness/`、`diagnostics/`
- `runtime/current/` 只允许保存 **latest/current truth**；不得把控制面心跳、轮询、调试快照做成无界累积
- `runtime/peers/*/events.jsonl` 与 `logs/runtime/*.log` 只作为调试辅助，必须是 **bounded recent window**，不能冒充长期历史真源

## 6. `sessions/`：按时间分桶的会话目录

这里参考 `codex` 的时间分桶方式，同时结合 `finger` 的 richer runtime 分层。

```text
~/.fin/sessions/
  YYYY/
    MM/
      <session-id>/
        session.toml
        meta.json
        events/
          stream.jsonl
        journal/
          worker-<worker-id>.jsonl
        progress/
          progress.jsonl
          latest.json
        notes/
          execution-notes.jsonl
        digests/
          digest-<closure-id>.json
        closures/
          closure-<closure-id>.json
        context/
          recent_contexts.json
          rebuilds/
        collab/
          inbound.jsonl
          outbound.jsonl
          shared-deltas.jsonl
        tasks/
          index.json
          task-<task-id>.json
        topics/
          index.json
          topic-<topic-thread-id>.json
        artifacts/
          candidates/

补充冻结规则：

- session 下的 `recent_*` 文件是 **bounded recent working set**，由 retention 控制；它们不是长期全量归档
- `latest.json` / `current_*.json` 保留当前 closure / 当前 runtime 的 authoritative snapshot，可大但必须是 **latest overwrite**，不是无界 append
- framework-owned hidden/control-plane turn（例如 startup/heartbeat/project-resume/assignment-resume）只允许更新 current/control truth；**不得污染普通会话 `conversation/messages.json`、recent history、events stream/archive`**
```

测试 session 的同构目录则进入：

```text
~/.fin/harness/runs/<run-id>/runtime-home/sessions/
  YYYY/
    MM/
      test-<session-id>/
```

### 6.1 session 目录内部边界

- `session.toml`
  - session 元信息
  - 例如创建时间、绑定 workdir、主 role、当前状态
- `events/stream.jsonl`
  - session 级结构化事件流
  - replay / debug / projection 的关键证据
- `journal/`
  - `RunJournal` 的原始流水账
  - 每个 worker 独立文件，不互相覆盖
- `progress/`
  - `ProgressBlock` 的持续快照
  - 包含工具执行 snapshot、当前 phase、blocker、next step
- `notes/`
  - `ExecutionNote` 连续流
- `digests/`
  - 每个完整 closure 生成一个 digest
- `closures/`
  - closure 级原始归档，供 rebuild / audit 使用
- `context/`
  - context rebuild 快照与上下文组装结果
- `collab/`
  - session 内协作增量
- `tasks/`
  - 当前 session 涉及的 task 明细与索引
- `topics/`
  - 当前 session 触达的 topic thread 映射
- `artifacts/candidates/`
  - 尚未发布到共享域的候选知识产物

### 6.2 session 的关键规则

1. 同一 session 下可有多个 worker journal
2. worker 原始记录不丢失，不做覆盖式合并
3. closure 完成时必须生成 digest
4. interrupted segment 不单独形成 final digest
5. 可共享价值优先晋升到 `workdirs/` 下的共享域，而不是直接写别人 session

## 7. `workdirs/`：workdirectory 级共享域

这是 `fin` 相比 `codex` 更关键的一层，用于回答：

> 多个 worker 在同一个工作目录下工作时，如何共享有价值内容而不污染彼此原始 session？

```text
~/.fin/workdirs/
  <workdir-id>/
    manifest.toml
    sessions/
      index.json
    task-graph/
      tasks.json
      dispatches.json
    topics/
      topics.json
    collabspace/
      board.jsonl
      mailbox/
    artifacts/
      task_shared/
      workdir_shared/
      repo_shared/
      indexes/
    health/
      workers.json
      leases.json
      watchdog.json
    retrieval/
      catalog.json
      classification.json
```

### 7.1 `workdir-id`

`workdir-id` 应由真实工作目录规范化得到，例如：

- 绝对路径规范化
- hash / slug 化
- 同时在 `manifest.toml` 中保留真实路径

### 7.2 `workdirs/` 内部职责

- `manifest.toml`
  - workdir 元信息
  - 真实路径、repo 标识、创建时间、最近访问时间
- `sessions/index.json`
  - 该工作目录关联的 session 列表
- `task-graph/`
  - `Project` / `TaskGraph` / `Dispatch` 真源
- `topics/`
  - 长期 topic thread 索引
- `collabspace/`
  - 多 worker 协作事实空间
- `artifacts/`
  - 跨 session 的知识共享主域
- `health/`
  - worker 健康、lease、watchdog 观测
- `retrieval/`
  - 检索索引与分类结果

### 7.3 共享规则

workdir 级共享不是共享全部历史，而是共享被框架晋升后的内容：

- `task_shared`
  - 当前任务族共享
- `workdir_shared`
  - 当前工作目录共享
- `repo_shared`
  - repo 层长期复用

因此：

- 原始 `RunJournal` 留在 session
- 共享靠 artifact / collab / task graph / topic graph
- retrieval scope 是共享边界的核心控制面

## 8. `logs/`：文本日志目录

```text
~/.fin/logs/
  cli/
  runtime/
  provider/
  orchestrator/
  debug-server/
  install/
  regression/
```

规则：

- 文本日志按模块拆目录，不做一个巨型总日志
- 结构化真源仍是 event / journal / progress / note / digest
- `logs/` 主要服务于人类排障与快速 grep

推荐日志命名：

- 按日期滚动：`YYYY-MM-DD.log`
- 或按运行实例：`<component>-<timestamp>.log`

## 9. `diagnostics/`：调试与故障证据

```text
~/.fin/diagnostics/
  crashes/
  error-samples/
  traces/
  snapshots/
  repro/
```

职责：

- `crashes/`：panic / 崩溃上下文
- `error-samples/`：失败输入、失败响应、异常样本
- `traces/`：按 trace_id 导出的诊断片段
- `snapshots/`：系统状态快照
- `repro/`：可重放的最小复现包

这部分优先面向：

- runtime debug
- harness 回放
- CI 失败取证
- 用户反馈问题复现

## 10. `harness/`：录制、回放、故障注入与报告

```text
~/.fin/harness/
  recordings/
  replays/
  fault-injection/
  baselines/
  reports/
```

职责：

- `recordings/`：录制的输入/事件材料
- `replays/`：可执行回放包
- `fault-injection/`：故障注入场景
- `baselines/`：基线输出
- `reports/`：每轮回归与验证报告

原则：

- harness 输出进入 `~/.fin`，不散落在 repo 各处
- CI 与本地回归应尽量写入同一报告模型

## 11. `archive/` 与 `tmp/`

```text
~/.fin/archive/
  sessions/
  logs/
  diagnostics/
  harness/

~/.fin/tmp/
```

说明：

- `archive/`：冷数据归档区
- `tmp/`：临时文件，可随时清理

## 12. 来自 `finger` 与 `codex` 的继承与重组

### 从 `codex` 保留

- `sessions/YYYY/MM/<session-id>` 这种时间分桶
- 配置、日志、session 的大类分离
- 全局家目录集中化

### 从 `finger` 保留

- runtime/debug/logging/config 各自独立目录
- 更丰富的 diagnostics / logs / orchestration 证据分区
- 长期运行系统的运维视角

### `fin` 的重组点

`fin` 不直接复制任一现有布局，而是做两点收敛：

1. 加入 `workdirs/` 作为多 session / 多 worker 共享域
2. 明确区分：
   - session-local 原始历史
   - workdir-shared 协作与知识
   - runtime-active 活跃态
   - install / harness / diagnostics 运维态

## 13. 当前冻结的目录级决策

当前可以视为目录级真源的内容：

1. `~/.fin` 是唯一运行时家目录
2. `config/`、`sessions/`、`workdirs/`、`logs/`、`diagnostics/`、`harness/`、`install/` 必须存在
3. session 原始历史与 workdir 共享知识必须分层
4. `~/.fin/bin/fin` 作为全局入口，指向版本化安装目录
5. build target 与 home runtime data 不混放

## 14. 当前非目标

以下留到模块实现阶段再细化：

- 各 json/toml 文件最终字段
- 具体 retention 策略
- 索引格式与压缩格式
- 加密/脱敏策略
- 多机同步时目录复制协议


## 13. latest-only context snapshot 规则（M1 冻结）

为了控制 CPU / 内存 / IO，本项目当前冻结如下写法：

- `~/.fin/runtime/current/current_context.json`
  - 仅保留最新一次 context snapshot
  - 覆盖写
- `~/.fin/sessions/YYYY/MM/<session-id>/context/recent_contexts.json`
  - 仅保留最近窗口
  - 当前默认窗口：8

禁止的写法：

- 每轮一个 context 文件无限增长
- 每个事件附带单独大 snapshot 文件
- 为了 debug 把完整上下文碎片化散落到多个目录


补充规则：

- `~/.fin/skills` 是全局 skills 的运行时真源目录
- 运行时 prompt build 可从这里装载全局 skill index
- repo 内 `skills/` 仍是项目本地技能真源，两者职责不同
