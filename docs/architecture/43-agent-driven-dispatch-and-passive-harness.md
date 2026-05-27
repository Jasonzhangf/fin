# Agent-driven Dispatch and Passive Harness

## 索引概要
- L1-L8 `purpose`：定义 agent-driven dispatch 与 passive harness 边界。
- L10-L21 `core-model`：正确的 system/project 协作主链。
- L23-L35 `framework-boundary`：框架能做什么、不能做什么。
- L37-L54 `current-audit`：当前错误设计审计。
- L56-L72 `required-fixes`：补齐措施。
- L74-L88 `verification`：必须通过的验证矩阵。

## purpose
这份文档定义 `user request -> system reasoning -> dispatch tool -> project execution -> progress/result -> system next action` 的唯一正确协作主链。

## core-model
1. 默认入口只有 `system agent`。
2. user request 先进入 system agent 的 session / ledger truth。
3. system agent 自己推理：
   - 是否能在当前 cwd/权限边界内完成。
   - 若不能，是否需要调用 `dispatch` / `agent.assign` 工具。
4. dispatch 是模型主动工具调用，不是 harness 预设动作。
5. project agent 接到任务后，在自己的 cwd 执行，并持续上报：
   - progress
   - tool turn / event refs
   - result / failed / timeout
6. system agent 接收这些事实后继续推理，决定：
   - 等待
   - 查询
   - 继续派发
   - 收口答复用户
7. project agent 内部 spawn 的 subagent 默认只在自己作用域可见；对外只暴露 progress/result truth。

## framework-boundary
framework 只允许提供：
1. tool catalog
2. mailbox / transport
3. permission boundary
4. event / ledger / receipt truth
5. control block 触发条件
6. retry / backoff / timeout policy

framework 禁止做：
1. 替 system agent 决定是否 dispatch
2. 在 system 推理前硬编码 dispatch payload 业务语义
3. 绕过模型直接把任务塞给 project agent
4. project 完成后替 system agent 直接收口总结
5. 用 self-loopback 或临时文件假装真实 agent 协作语义

## current-audit
当前实现仍存在以下错误设计：

1. `rust/crates/cli/src/local_multi_agent_lifecycle_harness.rs`
   - harness 在 `rpc_send_dispatch(...)` 之前就写入 `task.dispatch.sent/received` 事件。
   - 这意味着 dispatch 语义不是来自 agent tool call，而是来自 harness 预编排。

2. `rust/crates/cli/src/local_multi_agent_rpc.rs`
   - `rpc_send_dispatch(...)` 当前把消息从 `local.system` 发给 `local.system`。
   - payload `kind=user_request`，属于 system self-loopback。
   - 这不是“system 经过推理后 dispatch 给 project”，只是 harness 借 mailbox 驱动本地脚本。

3. `rust/crates/cli/src/local_multi_agent_node.rs`
   - `handle_system_inbox(...)` 只处理 `project_result`，没有完整的“用户请求 -> system 推理 -> dispatch tool -> project progress supervision”主链。
   - 当前 system summary 仍是固定 `user_summary: dispatch project task`，不是模型真实生成的对话闭环。

4. 当前 harness 虽然已经具备 mailbox / wait / progress / result / reconnect / disconnect 等 transport/lifecycle 骨架，
   但它仍过度持有业务语义，违反“framework 被动、agent 主动”的设计。

## required-fixes
1. ingress 真相修正
   - user request 必须首先写入 system session/ledger，作为真实用户输入。

2. dispatch 真相修正
   - 只能在 system agent 真实推理后，通过工具调用产生 dispatch。
   - dispatch payload 的 `task_description`、目标、报告要求必须来自模型输出。

3. progress 真相修正
   - project agent 必须周期性上报 progress/tool-turn refs。
   - system agent 必须有可观察的 waiting/supervision 状态推进。

4. completion 真相修正
   - project result 返回后，必须再经过 system agent 一轮推理决定后续动作。
   - harness 不得直接 synthesise 最终 summary。

5. prompt/tool 真相修正
   - system role prompt 必须明确：跨 cwd/跨项目任务应调用 dispatch tool。
   - tool schema 必须显式要求 `task_description` 为模型必填字段。

6. harness 角色收敛
   - harness 只保留：启动实例、注入原始 user request、执行 transport、等待事件、收据落盘。
   - 所有任务拆解语义从 harness 剥离到 prompt + tool + runtime truth。

## verification
必须至少通过以下验证：
1. 单元/契约
   - dispatch tool schema 包含模型必填 `task_description`。
   - system/project mailbox seq 单调递增。

2. 本地双实例静态 E2E
   - user request 先进 system。
   - system 产生 dispatch tool truth。
   - project 产生 progress/result。
   - system 基于 result 收口。

3. 本地双实例真实 LLM E2E
   - 使用真实 provider。
   - 同样完整闭环。
   - receipt 必须能证明 dispatch 业务语义来自模型，而非 harness 常量。

4. 构建交付
   - `bash scripts/install-fin-global.sh`
   - 编译、测试、安装、回归通过。
