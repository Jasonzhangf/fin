# 15 Global Install, Build, and Regression Flow

本文档定义 `fin` 的标准开发闭环：

1. 源码编译
2. 版本化安装
3. 回归测试
4. 提升为全局默认版本

目标不是给出最终命令清单，而是冻结流程边界与产物落点。

## 1. 总原则

### 1.1 全局安装走 `~/.fin/bin`

`fin` 的“全局安装”默认定义为：

```text
PATH 包含 ~/.fin/bin
~/.fin/bin/fin -> ~/.fin/install/current/bin/fin
```

好处：

- 不依赖系统级安装目录
- 用户只做一次 PATH 配置
- 每次升级 / 回滚只切换家目录内软链接
- 适合本地开发与频繁迭代

### 1.2 编译结果、安装物、回归证据分开存放

- repo 内保留源码编译临时物（如 `rust/target/`）
- `~/.fin/install/` 保留版本化安装物
- `~/.fin/harness/reports/` 保留回归报告
- `~/.fin/logs/install/` 与 `~/.fin/logs/regression/` 保留日志

### 1.3 未经过回归验证，不得提升为 current

这是标准硬规则：

- 编译成功 ≠ 可安装
- 单测通过 ≠ 可提升
- 必须通过当前变更所需的最小回归矩阵，才能切换 `current`

## 2. Build ID 与版本化安装目录

每次候选构建都应生成一个 `build-id`，建议至少包含：

- 时间戳
- git commit short sha
- 可选 profile / channel

例如：

```text
20260417-abc1234-dev
```

安装目录：

```text
~/.fin/install/staged/<build-id>/
~/.fin/install/versions/<build-id>/
```

## 2.1 Cargo semver 与 fin build version 必须分层

Cargo crate version 必须满足 Rust/Cargo 的 semver 约束。

因此：

- `0.1.0001` 不能直接写进 `Cargo.toml` 作为 crate version
- `0.1.0001` 应定义为 `fin` 的 build version / install version / display version
- Cargo crate version 与 fin build version 不是同一层语义

当前建议：

- Cargo crate semver：保持合法 semver，例如 `0.1.1`
- fin build version：从 `0.1.0001` 开始，按统一 build flow 自动递增

## 2.2 自动回归必须挂在统一 build flow

规则：

- 正式 build 不等于裸 `cargo build`
- 正式 build / install / promote 必须走统一入口
- 自动回归、自动 version bump、report/receipt 生成都挂在统一入口，而不是散落到人工命令

这也是未来支持：

- core 稳定后独立编译/发布
- feature/module 逐模块升级
- targeted rollback

的前置条件。

## 2.3 全局安装脚本

全局安装脚本的唯一入口是：

```bash
scripts/install-fin-global.sh [~/.fin/config/user.toml]
```

该脚本必须：

1. 先构建 release `fin-cli`，再调用 `fin-cli install-dev`，不得手动复制二进制绕过正式 build flow。
2. 安装完成后只创建用户级 symlink：`${FIN_GLOBAL_BIN_DIR:-~/.local/bin}/fin -> ~/.fin/bin/fin`。
3. daemon 重启只能调用 `fin stop` / `fin start`，禁止 `killall` / `pkill` / `kill $(...)` / `xargs kill`。
4. 首次安装在 macOS 上调用 `scripts/bootstrap-macos-permissions.sh`，只负责触发/打开权限授权入口并记录 marker，不得伪造系统授权结果。

## 2.4 首次安装权限 bootstrap

macOS TCC 权限（辅助功能、屏幕录制、输入监听、完全磁盘访问等）不能被普通 CLI 静默授予。

fin 的正确策略是：

1. 首次全局安装时自动触发 Apple Events 检查并打开相关 System Settings Privacy pane。
2. 写入 `~/.fin/install/macos-permissions-bootstrap.json`，表示权限 bootstrap 已经展示过。
3. 后续安装不重复弹出；如需重新打开，使用 `FIN_FORCE_PERMISSION_BOOTSTRAP=1 scripts/bootstrap-macos-permissions.sh`。
4. 若运行时仍缺权限，必须明确报错并指向 marker/log，不允许吞异常或假装权限已获得。

## 3. 标准闭环阶段

## Phase A：源码校验

目标：确保源码层最小正确性。

最小校验集合：

1. governance / 文档结构检查
2. code line-limit gate（默认 500 行，白名单例外）
3. `cargo fmt --check`
4. `cargo clippy`（至少 workspace 关键 crate）
5. `cargo test`
6. 必要时 `cargo build --release`

产物：

- 编译/测试日志写入 `~/.fin/logs/install/`

说明：

- 这一阶段仍是 repo 内构建视角
- 还没有形成“可推广的全局安装版本”

## Phase B：候选安装物 staging

目标：把候选构建物组织成可提升版本。

落点：

```text
~/.fin/install/staged/<build-id>/
```

至少包含：

```text
bin/
manifest.toml
checksums.txt
```

说明：

- `manifest.toml` 用于记录 build 来源、commit、profile、时间、crate 版本
- staging 成功后，才进入安装态回归

## Phase C：安装态回归验证

目标：验证“通过全局入口运行的版本”是否真能工作。

这一步比源码层测试更重要，因为它验证的是：

- 安装目录布局是否正确
- 配置查找是否正确
- runtime 是否能在 `~/.fin` 下正常写入数据
- CLI / debug / provider / harness 的实际入口是否可用

回归结果写入：

```text
~/.fin/harness/reports/<build-id>/
~/.fin/logs/regression/
```

测试输入配置与 session 必须隔离：

- test provider config 先生成到 `~/.fin/harness/runs/<run-id>/user.test.toml`
- test runtime home 使用 `~/.fin/harness/runs/<run-id>/runtime-home/`
- test session / task 命名带 `test-` namespace
- 禁止直接用正常 `~/.fin/config/user.toml` 跑回归

### 推荐最小回归层级

#### R0 Governance smoke
- 路由与必要文件存在

#### R1 Code quality / unit
- fmt / clippy / unit test 结果通过

#### R2 Contract smoke
- 关键 schema / envelope / provider config 通过
- provider 默认 smoke 输入来自 `~/.rcc/provider/ali-coding-plan/config.v2.json` 生成的 test `user.toml`
- 默认模型固定为 `qwen3.6-plus`
- 真实协议连通性通过 `scripts/probe-anthropic-provider.py` 做最小网络探测
- 对网络抖动允许显式 bounded retry，但每次 retry 必须写入回归日志，不能静默掩盖失败

#### R3 Installed-binary smoke
- 通过 `~/.fin/bin/fin` 或 staged binary 完成最小启动
- 能正确读取 `~/.fin/config/*`
- 能写入 `~/.fin/runtime/`、`~/.fin/logs/`、`~/.fin/sessions/`

#### R4 Observable smoke
- Web / debug server / event stream 最小可观察链路成立
- 或至少完成等价的 debug/projection smoke
- 多 agent 回归至少要有一条“真实 runtime_home -> WebUI 渲染”证据链；当前标准入口为：
  - `node scripts/webui/live-runtime-chat-smoke.mjs <run-id>`
  - 它必须直接读取本地双实例 E2E 产出的 `runtime-home`，通过 `web-debug` 服务真实 `/api/*`，再由前端 `chat.js` 渲染连续线程与 delegated card
  - 不允许只用合成 JSON 冒充“客户端已验证”

#### R5 Manual targeted validation
- 对本次改动影响最大的链路做人工观察验证

## 3.1 多 agent live gate 分层

对多 agent 闭环，live 回归必须区分两类：

1. **blocking live gates**
   - `scripts/run-local-multi-agent-e2e.sh live <run-id>`
   - `node scripts/webui/live-runtime-chat-smoke.mjs <run-id>`
   - 这两条共同证明：真实 LLM + 本地双实例 + runtime truth + 客户端渲染。

2. **non-blocking live provider slice**
   - `scripts/run-real-provider-smoke.sh <run-id>`
   - 它用于单独观测 provider slice 健康度与额度/配额问题。
   - 若失败原因是外部 quota / weekly limit / provider-side policy，而多 agent live E2E 已通过，则只记录为非阻断告警，不能反过来否定多 agent 主闭环。
   - Jason 额外规则：这类外部错误只有在“**同类错误连续 3 次**”时才可升级为真实阻断；中间一旦成功或错误类型变化，计数清零。
   - 对代码/harness 内部被标记为 retryable 的瞬时错误，统一使用**指数回退最多 5 次，且从 1s 起步**；标准节奏为 `1s/2s/4s/8s/16s`。attempt 必须有日志/receipt 事实，禁止静默线性重试。

规则：

- 不是每次都跑最重矩阵
- 但每次都要跑与改动层级相匹配的最小回归集
- 影响安装 / runtime / provider / debug 的改动，必须包含 R3

## Phase D：提升为 current

只有在 Phase C 通过后，才允许：

1. 将 staging 版本迁入 `versions/<build-id>/`
2. 更新 `previous`
3. 更新 `current`
4. 写入 `receipts/<build-id>.json`

`receipt` 至少记录：

- build-id
- commit
- install time
- regression report 位置
- promoted by
- previous version
- current version

## Phase E：安装后 smoke

提升后仍需要一次轻量 post-install smoke，确认：

- `~/.fin/bin/fin` 指向正确版本
- runtime 能正常启动
- 当前版本信息写入 `~/.fin/runtime/current/`
- 日志路径正常

这一步的结果写入：

- `~/.fin/logs/install/`
- `~/.fin/runtime/current/`

## 4. 回滚原则

回滚不靠重新编译，而优先依赖：

```text
~/.fin/install/previous
```

规则：

1. 保留最近一个已知可用版本
2. promote current 时同步维护 previous
3. 回滚也必须写回执与日志
4. 回滚后仍要做最小 smoke

## 5. 与 CI 的关系

CI 不一定完成“本机全局安装”，但 CI 至少要验证：

- Phase A 的自动门禁
- 与当前改动相关的最小 Phase C 子集

本机开发闭环则负责：

- 真正写入 `~/.fin/install/`
- 真正走 `~/.fin/bin/fin`
- 真正验证家目录落盘与最小运行闭环

## 6. 与 M1 的关系

M1 阶段，标准闭环至少要能证明：

1. 构建出可执行 `fin` 入口
2. 候选安装物能进入 `~/.fin/install/staged/`
3. 通过安装态 smoke
4. 正常写入：
   - `~/.fin/config/`
   - `~/.fin/runtime/`
   - `~/.fin/logs/`
   - `~/.fin/sessions/`
5. 提升后 `~/.fin/bin/fin` 可作为统一入口

## 7. 当前冻结的流程级决策

当前可以视为流程真源的内容：

1. “全局安装”默认通过 `~/.fin/bin` 完成
2. staging 与 promoted version 必须分离
3. 每次可提升版本都必须有 build-id
4. 每次提升前都必须有与改动层级匹配的回归报告
5. 安装、回归、提升、回滚都必须写日志或回执

## 8. 当前非目标

以下留到模块实现阶段再定：

- 更细的 channel（stable/nightly/dev）分支策略
- 是否额外产出系统包（pkg/homebrew/formula）
- 各类回归报告的最终 schema
- channel（stable/nightly/dev）细节
- 自动清理旧版本策略


## 2.3 future modular version topology（预留）

当前阶段不实现复杂模块版本系统，但设计上先冻结三层：

1. build version
   - 表示一次完整构建/安装结果
2. core version
   - 表示稳定 runtime/core binary
3. module version map
   - 表示 provider / feature / projection / web 等模块版本

未来目标：

- 不必每次整体升级全部模块
- 某些模块可独立构建和替换
- rollback 可以按 build version，也可以进一步按模块粒度

当前要求只有一个：

- 今天的 build/versioning 设计不能堵死这条演进路径
