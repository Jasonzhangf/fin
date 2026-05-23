/goal
目标：按 `fin` context compression / prompt cache 审计方案，重构上下文装配、context review 与压缩链路，使普通多轮任务/多轮绘画保持稳定前缀，只有达到上下文预算阈值才 compact。

实现文档：
- `docs/refactor/context-compression-cache-audit-2026-05-23.md`
- `docs/goals/context-compression-cache-alignment-plan.md`

执行规范：
- Rust runtime/provider 是唯一真源；UI 不承担 context/compression 语义。
- 禁止 fallback / 静默降级 / 通过裁剪真实 payload 换缓存命中。
- 先做 request shape、baseline diff、compact trigger 测试，再改实现。
- 普通 turn 只能 append/diff；compact 必须由 token usage/context budget/context limit 触发。
- 最新工具调用、工具结果、当前用户输入必须位于 prompt 尾部。

验证：
- request shape snapshot。
- baseline full-once + diff tests。
- low-usage no-compact / high-usage compact tests。
- compact history replacement tests。
- provider prompt_cache_key + usage record tests。
- 多轮绘画 fixture：compact 后保留 image/artifact refs。

完成标准：
- `ContextBudgetManager / ContextAssemblyPlanner` 成为 context review 与 compression 的唯一决策入口。
- 稳定前缀顺序被测试锁定，普通 turn 不重建完整 history。
- `/compact` 与 auto compact 共用同一 compact engine。
- 当前审计文档中的待审批修改点全部有实现、测试、证据或明确关闭说明。
