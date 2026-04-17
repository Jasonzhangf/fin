---
name: fin-testing-harness
description: Testing and harness workflow for fin. Use for replay design, fault injection, and CI validation planning.
---

# fin Testing + Harness Skill

## 1) Intent

用于确定测试层级、回放方式、故障注入矩阵以及 CI 最小门禁。

## 2) Canonical sources

1. `docs/architecture/07-harness-replay-fault-injection.md`
2. `docs/architecture/08-testing-and-ci-strategy.md`
3. `docs/architecture/15-install-build-regression-flow.md`
4. 相关 replay / fixture 文件

## 3) Workflow

1. 先确定改动影响的层级
2. 先按固定顺序选最小测试集：`provider config -> provider slice -> runtime builder -> recording/projection -> debug/install`
3. 先决定证据写入 `~/.fin/harness/reports/` 与 `~/.fin/logs/regression/` 的落点
4. 测试必须使用隔离的 test `user.toml`、test runtime home、test session namespace，禁止污染正常 `~/.fin/sessions/*`
5. 若跨 runtime / transport 边界，必须补 harness 场景
6. 若修复可复现故障，优先沉淀 replay 场景


## 3.1) Module test baseline（路由到全局 skill）

模块级测试闭环的通用分层统一遵循全局 `coding-principals` 中的：

- Unit
- Function / Contract
- Orchestration Regression
- Installed-binary / Runtime Smoke

本地只补 `fin` 的落点要求：

- 回归报告进入 `~/.fin/harness/reports/`
- 回归日志进入 `~/.fin/logs/regression/`
- 影响 runtime / provider / debug 链路的改动，优先补 replay / harness smoke
- provider 真实/半真实测试默认使用 `~/.rcc/provider/ali-coding-plan/config.v2.json` 生成 test `user.toml`
- 默认 provider/model 冻结为 `ali-coding-plan` / `qwen3.6-plus`，协议为 `anthropic-wire`
- 测试 session 必须带 `test-` 命名空间，并写入 `~/.fin/harness/runs/<run-id>/...` 的隔离目录

## 4) Minimal validation matrix

- L1：unit
- L2：contract
- L3：replay / harness smoke
- L4：cluster simulation
- L5：manual debug observe

## 5) Anti-patterns

- 跳过 replay 直接依赖人工排查
- 没有 raw event 就写 Web 调试结论
- 故障注入只看 UI，不看结构化事件
- 只跑 unit 就宣称模块可交付
- 用正常 `~/.fin` 家目录跑测试，导致 session / log / projection 污染
