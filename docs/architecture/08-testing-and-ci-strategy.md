# 08 Testing and CI Strategy

## 测试分层

### L1 Unit
- 纯函数
- 状态机转移
- 序列化/反序列化

### L2 Contract
- HTTP / WebSocket schema
- event envelope 兼容性

### L3 Harness
- 录制 / 回放
- 故障注入断言

### L4 Cluster Simulation
- 多 worker / 多 node / 跨网段模拟

### L5 Manual Debug Validation
- Web 调试后台观察
- raw event 与 timeline 核对

### L6 Installed-binary Smoke
- 通过 `~/.fin/bin/fin` 验证安装态入口
- 验证 `~/.fin` 目录写入是否符合预期

## 功能测试固定顺序

开发阶段先保证基础能力通，再向上叠加。默认顺序冻结为：

1. **G0 Code gate**
   - governance 结构检查
   - 非白名单代码文件 `< 500` 行
2. **P1 Provider config smoke**
   - 默认测试 provider 来源：`~/.rcc/provider/ali-coding-plan/config.v2.json`
   - 默认测试 model：`qwen3.6-plus`
   - 默认协议：`anthropic-wire`
   - 先生成隔离测试用 `user.toml`
   - 先验证 config mapping / provider descriptor / model target 是否正确
3. **P2 Provider slice smoke**
   - provider request build / normalize / timeout/failure contract
   - 在 Step 3 之前，P2 只要求 mock/contract，不宣称真实联网成功
4. **R1 Runtime builder**
   - inference operation builder
   - runtime policy -> operation contract
5. **R2 Recording / Projection**
   - progress / note / digest / projection
6. **D1 Debug / Install**
   - debug projection
   - installed-binary smoke

规则：

- 先把下层打通，再叠加上层逻辑
- 不允许一边 provider 基础不稳，一边继续堆 task / session / collaboration 复杂语义
- 当前 M1 阶段，真实 provider 调用只在 provider slice 落地后进入固定回归

## CI 门禁顺序

1. governance 结构校验
2. code line-limit gate（默认 500 行，白名单例外）
3. cargo fmt / cargo test
4. provider config / contract smoke
5. contract / replay smoke
6. installed-binary smoke
7. 后续再扩展 cluster simulation

## 原则

- 先内后外
- 先最小验证再扩展
- 如果改动影响 Web 调试链路，必须至少验证 raw event 与 projection 一致
- 测试必须使用隔离 test `user.toml` + test runtime home + test session namespace
- 测试与正常会话必须在命名和路径两侧同时隔离


## 运行时证据落点

- 回归报告：`~/.fin/harness/reports/`
- 回归日志：`~/.fin/logs/regression/`
- 安装日志：`~/.fin/logs/install/`
- 目录布局真源：`docs/architecture/14-runtime-home-layout.md`
- 安装流程真源：`docs/architecture/15-install-build-regression-flow.md`
- test provider config 生成脚本：`scripts/generate-test-user-toml.py`
- 真实 anthropic provider probe：`scripts/probe-anthropic-provider.py`
- code line-limit gate：`scripts/check-code-line-limit.py`


## 本轮新增观测回归点

在 provider / runtime / web debug 相关改动后，最少补以下检查：

1. `current_projection.json` 是否包含最新 provider 活动、UA、header names
2. `current_context.json` 是否包含本轮真实 input/context/provider path
3. `recent_contexts.json` 是否保持 bounded window，而不是无界增长
4. 真实 provider 回归必须走隔离 runtime home + test session namespace
5. 若需要开 Web debug，只允许前台启动并在测试结束后自然退出；不依赖后台悬挂进程
6. 多轮 `transcript-demo` 场景要验证 recent context continuity，并至少有一条真实 provider 闭环证据
