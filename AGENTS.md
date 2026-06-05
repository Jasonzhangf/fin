# AGENTS.md（fin 路由版）

## 索引概要
- L1-L8 `purpose`：本文件只保留入口、护栏、路由。
- L10-L19 `priority-principle`：docs / skills / AGENTS 的职责边界。
- L21-L31 `hard-guards`：项目级硬规则。
- L33-L44 `route-map`：任务路由到 docs 与 skills。
- L46-L52 `mandatory-flow`：执行顺序。
- L54-L58 `evidence-report`：回报要求。

## purpose
`AGENTS.md` 只负责项目入口、硬护栏、路径索引；不承载长流程设计与协议细节。

## priority-principle
1. 架构、协议、状态机、测试分层写入 `docs/`，它们是设计真源。
2. `skills/` 只保留执行适配：归属判断、唯一真源定位、最小验证矩阵、常见反模式。
3. `AGENTS.md` 只做路由，不复制 docs 与 skills 的正文。
4. Rust runtime 是执行真源，Web 是观察与调试层，不得双真源。
5. 先最小框架，再扩展能力；禁止先堆复杂自治再补可观测性。

## hard-guards
1. 先验证，后结论；无证据不宣称完成。
2. 禁止静默失败；错误必须结构化返回并记录事件。
3. 非授权不做破坏性操作（删除、回滚、迁移、发布）。
4. 禁止 broad kill：`pkill` / `killall` / `kill $(...)` / `xargs kill`。
5. 真实 payload 不可裁剪或改写语义；只允许裁剪内部调试快照。
6. 所有关键状态推进都必须产出结构化事件，供 Web / harness / CI 消费。
7. 所有实现遵循 owning layer，禁止跨层复制业务语义。
8. inference/provider/tool 改动默认按真实 provider E2E 验收；没有真实 session/provider/tool artifacts，不算闭环。
9. live provider/harness timeout 必须按阶段设置（connect / waiting provider / tool wait）；禁止用 180s/240s 这类短 wall-clock 总超时截断整条真实推理链。
10. 不得把 prompt 压缩当作通过真实业务测试的手段；context 工程只能在“正常会塞满上下文”的前提下优化装配与重建。

## route-map
1. 通用开发流程：`skills/fin-general-dev/SKILL.md`
2. 架构归属与 crate 边界：`skills/fin-architecture/SKILL.md`
3. 测试、harness、回放与故障注入：`skills/fin-testing-harness/SKILL.md`
4. 构建、版本、安装、提升与回滚：`skills/fin-build-versioning/SKILL.md`
5. runtime 调试与事件定位：`skills/fin-runtime-debug/SKILL.md`
6. prompt system 分层、role family、Agent-first prompt 边界、tool prompt spec：`skills/fin-prompt-system/SKILL.md`
7. 系统总览：`docs/architecture/01-system-overview.md`
8. 分层边界：`docs/architecture/02-layer-boundaries.md`
9. runtime 模型：`docs/architecture/03-rust-runtime-model.md`
10. 控制面与事件面：`docs/architecture/04-control-plane-http-ws.md`
11. 可观测性与 Web 调试后台：`docs/architecture/05-event-model-and-observability.md`、`docs/architecture/06-web-debug-console.md`
12. harness / replay / CI：`docs/architecture/07-harness-replay-fault-injection.md`、`docs/architecture/08-testing-and-ci-strategy.md`
13. workspace 与 crate 规划：`docs/architecture/09-workspace-and-crate-map.md`
14. 运行时对象与 session/task/topic 架构：`docs/architecture/10-runtime-session-task-architecture.md`
15. M1 最小可用脚手架与迭代顺序：`docs/architecture/11-m1-scaffolding-and-iteration.md`
16. Tentative session 与 formal task 的路由状态机：`docs/architecture/12-tentative-session-routing-state-machine.md`
17. Config 与 AI Provider 基础模块：`docs/architecture/13-config-and-provider-foundation.md`
18. `~/.fin` 运行时家目录与 session/workdir 布局：`docs/architecture/14-runtime-home-layout.md`
19. 全局安装、编译、回归、提升流程：`docs/architecture/15-install-build-regression-flow.md`
20. operation / event / projection 运行事实模型：`docs/architecture/16-operation-and-event-model.md`
21. debug 五层法与可观测工作流：`docs/architecture/17-debug-method-and-observability-workflow.md`
22. LiteLLM provider gateway 与 event pub/sub 架构：`docs/architecture/18-provider-gateway-with-litellm.md`
23. 最小 subscription registry 与 consumer 框架：`docs/architecture/19-subscription-registry-minimal-framework.md`
24. 外部 agent 消息通道、eventbus 与 mailbox 边界：`docs/architecture/20-external-agent-message-channel-eventbus-mailbox.md`
25. 外部消息 envelope 与最小握手状态机：`docs/architecture/21-message-envelope-and-handshake-state-machine.md`
26. M1 最小 agent core 模块切分：`docs/architecture/22-m1-minimal-agent-core-module-cut.md`
27. M1 第一批实现顺序与 crate/file 落点：`docs/architecture/23-m1-first-implementation-order-and-crate-landing.md`
28. session render 真源与 reasoning input assembly：`docs/architecture/24-session-render-truth-and-reasoning-input-assembly.md`
29. prompt system 分层、source ownership、Agent-first prompt 边界：`docs/architecture/25-prompt-system.md`
30. role family baseline、system/project agent 基线、owner/dispatcher/reviewer 边界：`docs/architecture/26-role-prompt-family-and-model-overlays.md`
31. stable core prompt 的归属、边界、装配顺序：`docs/architecture/27-stable-core-prompt.md`
32. stable core prompt 第一版文本草案：`docs/prompts/01-stable-core-prompt-v1.md`
33. 四类 role baseline 第一版文本：`docs/prompts/02-role-baselines-v1.md`
34. backend model 适配旧草案（已不再作为 agent prompt 真源）：`docs/prompts/03-gpt-codex-overlay-v1.md`
35. M1 contract 索引与最小 schema：`docs/contracts/00-m1-contracts-index.md`
36. prompt module / tool prompt / role prompt contract：`docs/contracts/prompt-module-contract.md`
37. provider operation / event contract：`docs/contracts/provider-operation-contract.md`、`docs/contracts/provider-event-contract.md`
38. conversation richness、reasoning/tool 渲染、interrupt / parallel 边界：`docs/architecture/28-conversation-richness-and-interrupt-model.md`
39. reasoning view contract：`docs/contracts/reasoning-view-contract.md`
40. tool execution semantic render contract：`docs/contracts/tool-execution-record-contract.md`
41. 多轮历史记录、Turn/Closure、Digest family、Context rebuild：`docs/architecture/29-multi-turn-history-model.md`
42. turn / step ledger / digest family / context rebuild index contracts：`docs/contracts/turn-record-contract.md`、`docs/contracts/step-ledger-contract.md`、`docs/contracts/digest-family-contract.md`、`docs/contracts/context-rebuild-index-contract.md`
43. M1 收口、冻结范围、回归矩阵、M2 backlog：`docs/closeout/m1-scope-and-freeze.md`、`docs/closeout/m1-regression-matrix.md`、`docs/closeout/m1-known-gaps-and-m2-backlog.md`
44. M1 收口 receipts：`docs/closeout/m1-receipts-2026-04-19.md`
45. M1 当前状态总结与下一阶段最小目标：`docs/closeout/m1-current-state-summary.md`
46. M1 单 agent 推理主链 review 与固定边界：`docs/closeout/m1-inference-mainline-review.md`
47. M1 receipt 标准化与 receipt-index 规则：`docs/closeout/m1-receipt-standardization.md`
48. M2 最小入口建议：`docs/closeout/m2-entry-recommendation.md`
49. M1 最终收口报告：`docs/closeout/m1-final-closeout-report-2026-04-20.md`
43. system/project/daemon 与 peer 分类：`docs/architecture/30-peer-taxonomy-and-supervision.md`
44. peer plane / execution plane / presence / binding：`docs/architecture/31-peer-plane-and-binding-model.md`
45. peer-aware 本地推理骨架与 local-only placeholder：`docs/architecture/32-peer-aware-local-reasoning-skeleton.md`
46. peer routing control skeleton 与 peer observation events：`docs/architecture/33-peer-routing-control-and-observation-events.md`
47. 文本 channel 卡片压缩与去重原则：`docs/architecture/34-text-channel-activity-cards.md`
48. runtime 启动身份与 entry role contract：`docs/architecture/35-runtime-startup-role-contract.md`
49. daemon / supervisor / entry agent / worker 启动 contract：`docs/architecture/36-daemon-supervisor-startup-contract.md`
50. project task system、owner loop、epic/task/claim/review 工作流：`docs/architecture/37-project-task-system-and-owner-loop.md`
51. project registry、wake queue、always_on / unfinished work 唤醒规则：`docs/architecture/38-project-registry-and-wakeup.md`
52. agent presence、busy/idle/offline、resume observation model：`docs/architecture/39-agent-presence-and-resume-model.md`
53. attached 前台 control-plane wrapper 与 continuation 顺序：`docs/architecture/40-attached-control-plane-cycle.md`
54. runtime 错误中心、禁止 fallback、错误 event/ledger 唯一路径：`docs/architecture/44-runtime-error-center.md`
55. runtime 模块 inventory + function map + verification map + 命名/layer gate：`docs/architecture/45-runtime-module-inventory.md`
56. runtime 9 个 owning domain 子目录（pipeline/ closure/ context/ tools/ session/ control/ task/ agent/ runtime_home/）的唯一入口与边界 gate：`rust/crates/runtime/src/pipeline/naming_static_tests.rs` 中 `lib_rs_domain_dirs_have_mod_entries` / `domain_mods_do_not_use_legacy_crate_paths` / `domain_dirs_have_no_fallback_or_salvage` / `cross_domain_no_direct_crate_file_imports`

## mandatory-flow
1. 先读本文件。
2. 再读相关 `docs/` 真源文档。
3. 再读对应 `skills/` 执行适配。
4. 先确定唯一真源与最小验证，再实现。
5. 新规律优先沉淀到 `docs/` 或 `skills/`，不要反灌长文到 `AGENTS.md`。

## evidence-report
每次交付至少回报：
- 变更内容
- 验证结果
- 未完成项 / 风险
- 下一步
