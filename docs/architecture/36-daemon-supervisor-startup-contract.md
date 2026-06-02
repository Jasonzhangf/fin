# 36 Daemon and Supervisor Startup Contract

本文档冻结 `fin` 的 **启动面分层**：

1. 谁负责启动用户入口 agent
2. 谁负责启动/监管 project worker
3. daemon / supervisor 与 entry agent 的责任边界

目标：

- 防止后续把 `system entry`、`project worker`、`channel gateway` 混着启动
- 给 always-on / reconnect / orphan recovery / peer supervision 提供统一真源

---

## 1. 核心结论

`fin` 的启动面必须分成三层：

```text
Launcher / CLI / Service Entry
  -> Daemon / Supervisor
  -> System Entry Agent
  -> Project Worker Agents / Channel Gateways / Capability Peers
```

冻结规则：

1. `system entry agent` 是唯一用户入口 agent
2. `daemon / supervisor` 是本机生命周期管理真源
3. `project worker` 不是启动入口，它们只能被 system/daemon 派生或恢复

---

## 2. 启动责任分工

### 2.1 Launcher / CLI

负责：

- 读取 `user.toml`
- 映射出 `system.toml`
- 初始化 `~/.fin`
- 启动前台模式或请求 daemon 接管

不负责：

- 长期保存 worker 生命周期
- project worker 的长期巡检
- channel peer 的崩溃恢复

### 2.2 Daemon / Supervisor

负责：

- 持有本机 runtime registry
- 启动 / 停止 / 重启 entry agent
- 启动 / 停止 / 重启 project worker
- 监督 QQ / channel gateway / capability peer
- 做 lease / heartbeat / watchdog / orphan reaping

这是 **生命周期真源**。

### 2.3 System Entry Agent

负责：

- 用户会话入口
- routing / orchestration
- project worker 任务分发
- 状态汇总与 channel 输出

这是 **用户会话与逻辑编排真源**。

### 2.4 Project Worker

负责：

- 项目内闭环执行
- progress / note / digest / artifact 回传
- 接受 system/daemon 的 supervision

不是：

- 用户入口
- 启动根
- 生命周期真源

---

## 3. 启动 contract

### 3.1 Entry startup

启动用户入口时必须读取：

```text
policy.entry_role
```

并实例化：

- entry runtime
- entry agent identity
- channel bindings

默认：

```toml
entry_role = "system"
```

### 3.2 Project startup

project worker 若未显式指定 role，读取：

```text
policy.default_role
```

默认：

```toml
default_role = "project"
```

### 3.3 Supervisor startup

daemon/supervisor 本身不是 prompt role。

它是 framework-owned control plane。

冻结规则：

- supervisor 不进入 prompt role taxonomy
- supervisor 只通过 registry / lease / health / assignment 与 runtime 交互

---

## 4. 最小启动序列

### 4.1 当前单前台最小实现

M1 当前仍允许：

```text
fin web-debug
  -> ensure runtime home
  -> start channel gateway
  -> start entry agent (entry_role)
```

这属于 **前台一体化模式**。

### 4.2 后续 daemonized 模式

后续标准模式应为：

```text
fin start
  -> daemon.ensure_running
  -> daemon.ensure_entry_agent(entry_role)
  -> daemon.ensure_gateways
  -> system agent binds session/channel
```

project worker 则通过：

```text
daemon.ensure_project_worker(project/default_role)
```

被派生、恢复或回收。

---

## 5. 命名与身份

所有启动出来的 agent 都必须满足：

- `agent_id = <device_name>.<agent_name>`
- `worker_id` 唯一
- runtime artifacts / registry / status probe / debug UI 可追踪

补充冻结：

- entry agent 的名字不能继续沿用 demo 名字
- project worker 的名字也不能退化成匿名 `worker-project`

---

## 6. 验收标准

至少满足：

1. 配置上能区分：
   - `entry_role`
   - `default_role`
2. 用户入口一次真实请求后：
   - context role 为 `system`
3. project worker 默认仍为 `project`
4. 文本 channel / Web debug / status probe 不再把启动身份和显示身份混淆
5. daemon / supervisor 的责任边界有单独真源文档，不再埋在 prompt 或 UI 行为里
