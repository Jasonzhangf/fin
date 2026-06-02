# M2 Entry Recommendation

本文档只回答一个问题：

> 在 M1 已收口的前提下，`fin` 下一阶段最小、最稳、最不容易重新发散的入口应该是什么？

它不是新的宏大路线图，而是 **M2 的最小开口建议**。

## 1. 前提判断

截至 2026-04-19，当前项目已经满足进入 M2 讨论的最低条件：

1. M1 closeout 文档已形成
2. 单 agent 推理主链已冻结
3. mainline receipts 已成组通过
4. build / install / regression gate 已恢复
5. runtime / session / web 的真源边界已明确

因此，下一步不该再继续以“补 M1 基础能力”为目标。

当前真正的问题已经变成：

- 哪条 M2 入口最稳
- 哪条入口最不容易把系统重新拉回大扩张
- 哪条入口最能延续当前已经建立的 durable truth / control-plane 方向

## 2. 当前不建议优先开启的方向

以下方向虽然都在 backlog 里，但**现在不建议作为 M2 第一入口**。

### 2.1 remote peer / distributed multi-agent

原因：

1. 会同时拉起 discovery / auth / heartbeat / reconnect / lease / capability routing
2. 会把本地 control-plane 尚未彻底固化的问题放大到分布式层
3. 一旦先做，很容易重新回到“边接边试”的扩张状态

结论：

- 可以保留架构接口
- 不建议作为 M2 第一阶段主线

### 2.2 channel / gateway 生产化接入

原因：

1. channel 会引入外部生命周期、鉴权、配对、消息规范
2. 这类工作对产品观感很强，但会冲淡 runtime 真源继续固化的优先级
3. 当前更适合作为 M2 后段、依附于更稳定 control-plane 的接入层

结论：

- 保留为 M2 backlog
- 不作为第一阶段

### 2.3 detached / headless daemon 完整常驻体系

原因：

1. 当前 supervisor / heartbeat / daemon observation 已有最小闭环
2. 但 runtime-owned scheduler / recovery / lease 生命周期仍处在“最小可观察”阶段
3. 如果直接做完整后台常驻，会很快引出 orphan、资源回收、升级切换等更大问题

结论：

- 先继续固化 runtime-owned truth
- 再进入真正 headless 化

## 3. 建议的 M2 最小入口

建议把 M2 第一阶段收敛为：

> **M2-A = runtime-owned control-plane hardening + session/task/topic formalization boundary clarification**

也就是两件事，但仍然属于同一条主线。

### 3.1 第一部分：runtime-owned control-plane hardening

目标不是再发明新概念，而是把 M1 已经出现的控制面事实进一步从“可观察”推进到“更可执行、更可持续”。

建议范围：

1. 继续收紧 `ExecutionState / PendingInput / SchedulerDecision / SupervisorCycle` 的 owning truth
2. 明确 runtime 如何推进 queue、wait、tick、reminder、lease 的 canonical state machine
3. 让 archive / session / status probe / web debug 对同一控制面事实保持严格同源
4. 把当前已存在的“观察结果”进一步压实成稳定的 framework-owned 控制动作边界

这条线的价值：

- 延续 M1 已经形成的 durable truth 基础
- 风险可控
- 对后续 daemon / peer / multi-agent 都是前置基础

### 3.2 第二部分：session/task/topic formalization boundary clarification

目标不是直接把完整产品交互全做完，而是先把边界冻结清楚。

建议范围：

1. 明确 tentative session / formal task / topic continuity 的 runtime truth 边界
2. 明确哪些判断来自 model control feedback，哪些属于 framework control action
3. 明确 session 复活、切换、reuse、side-topic 的最小 canonical objects
4. 先冻结对象模型与状态转移，再决定 UI/CLI 的产品化交互

这条线的价值：

- 直接回应当前系统最容易继续扩散的话题
- 但仍然停留在“冻结边界”，不是直接做大产品面
- 后续 memory / knowledge / multi-worker 协作都会依赖这层清晰对象模型

## 4. 为什么推荐这条入口

原因很直接：

1. **它延续当前项目已经做对的方向**
   - 不是重新换主题
   - 而是继续把 truth / control / session 这条主线做稳

2. **它能最大化保护 M1 已收下的成果**
   - 如果 control-plane 不继续固化，后续所有 M2 扩展都会重新引入第二事实源风险

3. **它对未来扩展的支撑性最强**
   - daemon
   - remote peer
   - channel/gateway
   - multi-agent
   都依赖更稳定的 runtime-owned control truth

4. **它最不容易让项目重新失焦**
   - 不会一上来就陷进网络、鉴权、部署、跨机生命周期
   - 仍然能以本地可验证闭环推进

## 5. 建议顺序

建议 M2 不要一口气展开，而是按以下顺序进入：

### Phase A
runtime-owned control-plane hardening

成功信号：

1. queue / wait / reminder / tick / heartbeat / lease 的状态推进边界进一步冻结
2. status / archive / session / web 对控制面事实无分叉
3. scheduler / supervisor 的回归矩阵明确稳定

### Phase B
session/task/topic object boundary formalization

成功信号：

1. tentative session / formal task / topic continuity 有明确对象模型
2. runtime vs model vs UI 的职责边界写清
3. 可以据此设计后续产品交互，而不是边做边猜

### Phase C
再决定是否进入 peer / channel / daemon / multi-agent

进入条件：

1. Phase A 与 B 的边界已稳定
2. 新入口不会破坏现有 truth consistency
3. 已能明确指出新增模块的 owning layer 与验证矩阵

## 6. 当前执行纪律

若正式进入 M2，建议继续保留以下纪律：

1. 仍然先冻 docs 真源，再落实现
2. 仍然优先做最小闭环，不做平行大扩张
3. 仍然先确保 runtime 真源，再做 Web / channel / peer 展示或接入
4. 任何会引入第二事实源的方便性写法，一律拒绝

## 7. 一句话结论

当前最合理的下一阶段入口不是：

- 直接做 remote multi-agent
- 直接做 channel/gateway
- 直接做 headless daemon 生产化

而是：

> **先做 runtime-owned control-plane hardening，再冻结 session/task/topic 的正式对象边界。**

这是当前最稳、最收敛、最能保护 M1 成果的 M2 起点。
