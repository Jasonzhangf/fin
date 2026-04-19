# M1 Receipt Standardization

本文档定义当前 M1/M1.1 阶段 receipt 的最小标准化规则。

目标不是发明复杂 harness 平台，而是先把当前已经存在的关键证据：

1. install smoke
2. provider probe
3. compact rebuild
4. status probe

统一收成一个**固定索引结构**，便于：

- closeout 引用
- 回归复核
- 后续自动化继续固化

---

## 1. 当前标准化结果

当前统一入口固定为：

```text
harness/reports/<build-version>/receipt-index.json
```

当前 schema：

```text
fin.receipt.index.v1
```

该 index 不替代原始 receipt，而是作为原始 receipt 的稳定目录页。

---

## 2. receipt-index 的职责

`receipt-index.json` 只负责回答：

1. 当前 build version 下有哪些 canonical receipts
2. 每个 receipt 是否存在
3. 每个 receipt 的状态是什么
4. 原始 report path 在哪里
5. 关键摘要字段是什么

它不负责：

1. 存储全部原始 payload
2. 替代原始 JSON receipt
3. 发明新的业务结论

换句话说：

> `receipt-index.json` 是 receipt 目录，不是 receipt 真源本体。

---

## 3. 固定字段

顶层固定字段：

- `schema_version`
- `generated_at`
- `build_version`
- `runtime_home`
- `report_dir`
- `receipts[]`

每个 receipt entry 固定字段：

- `receipt_id`
- `kind`
- `status`
- `report_path`
- `supporting_paths[]`
- `summary`

---

## 4. 当前 canonical receipt ids

当前 M1.1 先冻结 5 个 receipt id：

1. `install_smoke`
2. `provider_probe`
3. `compact_rebuild`
4. `status_probe`
5. `installed_binary_smoke_manual`

说明：

- `install_smoke` 对应当前正式 `install-dev` 成功后的 summary/install receipt
- `installed_binary_smoke_manual` 保留 closeout 隔离 run 的历史证据
- 后续若要继续扩 receipt family，必须先增加 schema 说明，不要随手乱长

---

## 5. status 语义

当前 receipt status 先冻结为三种：

1. `passed`
2. `failed`
3. `missing`

规则：

- receipt 文件存在且满足当前最小判定条件 => `passed`
- receipt 文件存在但关键条件不满足 => `failed`
- receipt 文件不存在 => `missing`

不要引入更多状态词，避免 closeout 阶段再复杂化。

---

## 6. 原始真源与索引的关系

### 6.1 install_smoke

原始真源：

- `harness/reports/<build>/summary.json`
- `install/receipts/<build>.json`

### 6.2 provider_probe

原始真源：

- `harness/reports/<build>/provider-probe.json`

### 6.3 compact_rebuild

原始真源：

- `harness/reports/<build>/web-debug-compact-response.json`
- 可选 supporting：
  - `web-debug-handcheck.json`

### 6.4 status_probe

原始真源：

- `harness/reports/<build>/web-debug-status-response.json`
- supporting：
  - `web-debug-binding.json`
  - `web-debug-handcheck.json`

### 6.5 installed_binary_smoke_manual

原始真源：

- `harness/reports/<build>/installed-binary-smoke-manual.json`
- supporting：
  - `logs/regression/...`

---

## 7. 当前生成方式

当前生成脚本：

```bash
scripts/build-receipt-index.py \
  --report-dir <.../harness/reports/<build>> \
  --runtime-home <runtime-home> \
  --build-version <build-version>
```

当前已实际生成：

1. `~/.fin/harness/reports/0.1.0001/receipt-index.json`
2. `~/.fin/harness/runs/m1-closeout-20260419-163831/runtime-home/harness/reports/0.1.0001/receipt-index.json`

---

## 8. 当前标准化的边界

当前这一步已经做到：

1. `build-receipt-index.py` 可稳定重建目录页
2. `install-dev / build-dev` 默认会刷新 `receipt-index.json`
3. `install_smoke` 摘要已带出 `session_id / task_id / operation_id / verified_paths`

仍然还没有做：

1. 所有 receipt 的统一生成器
2. 所有 receipt 的统一 schema
3. closeout run 的全部 receipt 自动收集
4. CI 自动发布 receipt index

这些属于下一轮可继续固化的范围。

---

## 9. 下一步最小演进方向

如果继续往前收，不要发散，优先顺序应固定为：

1. 保持 closeout 关键 run 默认生成 `receipt-index.json`
2. 补 mainline-focused receipt
3. 再考虑把更多 receipt family 纳入统一 schema

也就是说：

> 先把目录页固定，再逐步把原始 receipt 统一，而不是反过来一口气重做全部 harness。
