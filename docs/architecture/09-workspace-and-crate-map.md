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

## owning crate contract

| crate | owning truth | allowed deps | forbidden responsibility |
| --- | --- | --- | --- |
| `fin-shared` | 纯函数、底层 typed error、ID/time 小工具 | 无上层 crate | IO、provider、runtime 状态 |
| `fin-contracts` | operation/event/session/tool/prompt schema | `fin-shared` | 网络调用、状态推进、projection 聚合 |
| `fin-config` | user/system config、provider/role 映射、标准化 | `fin-contracts`、`fin-shared` | provider 执行、runtime fallback 默认值 |
| `fin-provider` | provider descriptor、registry、wire request/response、HTTP client | `fin-config` | session/closure 状态、tool dispatch、Web projection |
| `fin-runtime` | agent runtime、pipeline、closure、session materializer、tool dispatch、error center | `fin-config`、`fin-contracts`、`fin-provider`、`fin-shared` | CLI 参数解析、HTTP server、Web state truth |
| `fin-orchestrator` | dispatch policy、lease/ownership 推进策略 | `fin-contracts` | 复制 runtime session truth、直接写 projection |
| `fin-registry` | worker/node 注册发现 | `serde` | 推理执行、session artifacts |
| `fin-transport-http` | HTTP/WS transport binding | transport-local deps | 业务状态机、错误分类 |
| `fin-harness-core` | replay、fault injection、assertion harness | `fin-contracts` | 重新定义协议、修复运行时错误 |
| `fin-debug-server` | debug projection、raw artifact/API read model | `fin-contracts`、`fin-runtime` | 执行真相、runtime failure 二次分类 |
| `fin-cli` | 本地开发入口、命令参数、人工触发流程 | 上层入口依赖 | 拥有业务协议或错误中心 |

跨 crate 错误规则：各 crate 可以保留本层 typed error，但错误跨入 `fin-runtime` 主推理链后，必须归一到 `RuntimeError -> ErrorErr01..05`，并产出 event / ledger / user-visible decision。

## runtime module target layout

`fin-runtime` 允许继续承载 M1 主路径，但目标布局必须收口为 domain 入口：

```text
runtime/src/
  lib.rs
  pipeline/{input,reason,feedback,error}.rs
  closure/{run,round,retry,events,finalize,state}.rs
  context/{view,blocks,render,project,history}.rs
  tools/{catalog,dispatch,query,patch,exec,task,collab,peer}.rs
  session/{materializer,journal,turn,trace}.rs
  control/{feedback,plane,routing,scheduler,owner_loop}.rs
```

迁移规则：

1. 目录迁移必须保持唯一主路径；禁止旧文件 re-export 新文件形成双路径。
2. 迁移后物理删除旧模块文件。
3. `extended` / `v4a` / 泛化 `helpers` 不允许作为最终模块名。
4. `pipeline` 节点只允许相邻 builder/parser 转换；跨 domain 只能消费链尾输出。

## 拆分原则

1. 优先按职责拆，不做单体 core crate。
2. `config` 与 `provider` 先稳定，再往上叠 runtime / debug / task 能力。
3. 通用逻辑必须下沉，避免 orchestrator / web 双写。
4. 单 crate 过大时优先继续拆 crate 或模块，而不是继续堆。
5. 发现 dead semantic / duplicate design / wrong implementation 后必须物理删除，不能靠“不接入”长期保留。
6. Provider hub skeleton 若不进入 `fin-provider` 执行真源，就必须删除；若进入主路径，必须消除 dead_code warning。
