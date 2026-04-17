# 07 Harness, Replay, Fault Injection

`harness/` 是 `fin` 第一优先级基础设施之一。

## 目标

- 录制一次多 agent 协作
- 保存成 replay bundle
- 进行单步或整段回放
- 注入网络与进程故障
- 在 CI 里重放最小场景

## v1 场景能力

- 启动指定数量的 node / worker
- 派发任务
- 注入延迟、丢包、重复投递、断线、worker 重启
- 校验状态流与关键事件序列

## replay bundle 最小内容

- scenario metadata
- initial task input
- ordered events
- checkpoints
- expected assertions

## 关键规则

1. harness 与 Web 共用事件真源。
2. 没有 replay 的修复，默认调试成本高。
3. 故障注入优先围绕 transport / heartbeat / ownership / resume。
