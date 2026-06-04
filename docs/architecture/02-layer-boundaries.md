# 02 Layer Boundaries

## 分层

### A. Contracts / Shared
- 拥有：协议类型、事件结构、纯函数、错误模型、ID 与时间工具
- 禁止：网络 IO、任务编排、UI 投影
- 边界：`contracts` 只定义可跨进程 / 跨 crate 传播的 schema；`shared` 只放无状态纯函数与底层 typed error。

### B. Runtime / Registry / Transport
- 拥有：worker 生命周期、agent runtime、发现与注册、HTTP/WS transport
- 禁止：把 Web 观察视图当作状态真源
- 边界：runtime 拥有推理链、session materialization、tool dispatch、错误链入口；registry 只负责发现与注册；transport 只负责 wire 收发。

### C. Orchestrator
- 拥有：任务分发、状态推进、lease / ownership 组合策略
- 禁止：复制 contracts 或 transport 真相
- 边界：orchestrator 只能发出 operation / consume event，不得直接改 runtime session artifact。

### D. Harness / Debug Server
- 拥有：事件录制、回放、故障注入、projection 聚合
- 禁止：重新定义 runtime 状态机
- 边界：harness/debug 只能验证或投影 runtime events / artifacts，不得补业务语义。

### E. Web
- 拥有：timeline、trace、task graph、raw event inspector
- 禁止：定义执行真相、偷偷维护第二份状态
- 边界：Web 只展示 debug-server projection / raw events；所有可见错误来自 runtime error event / ledger。

## Runtime 内部 domain 边界

`fin-runtime` 当前是 M1 主执行真源，但内部必须继续按 domain 隔离，禁止把所有逻辑堆到 `lib.rs` 或 `tool_dispatch_extended_*`。

| domain | 拥有 | 禁止 |
| --- | --- | --- |
| `pipeline` | `InputIn*`、`ReasonReq/Resp*`、`FeedbackResp*`、`ErrorErr*` 节点与相邻 builder/parser | 跨节点 shortcut、散落 `From`、同义 DTO |
| `closure` | 单轮/多轮 closure 生命周期、retry、round record、finalize | 直接定义 provider wire schema 或 Web projection |
| `context` | context view、block render、project support、tool/history render input | 从 provider response 反推 session truth |
| `tools` | tool catalog、tool dispatch、tool result receipt、tool semantic render | 吞 tool error、把失败伪装为成功 tool result |
| `session` | session materializer、journal、turn/trace/ledger record | 覆盖写历史、跳过 event 直接改 projection truth |
| `control` | control feedback、routing action、scheduler/owner-loop decision | 用默认值替代模型/运行时失败 |
| `error` | `RuntimeError -> ErrorErr01..05 -> event/ledger/channel` 唯一路径 | fallback 成成功、CLI/debug 二次分类 runtime failure |

命名收口规则：

1. Pipeline 节点必须使用 `<Domain><Direction><NN><Node>`。
2. 临时版本名禁止进入主路径：`extended`、`v4a`、`support`、`helpers` 只能作为迁移待清理信号，不能作为长期 owning module 名。
3. 每个 domain 只能有一个公开入口模块；内部 helper 不得被跨 domain 直接调用。
4. 文件迁移完成后必须物理删除旧文件名，禁止保留空壳 re-export。

## owning layer 规则

1. 协议字段改动先落 `contracts`
2. 公共纯逻辑先落 `shared`
3. 运行时生命周期先落 `runtime`
4. 分发与推进策略先落 `orchestrator`
5. transport 细节只在 `transport-http`
6. 可视化只在 `web`
7. 错误进入 runtime 后必须走 `ErrorErr*` 链，见 `docs/architecture/44-runtime-error-center.md`
