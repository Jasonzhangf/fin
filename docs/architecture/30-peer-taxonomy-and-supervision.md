# 30 Peer Taxonomy and Supervision Model

本文档冻结 `fin` 在多机/多进程协作阶段的统一 peer 模型。

本文档回答四个问题：

1. `system agent`、`project agent`、`daemon` 各自是什么
2. remote peer 为什么不能只按 “agent / 非 agent” 粗分
3. 本机与远端 peer 如何统一纳入同一发现/接入框架
4. 生命周期、资源回收、异常恢复由谁负责

---

## 1. 核心结论

`fin` 不应把远端对象只理解成 “agent”。

系统应采用统一 `peer` 模型：

```text
System Agent
  -> connects to Peer Network

Peer Network
  ├── Capability Peer
  ├── Agent Peer
  └── Channel Gateway
```

同时，本机还必须有一个不直接承担用户会话的监督层：

```text
User / Web / CLI / Channel
  -> System Agent
  -> Daemon / Supervisor
  -> Local Peers / Remote Peers
```

冻结结论：

1. `system agent` 是唯一用户入口与总编排者
2. `project agent` 属于 `agent peer`，它不直接和用户交互
3. 非智能能力节点属于 `capability peer`
4. 输入输出接入节点属于 `channel gateway`
5. `daemon` 是本机 peer 生命周期与资源管理真源
6. 后续所有本地/远端协作，都先归一到 `peer`，再按 `peer_kind` 分流执行语义

---

## 2. 三个核心执行角色

### 2.1 System Agent

`system agent` 是：

- 唯一直接和用户打交道的 agent
- 唯一拥有用户会话主真源的 agent
- 全局路由、协作、汇总、恢复、切换的控制者

它负责：

- 用户输入与输出
- session / task / topic 主视图
- peer 选择与绑定
- task 拆解与协作分发
- 汇总 project/capability/channel 返回的进度、artifact、result

它不负责：

- 直接管理本机孤儿进程
- 直接承担本机 peer 的资源回收
- 把 project agent 变成第二个用户入口

### 2.2 Project Agent

`project agent` 是一种 `agent peer`。

它可以：

- 维护本地 runtime
- 执行 agent loop
- 使用 provider
- 消费 context view
- 产出 progress / note / digest / artifact / event

它默认：

- 不直接面向用户
- 不拥有用户 session 的主真源
- 不绕过 system agent 接收用户会话

它拥有的是：

- 本地执行账本
- task 级上下文
- worker/project 级连续性

### 2.3 Daemon / Supervisor

`daemon` 是本机监督与回收层，必须存在。

它负责：

- spawn / stop / restart local peers
- crash 检测与 backoff
- orphan 回收
- lease/watchdog
- 本机 peer registry
- 资源约束与空闲回收
- system agent 退出后的善后

冻结结论：

```text
system agent = 逻辑控制真源
daemon       = 生命周期/资源管理真源
project peer = 执行节点
```

---

## 3. Peer 分类

### 3.1 Capability Peer

`capability peer` 是标准能力节点，不是自主 agent。

特点：

- 提供标准 capability
- 有明确输入/输出 schema
- 不承担复杂上下文闭环
- 不直接拥有用户会话
- 可同步或异步返回结果

典型例子：

- shell runner
- search/index service
- file analyzer
- browser capability service
- retrieval service
- build/test worker

### 3.2 Agent Peer

`agent peer` 是能执行 agent loop 的执行节点。

特点：

- 支持多轮推理
- 支持工具执行
- 支持局部 session/task ledger
- 支持 digest/context rebuild
- 对外汇报 progress / artifact / result

`project agent` 属于 `agent peer` 的一个具体角色。

### 3.3 Channel Gateway

`channel gateway` 是输入/输出接入节点。

特点：

- 接收外部输入
- 把 system agent 输出渲染为具体 channel 格式
- 处理 delivery / ack / retry / render constraints

典型例子：

- Web UI
- CLI / TUI bridge
- QQ / Telegram / Slack adapter
- webhook ingress

冻结结论：

- gateway 不是用户会话真源
- gateway 不是 task 编排者
- gateway 不直接替代 system agent

---

## 4. Presence 对称，Binding 非对称

这是后续所有 peer 协作的核心原则。

### 4.1 Presence 对称

所有 peer 都可以：

- 宣告自己在线
- 发送 heartbeat
- 续租 lease
- 感知对方失联
- 尝试 reconnect

### 4.2 Binding 非对称

只有 `system agent` 可以发起：

- task binding
- session binding
- channel 到 session 的路由绑定
- 某个 peer 的实际执行归属

冻结结论：

- peer 可以重连
- peer 可以报告能力和状态
- peer 不能自行接管用户会话
- peer 不能自发成为用户主路由器

---

## 5. 本机与远端统一归一到 Peer Registry

无论对象来自：

- 本机 daemon 管理的 process
- 本地常驻 unattached project agent
- `fin start --slave` 启动的远端服务
- channel gateway

它们都先进入统一 registry：

```text
Peer Registry
  -> peer descriptors
  -> presence state
  -> health state
  -> capability catalog
  -> binding relations
```

冻结结论：

1. system agent 不直接“记忆一堆连接细节”
2. system agent 维护的是 peer 期望状态与绑定状态
3. daemon 维护本机 peer 的生命周期真源
4. 远端 peer 通过同样 descriptor 进入 system agent 的观察模型

---

## 6. Local unattached project agent 的正确启动方式

system agent 不应直接 fork 一个 project agent 然后自己托管。

正确路径：

```text
system agent
  -> daemon.ensure_peer(project agent)
  -> daemon spawn or reuse
  -> daemon returns peer descriptor
  -> system agent connects + lease + bind
```

这样可以保证：

- system agent 崩溃后不产生 unmanaged orphan
- daemon 可以统一 restart/reap
- daemon 可以做 idle timeout / quarantine / drain

---

## 7. `fin start --slave` 的语义冻结

`fin start --slave` 应被定义为：

> 启动一个 `project agent` service mode，进入 idle/listening，等待被 system agent 发现、连接、鉴权、建立 lease，并接收 task binding。

它启动后至少提供：

- handshake / auth
- heartbeat / lease
- task assign / task status
- progress / artifact / result 回传

冻结结论：

- `--slave` 是远端 `agent peer` 服务模式
- 它不是用户主入口
- 它不直接替代 system agent 的 session 管理

---

## 8. Daemon 必须拥有的能力

daemon 至少负责四类东西：

### 8.1 Lifecycle

- spawn
- stop
- restart
- drain
- reap

### 8.2 Resource

- pid
- socket / listen addr
- workdir / runtime home
- memory/cpu soft limit
- stale lock cleanup

### 8.3 Health

- heartbeat watch
- crash count
- repeated failure quarantine

### 8.4 Local Peer Registry API

- list local peers
- ensure local peer
- restart peer
- stop peer
- inspect health

---

## 9. 用户会话的真源边界

必须冻结：

> 用户会话主真源始终属于 `system agent`。

project/capability/gateway peer 不拥有：

- 用户会话主 timeline
- 用户 session 主路由
- 用户 task/topic 全局真源

它们拥有的是：

- 本地执行账本
- 局部 task/session 视图
- capability/job 结果
- channel 适配态

---

## 10. M1 收敛范围

M1 不做完整 federation，只做最小真框架：

1. local daemon
2. system agent
3. local capability peer
4. local project agent
5. remote `--slave` 明确地址接入
6. handshake / auth / lease / bind
7. progress / event / result 回流
8. daemon 负责 orphan / restart / reap

M1 不做：

- 局域网自动广播发现
- NAT 穿透
- 多 system agent 竞争
- 自动 task 迁移
- 跨机器共享文件系统

---

## 11. 一句话冻结

`fin` 的多机协作模型应冻结为：

> `system agent` 是唯一用户入口与总编排者；`project agent` 是可本地/远端部署的 `agent peer`；标准能力节点是 `capability peer`；输入输出接入是 `channel gateway`；`daemon` 是本机 peer 生命周期与资源管理真源。
