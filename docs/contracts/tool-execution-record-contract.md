# Tool Execution Record Contract

本文档定义 `fin` 后续用于工具执行语义化渲染的最小 contract。

目标不是记录一切 transport 细节，而是提供：

- 会话前台 Rich mode
- Full Trace / debug mode
- note / digest / replay

共同消费的稳定工具执行真源。

---

## 1. 最小对象

```text
ToolExecutionRecord
```

建议最小字段：

- `tool_call_id`
- `operation_id`
- `trace_id`
- `session_id?`
- `task_id?`
- `worker_id?`
- `tool_name`
- `tool_kind`
- `title`
- `purpose`
- `target_kind?`
- `target_ref?`
- `input_summary?`
- `output_summary?`
- `status`
- `started_at`
- `ended_at?`
- `duration_ms?`
- `side_effects`
- `artifact_refs`
- `error_summary?`

---

## 2. 字段语义

### identity / refs

- `tool_call_id`：工具调用唯一 id
- `operation_id`：所属 turn / closure
- `trace_id`：所属链路
- `session_id?` / `task_id?` / `worker_id?`：所属执行范围

### tool semantics

- `tool_name`：工具名
- `tool_kind`：例如 `model_tool | framework_tool | provider_bridge | shell_like`
- `title`：给 UI 展示的人类可读标题
- `purpose`：这个工具干什么

### target

- `target_kind?`：例如 `file | repo | provider | url | task | session | command`
- `target_ref?`：目标对象摘要

### I/O

- `input_summary?`：输入摘要
- `output_summary?`：输出摘要

### lifecycle

- `status`：`started | completed | failed | cancelled | timed_out`
- `started_at`
- `ended_at?`
- `duration_ms?`

### side effects

- `side_effects`：例如
  - 写文件
  - 发请求
  - 更新 projection
  - 追加 event
- `artifact_refs`：相关文件 / event / record 的引用
- `error_summary?`：失败摘要

---

## 3. UI 渲染要求

Rich mode 中，`ToolExecutionRecord` 必须能渲染成“语义卡片”，而不是只显示裸 JSON。

最少应直接可读：

1. 这是哪个工具
2. 它在做什么
3. 它操作谁
4. 成功还是失败
5. 产出了什么

Full Trace 中再允许展开更多字段与关联 artifacts。

---

## 4. 与当前 `ToolSnapshot` 的关系

当前 M1 只有：

```text
ToolSnapshot {
  tool_name,
  status,
  summary
}
```

它可视为 `ToolExecutionRecord` 的极简前身。

后续升级原则：

1. 不让 UI 继续依赖临时字符串猜工具语义
2. runtime 应成为工具执行语义真源
3. `ProgressBlock.tool_snapshots` 可保留为轻量摘要层
4. 详细语义应独立升级到 `ToolExecutionRecord`
