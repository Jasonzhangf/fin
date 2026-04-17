# fin

fin 是一个以 **Rust runtime + Web 调试后台** 为核心的多 agent 框架骨架仓库。

当前阶段优先建立：

- 项目级 `AGENTS.md` 路由入口
- 本地开发 `skills/`
- `docs/` 设计真源
- Rust workspace 骨架
- harness / web / fixtures / CI 基础目录

设计原则：

- Rust 负责执行真源
- Web 负责观察与调试
- 结构化事件是 runtime / web / harness / CI 的共同真源
- 先最小框架，后分支能力

当前 M1 已具备：

- `fin-cli home-init` 初始化 `~/.fin`
- `fin-cli runtime-demo` 产出单 runtime 事件闭环
- `fin-cli debug-projection` 输出当前投影
- `fin-cli web-debug` 启动最小 Web Debug MVP
