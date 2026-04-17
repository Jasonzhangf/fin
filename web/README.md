# web

这里将承载 fin 的 Web 调试后台。

第一阶段目标：

- cluster overview
- task timeline
- message trace
- raw event viewer
- session inspector
- replay console
- fault injection panel

数据只来自 runtime 暴露的 HTTP / WebSocket / replay bundle，不直接读取运行时私有内存。

## 当前 M1 MVP

当前最小可用入口已经落到 Rust `fin-cli`：

```bash
cd rust
cargo run -p fin-cli -- web-debug ~/.fin/config/user.toml 4040
```

默认提供：

- `/`：最小调试页面
- `/api/current_projection.json`
- `/api/current_snapshot.json`
- `/api/latest_events.jsonl`
- `/api/last_run.json`

当前页面先做：

- Current Projection
- Last Run
- Warnings / Errors
- Live Event Stream
- 基础 trace/task/session 文本过滤

说明：

- Rust runtime 仍是真源
- Web 只消费 `~/.fin/runtime/*` 已落盘调试数据
- M1 先使用 HTTP + polling；WebSocket 留到后续阶段