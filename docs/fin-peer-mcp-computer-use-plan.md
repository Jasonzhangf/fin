# Fin Peer MCP Computer Use 集成实施计划

## 任务概述

将 Codex 的 MCP Computer Use 能力通过 Fin Peer 架构集成到 fin 框架中，实现一个独立的 `computer-use-peer` 节点，通过 peer 拓扑感知、daemon 生命周期管理和 mailbox 异步通信提供 Computer Use 能力。

---

## 实施阶段

### Phase 1: Peer 代理模块创建 (2-3 天)

**目标**: 创建 `computer-use-peer` 代理模块，实现 MCP 客户端连接和消息处理。

**文件结构**:
```
fin/peers/computer-use-peer/
├── Cargo.toml
├── src/
│   ├── main.rs              # Peer 入口点
│   ├── peer.rs              # Peer 实现
│   ├── mcp_client.rs        # MCP 客户端封装
│   ├── protocol.rs          # 消息协议定义
│   └── config.rs            # 配置加载
└── config/
    └── peer.toml            # Peer 配置模板
```

**核心职责**:
1. 通过 `daemon.ensure_peer` 启动 computer-use MCP server 进程
2. 注册 peer descriptor 到 fin 拓扑
3. 监听 mailbox 消息并路由到 MCP 工具调用
4. 返回结构化结果给请求方

**验收标准**:
- [ ] Peer 模块编译通过
- [ ] 可注册到 fin peer 拓扑
- [ ] 可通过 daemon 启动/停止 MCP server
- [ ] 可接收并回复 mailbox 消息

---

### Phase 2: 消息协议实现 (1-2 天)

**目标**: 定义并实现完整的 MCP 工具调用消息协议。

**请求格式** (mailbox.send → computer-use-peer):
```json
{
  "message_type": "mcp_tool_call",
  "message_id": "msg-uuid-001",
  "tool_name": "screenshot",
  "tool_params": { "monitor": 0, "format": "png" },
  "requester_id": "worker-01",
  "timeout_sec": 120,
  "trace_id": "trace-xxx"
}
```

**响应格式** (mailbox.poll ← computer-use-peer):
```json
{
  "message_type": "mcp_tool_result",
  "message_id": "msg-uuid-001",
  "status": "success|error",
  "result": { "image_path": "/tmp/screenshot_001.png", "image_data": "base64..." },
  "error": null,
  "duration_ms": 850,
  "responder_id": "computer-use-peer"
}
```

**验收标准**:
- [ ] 消息序列化/反序列化正确
- [ ] 工具调用请求/响应完整
- [ ] 错误处理完善
- [ ] 超时机制实现

---

### Phase 3: MCP 客户端封装 (2-3 天)

**目标**: 复用 Codex 的 `rmcp-client` crate，封装为 fin 可用的 MCP 客户端。

**依赖配置**:
```toml
[dependencies]
fin-framework = { path = "../../framework" }
codex-rmcp-client = { path = "../../../codex/codex-rs/rmcp-client" }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
tokio = { version = "1.0", features = ["full"] }
```

**核心能力**:
- 支持 Stdio 和 StreamableHttp 两种传输方式
- 工具调用超时控制（默认 120 秒）
- OAuth 认证支持（如需要）
- 连接健康检查

**验收标准**:
- [ ] 可连接 computer-use MCP server
- [ ] 可列出可用工具
- [ ] 可调用工具并获取结果
- [ ] 连接断开自动重连

---

### Phase 4: Peer 配置与拓扑集成 (1 天)

**目标**: 创建 peer 配置文件，集成到 fin 的 peer 拓扑系统。

**配置文件** (`fin/config/peers/computer-use-peer.toml`):
```toml
[peer]
peer_id = "computer-use-peer"
peer_kind = "mcp_proxy"
enabled = true

[peer.daemon]
command = "npx"
args = ["-y", "@anthropic-ai/computer-use"]
env = { DISPLAY = ":0" }
health_check_interval_sec = 30
auto_restart = true
max_restarts = 3

[peer.capabilities]
mcp_server_name = "computer-use"
transport = "stdio"
tools = ["screenshot", "click", "type", "scroll"]
max_concurrent_sessions = 4
```

**验收标准**:
- [ ] 配置文件加载正确
- [ ] peer 可被 `peer.list` 发现
- [ ] `peer.describe` 返回完整能力描述
- [ ] `daemon.ensure_peer` 可启动 peer

---

### Phase 5: 工具链集成 (1-2 天)

**目标**: 在 fin 工具链中添加 `mcp_call` 工具，使 worker 可通过框架工具调用 Computer Use。

**工具签名**:
```rust
#[tool(name = "mcp_call")]
async fn mcp_call(
    peer_id: String,        // 目标 peer ID
    tool_name: String,      // MCP 工具名
    tool_params: JsonValue, // 工具参数
    timeout_sec: Option<u32>, // 超时时间
) -> ToolResult
```

**调用流程**:
1. `peer.describe` 检查目标 peer 状态
2. 如离线，调用 `daemon.ensure_peer` 唤醒
3. 通过 `mailbox.send` 发送工具调用请求
4. 等待响应（带超时）
5. 返回结果或错误

**验收标准**:
- [ ] 工具注册到 fin 工具链
- [ ] 可通过 worker 调用
- [ ] 超时和错误处理正确
- [ ] 结果包含 trace_id 便于追踪

---

### Phase 6: 测试与文档 (2 天)

**目标**: 端到端测试和文档编写。

**测试场景**:
1. 正常截图流程：worker → mailbox → computer-use-peer → MCP server → 返回截图
2. Peer 离线唤醒：daemon.ensure_peer → 启动 → 调用
3. 超时处理：长时间操作自动超时返回
4. 错误传播：MCP server 错误正确返回给 worker
5. 并发调用：多个 worker 同时调用不冲突

**验收标准**:
- [ ] 所有测试场景通过
- [ ] 集成文档完整
- [ ] 配置示例可用
- [ ] 故障排查指南编写

---

## 总时间估算

| 阶段 | 时间 | 依赖 |
|------|------|------|
| Phase 1: Peer 代理模块 | 2-3 天 | 无 |
| Phase 2: 消息协议 | 1-2 天 | Phase 1 |
| Phase 3: MCP 客户端 | 2-3 天 | Phase 1 |
| Phase 4: 拓扑集成 | 1 天 | Phase 1-3 |
| Phase 5: 工具链集成 | 1-2 天 | Phase 1-4 |
| Phase 6: 测试与文档 | 2 天 | Phase 1-5 |
| **总计** | **9-13 天** | |

---

## 风险与缓解

| 风险 | 影响 | 缓解措施 |
|------|------|---------|
| Codex rmcp-client 依赖冲突 | 高 | 提前验证 Cargo.toml 兼容性 |
| computer-use MCP server 环境依赖 | 中 | 提供 Docker 配置备选方案 |
| 并发调用导致资源竞争 | 中 | 实现 session 池和队列 |
| 截图结果过大传输慢 | 低 | 支持文件路径引用而非 base64 |

---

## 后续扩展

- 支持多个 MCP server peer（browser-use-peer, file-system-peer）
- 消息优先级队列（高优先级请求优先处理）
- MCP 工具结果缓存（相同参数复用）
- Peer 热重载配置
