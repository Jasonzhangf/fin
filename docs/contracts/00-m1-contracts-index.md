# 00 M1 Contracts Index

本文档是 `fin` 的 M1-A contract 总入口。

M1-A 当前先冻结六类最小 contract：

1. `Operation Envelope`
2. `Event Envelope`
3. `Progress Block`
4. `Execution Note`
5. `Digest Record`
6. `Projection View`

## 目标

- 给 `fin-contracts` 提供最小稳定边界
- 给 runtime / debug / harness / Web 提供共同消费模型
- 让 M1 可以先打通单 runtime、单推理闭环、最小调试闭环

## 文档索引

1. `docs/contracts/operation-envelope-contract.md`
2. `docs/contracts/event-envelope-contract.md`
3. `docs/contracts/progress-block-contract.md`
4. `docs/contracts/execution-note-contract.md`
5. `docs/contracts/digest-contract.md`
6. `docs/contracts/projection-view-contract.md`

## 扩展 contract（后续 provider / pub-sub 真源）

以下文档不属于最初 M1-A 六类最小 schema，但已作为后续真源预留：

1. `docs/contracts/provider-operation-contract.md`
2. `docs/contracts/provider-event-contract.md`

## crate 真源

对应 crate：

- `rust/crates/contracts`

## 当前非目标

以下留到后续模块阶段：

- 完整 capability matrix
- 完整 provider response schema
- 多机复制协议
- 历史压缩格式
- 最终 HTTP/WS wire 命名