# 12 Tentative Session and Routing State Machine

本文档定义 `fin` 在 **尚未正式创建 task** 时，到 **进入正式任务闭环** 之间的控制状态机。

目标只包括：

- 新输入如何启动
- 什么时候保持 simple chat
- 什么时候由模型显式建议 formalize / reuse / switch
- 什么时候正式执行 `task creation operation`
- 谁负责判断，谁负责刚性执行

不展开：

- 存储格式
- API/wire shape
- UI 组件细节
- provider 调用细节

## 1. 核心原则

1. 新输入默认先进入 `TentativeSession`。
2. 在没有正式 `task_id` 前，所有自然语言输入都必须先进入模型；framework 不得先行做自然语言意图判断。
3. 模型先输出结构化 routing / control feedback，由它判断当前更像 simple chat、普通任务推进、还是需要 formalize 的任务候选。
4. 框架只消费模型的显式反馈或用户刚性命令；**不得仅凭 confidence 阈值、聊天文本或 wrapper 启发式把普通输入自动升级成“新任务候选”**。
5. 正常用户↔agent 对话中，framework 保持透明；若需要确认/澄清，应由模型在可见回复中自己表达，而不是 framework 插话。
6. 只有当模型显式给出对应 control 指标、或用户显式触发支持的刚性命令时，框架才执行正式 `task creation operation`。

## 2. 关键对象

### `TentativeSession`

未正式绑定 task/topic 的临时交互容器。

持有：

- 当前输入与短期连续性
- intent summary
- preview
- candidate binding
- routing confidence

### `RoutingFeedbackBlock`

模型在本轮结束时回给框架的结构化判断。

最小字段：

- `candidate_task_id`
- `candidate_topic_thread_id`
- `continuity_confidence`
- `topic_shift_confidence`
- `simple_query_confidence`
- `reason`

### `TaskListBlock`

当前上下文中的结构化 task 列表，帮助模型判断是否延续已有 task。

### `TopicListBlock`

当前上下文中的结构化 topic 列表，帮助模型判断是否应 revive / switch / bind 历史 topic。

### `FormalizationOperation`

框架在用户确认后执行的正式动作，用于：

- 创建 `task_id`
- 绑定 `topic_thread_id`
- 把 tentative 内容并入首个正式 closure

## 3. 状态集合

### `tentative_open`

默认起始状态。

表示：

- 新输入已进入系统
- 尚无正式 `task_id`
- 正在收集 intent / preview / routing feedback

### `tentative_simple_chat`

表示：

- 当前输入被判断为 simple chat / simple query
- 暂不进入正式 task 体系

### `tentative_candidate_existing`

表示：

- 模型的 control feedback 表明当前输入可能属于已有 topic / task
- framework 仅记录该候选，等待后续显式 trigger

### `tentative_candidate_new`

表示：

- 模型的 control feedback 表明当前输入可能需要新建正式 task/topic
- framework 仅记录该候选，等待后续显式 trigger

### `pending_observation`

表示：

- 当前 continuity / topic shift 判断不够稳定
- 先继续观察，不立即打断用户

### `pending_user_choice`

表示：

- 模型已显式表明需要用户补充/确认，framework 只记录阻塞状态
- 正在等待用户选择：
  - 继续当前
  - 复用历史 topic
  - 新建 task/topic
  - side topic
- 用户可见说明必须来自模型回复或用户显式命令路径，不由 framework 单独发 prompt

### `formalizing`

表示：

- 用户已确认
- 框架正在执行 `FormalizationOperation`

### `task_bound`

表示：

- 已正式获得 `task_id`
- session 已进入正式 task 闭环

### `side_topic_active`

表示：

- 当前进入 side topic 模式
- 默认不保存为正式长期主线

### `discarded`

表示：

- tentative 会话结束
- 未进入正式 task 体系

## 4. 状态迁移

## A. 启动阶段

`new input -> tentative_open`

条件：

- 到来一条新输入
- 当前没有可直接确认绑定的正式 task

动作：

- 建立 `TentativeSession`
- 构造最小 context
- 调用模型获得首轮 routing feedback
- 在模型返回前，framework 不对自然输入做任务/聊天预判

## B. 简单聊天路径

`tentative_open -> tentative_simple_chat`

条件：

- `simple_query_confidence` 高
- 且没有强烈的新 task / topic revival 信号

动作：

- 走轻量聊天路径
- 不创建正式 `task_id`

`tentative_simple_chat -> discarded`

条件：

- 对话自然结束
- 未升级为正式任务

`tentative_simple_chat -> tentative_candidate_existing | tentative_candidate_new`

条件：

- 后续输入表明已不再是 simple chat

## C. 绑定已有 topic/task 候选

`tentative_open -> tentative_candidate_existing`

条件：

- `continuity_confidence` 或 `topic revival` 置信度高
- 命中了已有 task / topic

动作：

- framework 记录历史候选列表
- 是否继续/是否需要确认，必须等待模型 control signal 或用户刚性命令

`tentative_candidate_existing -> pending_user_choice`

条件：

- 需要用户显式确认

`tentative_candidate_existing -> task_bound`

条件：

- 模型显式给出可直接继续/复用的 control 指标
- 且无需额外用户确认

## D. 新建 task/topic 候选

`tentative_open -> tentative_candidate_new`

条件：

- 模型显式给出“建议 formalize 为新任务”的反馈
- 明显不是 simple chat
- 且不属于当前已有 task/topic

动作：

- framework 记录 preview
- 不主动生成可见确认话术
- 不允许只因为 `continuity/topic/simple` 分数落到某个阈值，就由 framework 自己生成新任务候选

`tentative_candidate_new -> pending_user_choice`

条件：

- 需要用户确认新建

## E. 不确定观察态

`tentative_open -> pending_observation`

条件：

- continuity / topic shift 判断不稳定
- 不值得立即打断用户

动作：

- 保守维持当前 tentative 路径
- 再观察后续 1~几轮反馈

`pending_observation -> tentative_candidate_existing | tentative_candidate_new | tentative_simple_chat`

条件：

- 后续置信度升高

## F. Formalize 只做 bind，不做 hidden follow-up

`pending_user_choice -> formalizing -> task_bound`

条件：

- 用户显式刚性命令触发 formalize / reuse / switch，或
- 模型已返回明确 control 指标且后续用户输入满足该显式要求
- framework 已完成 task/topic bind

动作：

- framework 执行 `FormalizationOperation`
- 生成正式 `task_id`
- 绑定 `topic_thread_id`
- 追加 framework 事件：
  - `session.formalized`
- formalize 完成后，framework 不自动 enqueue hidden follow-up input，也不自动推进 scheduler turn

规则：

1. formalize 只做 bind，不直接把 decomposition/planning 混进同一个用户动作
2. formalize 之后若要继续 direct / managed 推进，必须来自：
   - 后续正常 agent 推理轮，或
   - 用户刚性命令，或
   - 已落盘 control / execution truth 触发的工具集切换与后续推理
3. framework 不得把 `/formalize` 自动扩展成 hidden follow-up input、hidden prompt、或自动 scheduler 推进
4. `continuity_confidence / topic_shift_confidence / simple_query_confidence` 仍可作为模型输出里的观测字段保留，但 framework 不得单独依赖这些分数做 formalize / switch / dispatch 决策

## G. 等待用户选择

`pending_user_choice -> formalizing`

条件：

- 用户明确选择，或显式发送受支持命令：
  - 绑定历史 topic
  - 新建 task
  - 继续当前并 formalize

`pending_user_choice -> side_topic_active`

条件：

- 用户选择 side topic

`pending_user_choice -> tentative_simple_chat | tentative_open`

条件：

- 用户选择暂不 formalize
- 继续轻量对话

## H. 正式化

`formalizing -> task_bound`

条件：

- `FormalizationOperation` 成功

动作：

- 创建 `task_id`
- 绑定 `topic_thread_id`
- 将 tentative 内容并入首个正式 closure

`formalizing -> tentative_open`

条件：

- formalization 失败但可恢复

动作：

- 切回 tentative 状态
- 记录异常事件

## I. Side topic

`side_topic_active -> tentative_open | task_bound | discarded`

条件：

- side topic 结束
- 或用户选择提升为正式 task
- 或直接结束

## 5. 控制权边界

### 模型负责

- 理解自然语言输入
- 输出 routing feedback
- 给出置信度与原因
- 帮助生成 preview 和 intent summary
- 在需要确认/澄清时，于可见回复中自己向用户表达

### 框架负责

- 维护状态机
- 解析刚性命令
- 解析模型返回的显式 control 指标
- 执行 `FormalizationOperation`
- 在显式 trigger 已满足时推进 formal task 体系
- 记录 `session.formalized` 等 framework-owned durable truth

附加硬规则：

- framework 不得理解或判断自然语言用户输入
- framework 不得把“未绑定 task + 某个 confidence 阈值”直接等价成“新任务”
- framework 不得根据聊天文本、wrapper shortcut、可见回复文案或 heuristic 自行触发 formalize/routing/scheduler
- 新任务 formalize 的触发必须来自：
  - 模型显式 control signal，或
  - 用户显式刚性命令（如 `/formalize`）

### 用户负责

- 在需要时确认：
  - 复用历史话题
  - 新建正式任务
  - side topic
  - 保持轻量聊天

## 6. 用户确认点

以下场景可以由**模型在可见回复中**提出确认，framework 只记录等待状态，不单独发提示：

1. 当前输入疑似属于历史 topic，需要确认是否复用
2. 当前输入已形成明确目标，需要确认是否创建正式 task
3. 当前对话发生话题偏移，需要确认是否切换 topic
4. 用户希望临时旁路讨论，需要确认是否进入 side topic

## 7. 与 M1 的关系

M1 不要求实现完整 topic revive / merge / split 体系，但要求至少支持：

- `TentativeSession`
- simple chat vs formal task 的基础判断
- model-driven clarification + framework passive execution
- formal task creation
- tentative -> first closure 的并入

## 8. 当前非目标

以下属于后续模块阶段再展开：

- revive / merge / split 的完整算法
- 自动 rebind 的复杂策略
- topic drift 的高级观察窗口
- 多 worker 下的协同 bootstrap
