# Model Output Repair Contract

本文档冻结 `fin` 对模型输出的修复边界。

目标：

- 在不改写模型语义的前提下，提高 structured output 的兼容性
- 让 runtime 能区分“严格解析成功 / 确定性修复成功 / 检测到但无效”
- 为后续 retry feedback / Web debug / harness 断言提供共同真源

---

## 1. 核心原则

`fin` 对模型输出的处理只允许：

> **确定性、语义保持的形状修复**

明确禁止：

> **任何需要语义推断、值补全、意图猜测的修补**

换句话说：

- 可以修结构
- 可以做白名单协议归一化
- 不可以补业务语义
- 不可以把 prose 解释成工具调用

---

## 2. 适用范围

当前 contract 适用于三块 structured output：

1. `fin_user_response`
2. `fin_control_feedback`
3. `fin_tool_calls`

三者共享同一条总规则，但容忍度不同：

- `control_feedback`：允许更高兼容度的 whitelist salvage
- `tool_calls`：允许确定性形状修复，但只要语义值不完整就禁止执行

---

## 3. 允许的修复

以下修复属于“形状修复”，允许由 runtime 自动执行：

### 3.1 外壳修复

- 去掉 code fence（如 ```json）
- 去掉 tag 内多余空白
- 为缺失的结束 tag 做确定性补齐
- 单个 tool object 包装为数组

### 3.2 协议白名单归一化

- `name -> tool_name`
- `args -> arguments`
- 明确布尔字段 `"true" -> true`
- 明确置信度字段 `0.98 -> 98`
- 协议允许的默认空对象：`arguments -> {}`

### 3.3 确定性结构闭合

只有在**值语义已经完整**时，才允许补齐：

- 缺失的 `]`
- 缺失的 `}`
- 缺失的 tag closing

要求：

- 修复结果唯一
- 不需要猜测字段值或字符串内容

---

## 4. 禁止的修复

以下都属于语义推断，严格禁止：

- 把“我准备读取目录”解释成 `exec_command`
- 根据上下文猜测 tool name
- 根据半截参数猜测命令内容
- 给截断字符串补全内容
- 把 prose 重新生成为 structured tool call

一句话：

> runtime 可以补壳，不能补脑。

---

## 5. Tool Call 执行边界

`fin_tool_calls` 的解析结果冻结为四档：

1. `exact`
   - 原始输出已严格合法
2. `repaired_deterministic`
   - 经过确定性形状修复后合法
3. `masked_partial`
   - 只检测到局部结构或局部字段，足够 debug，不足够执行
4. `invalid`
   - 既不能严格解析，也不能确定性修复

执行边界：

- 可执行：
  - `exact`
  - `repaired_deterministic`
- 不可执行：
  - `masked_partial`
  - `invalid`

说明：

- `tool_calls` 可以 repair 后执行
- 但不能 mask 后执行

---

## 6. Control Feedback 边界

`fin_control_feedback` 允许更高兼容度的 whitelist salvage：

- 允许字段级 mask
- 允许协议白名单默认值
- 允许布尔/整数标准化

但仍然禁止：

- 根据 prose 猜测 continuation / topic shift / task 语义
- 生成不在白名单内的新控制字段

---

## 7. 修复证明责任

每一次 runtime repair 都必须能回答：

> **这次新增的内容，是否只包含结构符号或协议映射，而没有新增业务语义？**

若答案是否定，则不得自动修复。

允许新增：

- `]`
- `}`
- `</fin_tool_calls>`
- 外层数组包裹
- alias 到 canonical 字段名的映射

禁止新增：

- 新字符串值
- 新数字值
- 新 tool name
- 新 arguments 值
- 新命令内容

---

## 8. 对 retry feedback 的约束

当 structured output 不可执行时，runtime 应反馈**结构错误**，而不是改写模型原意。

例如：

- `fin_tool_calls` 检测到但 JSON 非法
- `reasoning.stop` 参数值被截断，无法确定性修复
- `fin_control_feedback` 缺少合法 JSON shape

反馈目标：

- 保持原语义不变
- 只要求模型修复结构
- 减少无意义重试

---

## 9. 当前 M1 收口要求

M1 当前至少要做到：

1. parser 能区分 exact / repaired / invalid 的基础状态
2. deterministic repair 进入 runtime truth / round debug 观测
3. tool call 若语义值不完整，绝不执行
4. Web / harness / receipt 能看出“没调工具”和“输出了坏工具块”之间的区别

## 10. Retry attempts 的 durable truth 边界

当 output contract retry 发生时，runtime 必须把它视为：

- **同一 logical round**
- **多个 provider attempt**

冻结边界：

1. `RoundRecord`
   - 仍表示 logical round
   - 只记录最终被接受的 attempt
2. `ProviderRequestRecord / ProviderResponseRecord`
   - 必须为每个 attempt 各写一条 durable artifact
   - 通过 `round_index + attempt_index` 唯一标识
3. `StepRecord`
   - 必须保留 attempt 级摘要，便于 timeline/debug 看出：
     - 第几次 attempt
     - 为什么被判定为 invalid / retry
     - 最终哪一次被 accepted

一句话：

> logical round 只有一个，但 provider/session truth 必须完整保留每次 retry attempt。
