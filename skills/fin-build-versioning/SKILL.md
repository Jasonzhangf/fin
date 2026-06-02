---
name: fin-build-versioning
description: Standard build, regression, install, promote, rollback, and versioning workflow for fin. Use when discussing or implementing auto regression on build, build version bumping, install promotion, rollback, or future core/module version topology.
---

# fin Build + Versioning Skill

## 1) Intent

用于统一 `fin` 的编译、回归、安装、提升、回滚、版本号讨论与实现入口。

本 skill 只回答：
1. 哪个版本号属于哪个层级
2. 哪个命令才算“正式 build”
3. build 时必须自动跑哪些回归
4. build / install / promote / rollback 的证据落点在哪

## 2) Canonical sources

1. `AGENTS.md`
2. `docs/architecture/15-install-build-regression-flow.md`
3. `docs/architecture/13-config-and-provider-foundation.md`
4. `docs/architecture/14-runtime-home-layout.md`
5. `docs/architecture/08-testing-and-ci-strategy.md`
6. 对应 CLI / install / harness 源码

## 3) Version layers

`fin` 当前至少区分三层版本概念：

1. **Cargo crate semver**
   - 服务 Rust/Cargo 编译生态
   - 必须满足 Cargo 约束
   - 例如 `0.1.1`
   - 不能使用 `0.1.0001`

2. **fin build version**
   - 服务构建、安装、展示、回滚
   - 当前建议格式：`0.1.0001`、`0.1.0002`
   - build version 是产品级构建编号，不等价于 Cargo crate version

3. **future module versions**
   - 当前只作为架构预留，不提前复杂实现
   - 未来允许：
     - core binary / core runtime version
     - feature module version
     - provider module version
     - projection / web module version
   - 目标是支持逐模块升级，而不是每次整体一起升级

## 4) Standard rule

### 4.1 什么命令才算正式 build

正式 build / install / promote 不能等价于裸：
- `cargo build`
- `cargo test`

这些只算开发期局部检查。

正式交付入口必须是统一 build flow，例如：
- `fin build-dev`（当前推荐）
- `fin install-dev`（当前兼容别名）
- `fin promote <build-version>`

只有统一入口才允许：
- 自动 bump build version
- 自动生成回归 run id / report
- 自动决定是否 promote
- 自动更新 current / previous / receipt

### 4.2 自动回归是 build flow 的一部分

规则：
- 每次正式 build 都必须自动跑与改动层级匹配的最小回归集
- 不允许“先编译，再手动想起回归”
- 不允许“回归散落在不同临时命令里”

### 4.3 自动 bump 的对象

当前默认只自动 bump：
- `fin build version`

当前不要求每次 build 自动 bump：
- Cargo crate semver
- future module versions

这些版本由发布/模块演进策略单独控制。

## 5) Default build flow

1. 分配新的 `build version`
2. 生成 `build-id` / run-id / receipt 占位
3. 跑编译前门禁
4. 执行编译与 workspace tests
5. 执行自动回归
6. 写 reports / logs / receipts
7. 只有全部通过才 promote 到 current
8. 保留 previous 供 rollback

## 6) Minimal automatic regression set

正式 build 默认至少自动覆盖：

1. line-limit gate
2. `cargo fmt --check`
3. `cargo test --workspace`
4. provider config / contract smoke
5. isolated runtime-demo smoke
6. projection / context artifacts smoke
7. installed-binary smoke

若改动影响 provider / runtime / debug：
- 必须包含真实或半真实 provider regression
- 必须验证 `current_context.json` / `current_projection.json` / `latest_events.jsonl`
- 网络型 provider smoke 允许显式、有日志的 bounded retry（当前 install flow 默认 3 次），但不得静默吞错或私自改走别的路径

## 7) Evidence sinks

- build / install logs：`~/.fin/logs/install/`
- regression logs：`~/.fin/logs/regression/`
- regression reports：`~/.fin/harness/reports/<build-id>/`
- receipt index：`~/.fin/harness/reports/<build-id>/receipt-index.json`
- install versions：`~/.fin/install/versions/<build-id>/`
- current / previous / receipt：`~/.fin/install/`
- runtime current metadata：`~/.fin/runtime/current/`

补充规则：
- `install-dev / build-dev` 完成后，默认应刷新 `receipt-index.json`
- `install_smoke` receipt 至少应带出 `session_id / task_id / operation_id / verified_paths`

## 8) Future modular versioning guidance

当前先冻结边界，不提前实现复杂版本编排。

后续若进入模块化升级阶段，优先采用：
- **build version**：表示整次构建/安装产物
- **core version**：表示稳定核心 runtime / binary
- **module version map**：表示各可插拔模块版本

建议关系：
- build version = 一次组装结果
- core/module versions = 组装内容

因此未来可以：
- 同一 core version 下升级某个模块
- 不改 core binary，仅替换某个模块版本
- 按 module scope 做 targeted rollback

## 9) Anti-patterns

- 把 Cargo crate version 当成产品 build version
- 在裸 `cargo build` 上塞自动改版本逻辑
- build 成功就直接 promote，不跑自动回归
- 回归不隔离 runtime home / session namespace
- 还没进入模块化阶段，就先做复杂版本编排系统
