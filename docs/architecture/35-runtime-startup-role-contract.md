# 35 Runtime Startup Role Contract

本文档冻结 `fin` 的 **启动身份真源**。

目标：

1. 明确“用户入口 agent 用什么 role 启动”是配置问题，不是显示问题
2. 明确 `system` 与 `project` 在启动阶段的不同责任
3. 防止后续再次出现“前台显示 system，实际 runtime 还是 project”的双真源

---

## 1. 核心结论

`fin` 运行时必须区分两类 role 入口：

1. `default_role`
2. `entry_role`

冻结语义：

- `default_role`：普通 runtime / project execution / worker runtime 的默认 role
- `entry_role`：**用户入口 agent** 的启动 role

当前默认值：

```toml
[policy]
default_role = "project"
entry_role = "system"
```

冻结原因：

- 用户入口必须是 `system`
- 项目闭环执行默认仍然是 `project`
- 两者不能再共用一个“默认 role”字段去赌调用方自己记得传参

---

## 2. 角色与启动面

### 2.1 entry role

`entry_role` 只用于：

- Web / QQ / CLI chat 这类用户对话入口
- system frontstage
- 总编排、状态汇总、peer 感知、路由决策

当前要求：

- 所有 channel ingress 默认都必须走 `entry_role`
- 若无特殊配置，`entry_role = system`

### 2.2 default role

`default_role` 用于：

- demo / transcript / project runtime 的普通启动
- project 闭环执行
- 未显式指定 role 的本地执行体

当前要求：

- 若无特殊配置，`default_role = project`

---

## 3. 启动规则

### 3.1 用户入口

用户入口组件启动 runtime 时，必须读取：

```text
system.policy.entry_role
```

而不是：

- 写死 `system`
- 依赖 UI 文案
- 依赖当前 session 上下文猜测
- 复用 `default_role`

### 3.2 project / worker runtime

project execution runtime 若未显式指定 role，走：

```text
system.policy.default_role
```

project worker 即使派生多个 runtime，也仍共享 `project` role 真相；差异体现在：

- `agent_id`
- `worker_id`
- task binding
- mailbox / assign / progress

而不是新增 role。

---

## 4. 配置 contract

当前最小配置 contract：

```toml
[policy]
default_role = "project"
entry_role = "system"
protocol_version = "fin.m1"
```

校验规则：

1. `default_role` 必须非空且存在于 `policy.roles`
2. `entry_role` 必须非空且存在于 `policy.roles`
3. `default` 只允许作为历史兼容 alias，解析后落到 `project`
4. `entry_role` 不允许偷偷走 alias 语义；它必须是显式真实 role

---

## 5. 当前实现绑定

### 5.1 Web / Channel ingress

当前 `web-debug` / channel ingress 主链必须：

- 用 `policy.entry_role` 启动前台入口 runtime
- 不再把用户入口 runtime 冒充为 `project`

### 5.2 Context truth

`current_context.json` / session context artifacts 里的：

- `role_prompt.role_id`

必须反映真实启动 role。

验收口径不是“界面显示 system”，而是：

```text
真实 current_context / runtime artifacts / prompt assembly / routing truth 都显示 system
```

---

## 6. Agent naming 约束

入口 agent 与 project runtime 的名字也必须和角色语义一致。

冻结规则：

- 用户入口 agent 不应该继续沿用 `cli-demo` 这类 demo 名称
- 用户入口 agent 的命名应由启动器/daemon/system entry 语义决定
- project runtime / worker runtime 可继续按本地名字池分配

也就是说：

```text
role truth != display alias
agent name != demo leftover
```

---

## 7. 验收标准

至少满足：

1. `ConfigMapper` 产出的 system config 默认包含：
   - `default_role = project`
   - `entry_role = system`
2. 若 `entry_role` 不在 `policy.roles` 中，配置校验失败
3. 用户入口发起一次真实请求后：
   - `current_context.role_prompt.role_id == "system"`
4. project runtime 的默认 role 仍为 `project`
5. 当前文本 channel / Web frontstage 的展示不能再与真实 runtime role 脱钩
