# 42 Event-Driven Collaboration And Trigger Model

本文档冻结 `fin` 的协作驱动总原则。

目标：

1. 明确 `framework` 与 `model` 的职责边界
2. 明确 startup / control block / tools 三类触发面
3. 统一后续 daemon / startup / presence / progress / task-system 的解释口径

---

## 1. 核心结论

`fin` 不是“框架主动编排一切”的系统，而是：

```text
framework = truth + event + trigger + sync
model     = decide + act + report
```

冻结规则：

1. framework 持有和同步结构化真相
2. framework 记录事件与控制面快照
3. framework 只在**刚性条件**满足时 materialize trigger
4. model 基于 truth / trigger / tools 自己决定下一步
5. model 的决定再通过 tool / control block / runtime write-back 同步回 framework

禁止：

- framework 从普通自然语言自行判断业务推进
- framework 替 model 做 dispatch / review / resume / continue 的业务决定
- framework 用 UI 文案、关键词、阈值 heuristic 冒充 trigger

---

## 2. 四层模型

### 2.1 Truth layer

framework 持有可共享的结构化事实，例如：

- session / task / assignment / project registry
- agent presence / presence registry
- startup topology / startup summary
- wake queue / supervision / recovery report
- operation / event timeline
- tool receipt / delivery state / binding state

这层回答的是：

```text
现在已知什么是真的？
```

### 2.2 Trigger layer

trigger 不是业务执行本身，而是：

```text
某个刚性条件已经满足，允许 model 或 runtime 进入下一步判断/动作
```

合法 trigger 来源只允许三类：

1. **Startup trigger**
   - framework 在启动时准备：资源、已注册 agent、unfinished work、recoverable assignment、project/task truth
   - 这些 truth 进入 startup snapshot / current files / initial prompt surface
   - system agent 启动后先做自检，再决定是否恢复 worker

2. **Control-block trigger**
   - model 在标准化 control block / schema 中显式表达：resume / dispatch / report / wait / complete 等
   - framework 只做解析、校验、持久化、刚性执行结果
   - framework 不得从普通聊天文本猜 control intent

3. **Tool trigger**
   - model 调用 project/task/presence/agent 等工具后，工具会把共享 truth 写回 framework
   - 这些更新会成为新的 trigger surface，供别的 agent / 后续 turn 读取

### 2.3 Model decision layer

model 负责：

- 读取当前 truth / trigger / toolset
- 判断继续执行、派发、恢复、等待、汇报
- 必要时调用工具或输出 control block
- 对自己的 worker/session 做 self-check 与 report

这里的关键不是“框架已经安排好流程”，而是：

```text
框架把事实摆出来，model 自己决定怎么走。
```

### 2.4 Sync / materialization layer

framework 负责把 model 的显式决定同步成 durable truth：

- tool write-back
- operation / event append
- projection update
- current snapshot update
- delivery / status / web 可读快照

这层不拥有业务判断权，只拥有：

- schema 校验
- 持久化
- side effect 执行
- 结果回写

---

## 3. Startup 触发原则

标准启动流程冻结为：

```text
launcher / daemon startup
-> framework materialize startup truth
-> start canonical system entry agent
-> system agent reads startup truth and self-checks
-> system agent decides whether to restart / recover project workers
-> worker starts
-> worker self-checks
-> worker reports back
```

边界：

1. framework 可以准备 startup snapshot，但不能替 system agent 决定“恢复哪个 worker、继续哪个任务”
2. `always_on / unfinished_work / recoverable_assignment` 是 trigger truth，不是 framework 已替 agent 执行完成
3. worker 启动后的第一动作应是 self-check + report，而不是 framework 代填“已恢复成功”

---

## 4. 推理期触发原则

推理过程中，framework 只能基于结构化条件触发。

合法做法：

- 发现 due reminder，记录 trigger truth
- 发现 stale lease，记录 recovery-needed truth
- 解析到 control block 的 `dispatch_ready_task`，切换/缩减 visible toolset 或执行对应刚性 side effect
- 读取工具回执后更新 task / presence / supervision truth

非法做法：

- 因为聊天里看起来像任务，就自动 formalize / dispatch
- 因为某 worker 离线，就自动把它推到 `idle/resume_ready`
- 因为 wake queue 非空，就自动继续 project runtime
- 因为 status 卡看起来该恢复，就替 model 执行 continuation

---

## 5. 工具与共享状态原则

model 内部可以有自己的短期记忆与推理态；
但**共享协作态**必须通过 framework 可见接口同步。

允许进入共享真相的入口：

- tool 调用
- control block
- runtime 写盘的 operation / event / projection

因此：

- project 管理数据由 project/task 工具同步
- agent 自身状态由 presence/report 工具或 runtime observation 同步
- 别的 agent 只能通过 tool/context/truth 读取这些共享状态

---

## 6. 审查标准

判断一个模块是否越界，只问四个问题：

1. 它是在**记录 truth**，还是在**替 model 做决定**？
2. 它是在**materialize trigger**，还是在**直接推进业务执行**？
3. 它的动作是否可追溯到**startup truth / control block / tool write-back**？
4. 它是否把“等待 model 决策”偷换成了“framework 已经执行完成”？

若任一答案偏向后者，即判为 boundary violation。

---

## 7. 与其它文档的关系

- `16-operation-and-event-model.md`：定义事实流与事件流
- `36-daemon-supervisor-startup-contract.md`：定义启动分层
- `38-project-registry-and-wakeup.md`：定义 registry / wake trigger 真相
- `39-agent-presence-and-resume-model.md`：定义 presence 只表达观察真相
- `40-attached-control-plane-cycle.md`：定义 attached 模式下的 truth refresh / trigger scan wrapper
- `41-progress-updater-and-delivery.md`：定义如何展示这些 truth，而不伪装成框架已执行业务
