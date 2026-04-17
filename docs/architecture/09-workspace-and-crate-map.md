# 09 Workspace and Crate Map

`rust/` 采用 workspace，crate 命名统一 `fin-*`。

## v1 crate 规划

- `fin-config`：用户配置、系统配置、映射与标准化
- `fin-provider`：多协议 provider 抽象、注册与调用
- `fin-contracts`：协议、事件、状态枚举
- `fin-shared`：纯函数、错误、ID/时间工具
- `fin-runtime`：agent runtime 抽象与生命周期
- `fin-orchestrator`：任务分发与推进策略
- `fin-registry`：worker / node 注册与发现
- `fin-transport-http`：HTTP + WS transport
- `fin-harness-core`：录制、回放、断言、故障注入
- `fin-debug-server`：调试视图聚合
- `fin-cli`：开发与调试入口

## 拆分原则

1. 优先按职责拆，不做单体 core crate。
2. `config` 与 `provider` 先稳定，再往上叠 runtime / debug / task 能力。
3. 通用逻辑必须下沉，避免 orchestrator / web 双写。
4. 单 crate 过大时优先继续拆 crate 或模块，而不是继续堆。