# M2 Merge Regression Closeout — 2026-06-03

本文档记录 `codex/pipeline-unique-type-integrated` 合并回 `main` 后的回归收口结论。

## 1. 目标

1. 将 `pipeline unique type` 重构（含 reason/feedback/error/hub 链节点锁定）合并到 `main`。
2. 真实 provider live smoke 在 `main` 上恢复闭环（reasoning + app config + upgrade path 三条主链全部能跑真实 LLM）。
3. 修复合并过程中暴露的 silent drop / silent bug。

## 2. 完成判定

| 维度 | 证据 |
| --- | --- |
| merge 到 main 无冲突 | `dda65f2 merge: pipeline unique type + 14/14 local regression receipt`（合并 commit 在 `main` 上） |
| upstream main 与本地一致 | `origin/main = 7fbba93908978a34b0f9a3617d8d69a6a6dd4a16`（已 push） |
| mainline receipt 入仓 | `reports/regression/mainline-pipeline-e2e/mainline-receipts.json`（14/14 green） |
| provider 单测 | `cargo test -p fin-provider` → 13/13 passed |
| 真实 provider live smoke | `mini27`（OpenAI-compatible），3 turns，8 provider request，status=200，`stop_reason=stop`，`reasoning.stop=true` |
| 端到端 receipt | `/Users/fanzhang/.fin/harness/runs/test-merge-mini27-20260603-034345/provider-live-smoke-report.json` |

## 3. 合并中暴露并修复的真实问题

### 3.1 `execute_prepared` 丢 OpenAI 协议分支

pipeline unique type 重构时只保留了 `AnthropicWire` 分支，`ProviderProtocol::OpenAiCompatible` 走到 `UnsupportedProtocol`。
这导致 `provider-live-smoke` 一打开就失败，但 g3_live_provider_smoke 之前被认为是 green 的事实来自更早的合并 commit，根因是**重构吞掉了协议分支**而非"协议还没实现"。
修复：`ProviderFacade::execute_openai_compatible`（POST `/chat/completions`、Bearer header、`parse_openai_response`），并通过 TCP-Listener 单测锁住 endpoint + header + payload。

### 3.2 429 / 5xx / quota 错误不重试

`status >= 400` 走 `HttpStatus` 直接返回，违反项目硬护栏 #15（瞬发 provider 配额/限流错误必须自动指数回退 5 次）。
修复：`http_client::classify_http_status` + facade 中 5 次尝试 + `1s*2^(attempt-1)` 回退；`MAX_REQUEST_ATTEMPTS` 3→5。
覆盖：429 / 503 / 5xx 一定重试；quota/usage_limit/余额不足 body 与 400 一起识别为重试；干净 400 不重试。
单测：`classify_http_status_retries_rate_limit_and_quota_errors`。

## 4. 仍存在的债务

- 静态 CI 还没有把"execute_prepared 必须覆盖全部已注册协议"作为不变量检查；本轮靠 live smoke + 单测兜底。下一个迭代应在 `verify-governance.sh` / `provider_static.rs` 增一道红线。
- live provider 等待 3 turns ~ 7 分钟，仍未按阶段设置 timeout（connect / waiting provider / tool wait 分开），应作为 M2 改进项。
- `execute_openai_compatible` 未做 streaming、tool calls、usage 反向回填；当前只覆盖 simple chat/completions。

## 5. 进入 M2 的下一步

- 把 `docs/architecture/44-pipeline-unique-type-and-error-chain.md` 提到的"已发布节点不可重排/不可复用/不可改语义"作为 e2e 不变量落入 `scripts/verify-governance.sh`。
- 给 `provider-live-smoke` 加 `--phase-timeout` 显式控制 connect / waiting provider / tool wait。
- 把"execute_prepared 协议分支静态覆盖"作为 `fin-provider` 的 new test。
