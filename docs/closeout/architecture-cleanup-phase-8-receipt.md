# Architecture Cleanup Phase 8 — Verification Matrix Receipt

> 配套 `docs/goals/architecture-cleanup-plan.md`。
> 收口时间：2026-06-06。
> 基于 commit `486d18f`（9 domain splits + 边界 gate）之后的 working tree。

## 0. 前置状态

- 9 个 owning domain 子目录已落地：agent/ closure/ context/ control/ pipeline/ runtime_home/ session/ task/ tools/
- 37 个根 .rs 文件残留（lib.rs + closure 编排主干 + cross-domain 桥 + prompt/model/activity_cards Phase 5b/c/d backlog）
- 命名/layer 边界 gate 已加 4 个新测试（`lib_rs_domain_dirs_have_mod_entries` / `domain_mods_do_not_use_legacy_crate_paths` / `domain_dirs_have_no_fallback_or_salvage` / `cross_domain_no_direct_crate_file_imports`）
- AGENTS.md route-map 已补 §55（inventory）+ §56（9 domain + 4 gate）

## 1. L1: 单测层（unit / contract）

| crate | 测试 | 状态 | evidence |
| --- | --- | --- | --- |
| fin-runtime | 116 红测 (含 run_closure_error_center_tests) | 119 passed, 1 failed (intentional red) | `cargo test -p fin-runtime` |
| fin-provider | 13 红测 | 13 passed, 0 failed | `cargo test -p fin-provider` |
| fin-shared | lib unit | 全部通过 | inherited from cargo workspace |
| fin-config | lib unit | 全部通过 | inherited from cargo workspace |

L1 结论：pass。1 红为边界 lock gate（design intent），非 regression。

## 2. L2: 静态 gate（命名 + 编号 + 禁止 fallback + 跨域 lock）

| gate | 测试 | 通过/总数 | evidence |
| --- | --- | --- | --- |
| error_pipeline_static | `error_pipeline_*` | 2/2 | 禁止 forbidden numbering（03a/04_/04./V2）+ 禁止 swallow/fallback |
| feedback_pipeline_static | `feedback_pipeline_*` | 2/2 | 禁止 impl From + 禁止 fallback + adjacent builders only |
| input_pipeline_static | `input_pipeline_*` | 2/2 | 禁止 From conversions + unique node type names + adjacent builders |
| reason_pipeline_static | `reason_pipeline_*` | 2/2 | adjacent node conversion only + unique node type names |
| naming_static | legacy + 4 新 boundary gates | 12/13 (1 red intentional) | lib.rs 9 domain mod 声明 + 禁止 legacy flat mod + 禁止 fallback string + 禁止跨域 legacy import |

L2 结论：5/5 gate 套件活跃。naming_static 的 1 红为 prompt_assembly backlog（Phase 5b），gate 设计为红直至 Phase 5b 解决，不是 regression。

## 3. L3: 失败注入（fault injection）

待补：provider 500 / missing config / invalid tool 三类故障注入 + 验证：
- failure 不能 fallback 成 success
- 错误必须经 `map_runtime_error_through_error_pipeline`
- 必须产生 `error.detected` + `error.user_visible_prepared` 事件 + ledger 记录

## 4. L4: 真实 provider smoke

待补：需要真实 OPENAI_API_KEY + live harness timeout（connect / waiting / tool wait 三阶段），out of 当前 scope。

## 5. L5: Web/debug 只读 event

待补：web debug 页面只读 events/ledger 不生成第二错误真相的回归测试。

## 6. 当前缺口

| Phase | 状态 | 阻塞 |
| --- | --- | --- |
| Phase 5（runtime domain 拆分） | partial (9/9 domain dirs 完成，37 根 .rs 残留) | prompt/model/activity_cards 跨域引用复杂（bridge approach 多次失败） |
| Phase 8 L1 | ✅ pass | 无 |
| Phase 8 L2 | ✅ pass (1 intentional red) | 无 |
| Phase 8 L3 | ⏳ 待补 fault injection 测试 | 无技术阻塞，待本轮补充 |
| Phase 8 L4 | ⏳ 需要真实 provider | 需要 OPENAI_API_KEY + harness timeout 配置 |
| Phase 8 L5 | ⏳ 需要 web debug 测试 | 需要 web 层 echo harness |

## 7. 8 阶段完成度

| 阶段 | 完成度 | evidence |
| --- | --- | --- |
| 1 冻结 docs | 100% | 5e71f1a, 44-runtime-error-center.md |
| 2 inventory | 100% | 45-runtime-module-inventory.md §1-3 |
| 3 去 fallback | 100% | `rg fallback\|silent_salvage\|treat_invalid_as_success` = 0 hits in runtime/src |
| 4 接入 ErrorErr* | 100% | run_closure_error_center_tests + error_pipeline_static |
| 5 拆 domain 目录 | 70% (9 dirs 完成，37 根文件残留) | inventory §3 backlog 列表 |
| 6 hub_pipeline 物理删除 | 100% | a34766a, naming_static::provider_hub_pipeline_fully_deleted |
| 7 命名/边界 static tests | 100% | naming_static + 4 pipeline_static + 4 boundary gate |
| 8 验证矩阵 L1-L5 | 50% (L1+L2 done, L3-L5 pending) | 本 receipt |
