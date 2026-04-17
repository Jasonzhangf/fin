# 02 Layer Boundaries

## 分层

### A. Contracts / Shared
- 拥有：协议类型、事件结构、纯函数、错误模型、ID 与时间工具
- 禁止：网络 IO、任务编排、UI 投影

### B. Runtime / Registry / Transport
- 拥有：worker 生命周期、agent runtime、发现与注册、HTTP/WS transport
- 禁止：把 Web 观察视图当作状态真源

### C. Orchestrator
- 拥有：任务分发、状态推进、lease / ownership 组合策略
- 禁止：复制 contracts 或 transport 真相

### D. Harness / Debug Server
- 拥有：事件录制、回放、故障注入、projection 聚合
- 禁止：重新定义 runtime 状态机

### E. Web
- 拥有：timeline、trace、task graph、raw event inspector
- 禁止：定义执行真相、偷偷维护第二份状态

## owning layer 规则

1. 协议字段改动先落 `contracts`
2. 公共纯逻辑先落 `shared`
3. 运行时生命周期先落 `runtime`
4. 分发与推进策略先落 `orchestrator`
5. transport 细节只在 `transport-http`
6. 可视化只在 `web`