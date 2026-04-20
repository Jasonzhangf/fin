# 34 Text Channel Activity Cards

本文档冻结 `fin` 的纯文字 channel 活动卡体系。

本轮目标不是实现具体 QQ/Web UI，而是先把：

1. 用户前台看到的卡是什么
2. source 自报卡与总卡的关系是什么
3. 编辑/非编辑渠道如何统一更新
4. 工具执行如何被语义化解释给用户

固定成后续实现唯一真源。

---

## 1. 核心结论

`fin` 的纯文字 channel 采用 **双层卡体系**：

```text
source-owned progress cards
  +
system-owned user activity card
```

冻结结论：

1. 每个 `source` 都维护一张自己的活动卡
2. `system agent` 额外维护一张面向用户的总卡
3. 用户前台会话以总卡为主视图，source 卡是插入式补充
4. source 卡有更新才更新，无更新静默
5. 总卡也只在聚合结果变化时更新，无变化静默
6. 若渠道不支持编辑，则逻辑上仍视为“同一张卡”，物理上发送 **紧凑重绘版**
7. 若 1 分钟内完全无变化，则允许发送一次最小心跳更新；若仍无必要信息变化，可继续静默
8. 文字 channel 与 WebUI 共用同一套工具语义渲染规则，不允许两套解释逻辑

---

## 2. 三层模型

### 2.1 Runtime fact truth

运行事实真源仍然是：

```text
operation -> event
```

### 2.2 Source card view

每个 source 从 runtime/session truth 投影出自己的 card view。

典型 source：
- `system agent`
- `project agent`
- `agent peer`
- `capability peer`
- `channel gateway peer`

source card 负责回答：
- 我是谁
- 我现在在做什么
- 我最近做了什么
- 我是否在等待/失败/空闲

### 2.3 User activity card view

`system agent` 负责从多个 source card 聚合出用户前台总卡。

总卡负责回答：
- 当前整体谁在工作
- 当前焦点 source 是谁
- 当前全局阶段是什么
- 用户此刻最该关注什么

冻结边界：
- source 不直接接管用户前台总卡
- 总卡不直接复制原始 runtime event，而是消费 source card view

---

## 3. 卡的 owning scope

### 3.1 Source card

每个 source 一张卡。

归属规则：
- source 自己拥有自己的状态摘要
- source 自己决定何时产出新 card view
- 但投递节流与最终 channel 发送仍由 framework 控制

### 3.2 User activity card

用户前台总卡绑定 **用户前台会话**，而不是 task/session/channel 全局单例。

冻结规则：
- 当前只有 `system agent` 与用户直接对话
- 因此用户前台总卡由 `system agent` 拥有
- 后续即使接入更多 project agents / peers，它们仍不直接替代总卡 ownership

---

## 4. 更新模型

### 4.1 逻辑模型

框架逻辑上维护：

- 每个 source 的一张当前活动卡
- 用户前台的一张当前总卡

这些都是“当前视图”，不是历史日志本身。

### 4.2 渠道适配

若渠道支持编辑：
- 编辑当前卡

若渠道不支持编辑：
- 发送新的 **紧凑重绘版**
- 逻辑上仍然表示“更新同一张卡”
- 不发送零散无上下文的变化行

### 4.3 何时更新

source card / user card 统一规则：
- 有变化才更新
- 无变化静默
- 最长 1 分钟允许一次最小心跳
- 心跳也必须走 card view diff，不允许盲目重复发送旧内容

### 4.4 Delta 判定

delta 不直接按原始 event 触发。

必须按：

```text
previous delivered card view
vs
current card view
```

做标准化 diff。

若 diff 为空：
- 不更新

若 diff 非空：
- 生成下一次 card delivery

---

## 5. 可见性与自动提升

每个 source 都有基础可见级别：

- `hidden`
- `compact`
- `detailed`
- `verbose`

在此基础上，framework 允许按运行态自动提升：

- 当前主执行 source
- `failed`
- `waiting too long`
- `active and user-relevant`

冻结规则：
- source 基础级别与当前用户总卡的展开级别不是同一个概念
- source 可以是 `verbose`，但总卡仍然只引用其摘要
- 自动提升只能增加可见性，不能绕过 owning truth

---

## 6. 文本折叠语义

纯文字 channel 支持文本层面的折叠语义：

- 默认只显示摘要行
- 只有存在重点 source / 长等待 / 失败 / 当前主执行者时，才补充一行明细

冻结结论：
- 不做“一字段一行”的低密度输出
- 尽量利用每行宽度，减少无意义换行
- 总卡默认 3 行，最多 4 行
- source card `compact` 默认 2–3 行

---

## 7. 总卡结构

用户前台总卡默认由以下逻辑区组成：

### 7.1 卡头

放最稳定、最核心的信息：
- 项目
- 当前角色
- 总状态
- 当前焦点任务/会话/执行者

目标：
- 一行内完成
- 不放长 prose

### 7.2 活动源摘要

放当前活跃 source 的紧凑摘要：
- source 名称
- 当前状态
- 是否被自动提升
- 必要时标记 detailed/verbose

目标：
- 一行完成为主
- 只放当前用户应关注的 source

### 7.3 当前进度

放：
- 当前阶段
- 最近 2–3 个关键动作
- 当前等待态
- 已耗时/最近更新时间

### 7.4 按需明细

仅在必要时出现：
- 失败原因
- 长等待原因
- 当前主执行 source 的额外说明

---

## 8. Source card 结构

每个 source card 至少包含：

1. `source identity`
2. `runtime state`
3. `current activity summary`
4. `recent semantic actions`
5. `waiting/failure detail`
6. `visibility level`

冻结规则：
- source card 是 source 自己的当前状态视图，不是完整历史
- 历史仍保留在 session/runtime truth 中
- source card 不能直出原始 tool args / raw event dump

---

## 9. Verbose 与分片

当某个 source 被要求 `verbose` 时：

- 允许该 source 产生 **分片更新**
- 分片用于承载高密度工具活动的详细信息
- 分片属于 source card 的扩展交付，而不是总卡本体

冻结规则：
- 分片只作用于 **source card**
- 总卡最多引用“该 source 有详细更新/最近更新了几片”这类摘要
- 不允许把 verbose 明细整段灌进总卡，破坏总卡紧凑性

---

## 10. 工具语义渲染 contract

文字 channel 与 WebUI 统一遵循同一套工具语义渲染规则。

用户只关心“做了什么”，不关心原始工具参数。

### 10.1 读文件类

应显示：
- 读了什么文件/目录
- 目的是什么

不显示：
- 原始 `path` 参数 JSON

### 10.2 写文件类

应显示：
- 写了什么文件
- 写入意图是什么

### 10.3 计划更新类

应显示：
- 更新了什么计划

冻结规则：
- `compact` 显示摘要
- `detailed/verbose` 显示完整计划条目

### 10.4 命令执行类

应显示：
- 跑了什么命令
- 命令目的是什么
- 成功/失败

### 10.5 搜索类

应显示：
- 搜了什么关键词
- 在哪里搜
- 搜索范围/来源是什么

例如：
- 代码搜索：关键词 + 代码范围
- Web 搜索：关键词 + 来源/站点

### 10.6 禁止项

禁止直出：
- 原始 tool args
- 原始 JSON payload
- 大段原始 trace
- 仅内部可读的 framework 参数

---

## 11. 交付与真源边界

冻结结论：
- channel adapter 不拥有业务语义
- channel adapter 只负责：
  - 读取 card view
  - 做 diff
  - 适配编辑/非编辑渠道
  - 投递
- card view 的业务语义必须来自 framework-owned builder
- WebUI 与文字 channel 必须消费同一套 card/tool semantic truth

---

## 12. 后续实现顺序

建议实现顺序冻结为：

1. 定义 source card / user card contract
2. 实现 source card view builder
3. 实现 user card aggregator
4. 实现 tool semantic render 统一层
5. 实现 card diff + delivery policy
6. 接入 qqbot / WebUI 共同消费

结论：

> `fin` 的纯文字 channel 不是把 Web debug 文本化，而是通过 “source card + user card + semantic tool rendering + diff-based delivery” 提供一个紧凑、可持续、可多 source 协作的正式前台。
