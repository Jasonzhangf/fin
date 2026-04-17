# CACHE — Short-Term Memory

## Current Task
- Task: 收敛 provider debug + bounded context snapshot + Web 可观测基础
- Status: in_progress

## Recent Decisions
- [2026-04-17] build/回归/安装/提升要收敛到统一 build flow
- [2026-04-17] build version 与 Cargo semver 分层；未来预留 core/module version topology
- [2026-04-17] current context 只写 `runtime/current/current_context.json` 最新覆盖版
- [2026-04-17] session recent context 只写 `context/recent_contexts.json` bounded window（8）
- [2026-04-17] M1 不引入 detached debug daemon，避免 orphan process

## Completed Evidence
- transcript-demo real provider closure passed; turn-3 returned BANANA-42 from prior context
- CLI build/versioning split completed; main.rs removed from line-limit whitelist
- real `build-dev` isolated run passed
- real manual staged -> `promote 0.1.0099` passed
- install-dev isolated run passed with auto build versions `0.1.0001 -> 0.1.0002`
- current/previous symlink update verified
- `cargo test --workspace` passed
- isolated real provider runtime-demo passed with `OK`
- projection / current_context / recent_contexts artifacts verified in isolated runtime home

## Next Steps
- 继续细化 Web tab/分区设计
- 考虑把 transcript/demo 闭环收进 installed smoke 或 harness 脚本
- 再推进 M1 最小 agent core