# M1 Known Gaps and M2 Backlog

本文档只记录两类内容：

1. **M1 已知但非阻塞的缺口**
2. **明确延期到 M2 的 backlog**

目的不是继续扩张，而是把“不做的东西”正式写清楚。

## 1. M1 已知缺口（非阻塞）

### 1.1 installed-binary smoke 自动化已补齐，剩余只是在持续使用中防回归

现状：

- 当前已有 repo 内 `cargo test` 基线
- 已有一次 manual receipt：`docs/closeout/m1-receipts-2026-04-19.md`
- `install-dev` / `build-dev` 当前默认会刷新 `harness/reports/<build>/receipt-index.json`
- install smoke summary 已带出 `session_id / task_id / operation_id / verified_paths`

影响：

- 不再阻塞 M1 closeout
- 后续重点变成“持续保证 receipt 不漂移”

建议：

- 保留 manual receipt 作为 closeout 历史证据
- 正式 build/install 以后默认检查 `receipt-index.json` 是否存在且 `install_smoke=passed`

### 1.2 正式 build/install gate 已恢复，后续重点变为 receipt 标准化

现状：

- `install-dev` 正式入口已真实执行并通过
- line-limit / fmt / test / install 当前都已打通
- 当前缺的不是 gate 可用性，而是 closeout receipt 的进一步标准化与持续同步

建议：

- 后续把 install/build 结果继续纳入更稳定的 closeout receipt 流程

### 1.3 archive summary / truth consistency 仍需持续盯防

现状：

- latest-per-type 聚合规则已确定
- 多轮 operation 摘要已有修正

风险：

- 若后续又让 Web 侧重新“自行推导”，会再破坏 truth

建议：

- closeout 后把 archive summary consistency 当作固定回归点

## 2. 明确进入 M2 的 backlog

### 2.1 多 agent 执行面

包括：

- project agent 真执行链
- 跨机 agent 协作
- remote peer 生命周期
- distributed mailbox / eventbus

### 2.2 detached / headless daemon

包括：

- 真正后台常驻 supervisor
- 独立 lease / recovery loop
- orphan-safe lifecycle management

### 2.3 真正可恢复的 pause/resume

包括：

- 中断后从中间步骤继续
- provider/tool loop 的 resumable execution
- 更细粒度 execution stack checkpoint

### 2.4 普通推理并行执行

说明：

- 当前只支持 `status_probe` 这类 non-interrupting inquiry
- 普通用户输入的真正并行推理不在 M1

### 2.5 session / task / topic 产品化状态机

包括：

- tentative session -> formal task 的完整产品交互
- session 复活、切换、修正、复用的完整 UI/控制面
- 用户确认流与 reuse/switch 的系统化闭环

### 2.6 mature memory / knowledge graph

包括：

- digest / note / knowledge artifact 的系统化抽取
- project wiki / graph / truth pruning
- 历史认知纠错与唯一真源合并

### 2.7 peer / channel / gateway 体系

包括：

- qqbot 内置 channel peer
- channel pairing / credential lifecycle
- 非智能 peer capability center
- 智能 peer 统一接入协议

## 3. M2 backlog 的进入条件

只有满足以下条件之一，才允许从 M1 closeout 切回 M2 扩展：

1. M1 回归矩阵稳定
2. closeout 文档完成
3. 当前阻塞 M1 的缺口清零

否则默认继续收口，而不是扩张。

## 4. closeout 期间的执行规则

1. 如果某需求不阻塞 M1，可先记到本文档
2. 不用“顺手实现”来替代 backlog 记录
3. backlog 进入实现前，先回到 architecture docs 重新冻结 owning layer
