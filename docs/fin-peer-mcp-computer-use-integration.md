# Fin Peer MCP Computer Use 集成方案

## 1. 背景

Codex 的 Computer Use 不是独立二进制或内置实现，而是通过 **MCP (Model Context Protocol)** 调用外部 `computer-use` MCP server。Codex 的 MCP 客户端实现在 `~/code/codex/codex-rs/rmcp-client/` 和 `~/code/codex/codex-rs/codex-mcp/`，是纯 Rust 的成熟库。

本方案将 `computer-use` MCP server 作为 fin 框架中的独立 peer 节点，通过 fin 的 peer 拓扑和 mailbox 机制进行协作。

## 2. 整体架构

```
fin peer 拓扑:
┌─────────────────────────────────────────────────────┐
│                    System (调度器)                    │
│  peer.list ──→ 拓扑感知                              │
│  daemon.ensure_peer ──→ 生命周期管理                 │
│  agent.assign ──→ 任务分派                           │
└──────────────┬──────────────────────────┬────────────┘
               │                          │
      ┌────────▼────────┐        ┌────────▼────────┐
      │  worker-01/02   │        │ computer-use-peer│
      │  (通用任务执行)  │        │ (MCP Server 代理) │
      │  mailbox.poll   │        │  ├─ screenshot   │
      └─────────────────┘        │  ├─ click        │
                                 │  ├─ type         │
                                 │  └─ emitImage    │
                                 └─────────────────┘
```

## 3. Peer 描述符设计

```json
{
  "peer_id": "computer-use-peer",
  "peer_kind": "mcp_proxy",
  "presence": "idle|busy|offline",
  "binding_support": ["session_binding", "agentic_execution"],
  "capabilities": {
    "mcp_server_name": "computer-use",
    "transport": "stdio|streamable_http",
    "tools": ["screenshot", "click", "type", "scroll", "emitImage"],
    "max_concurrent_sessions": 4
  },
  "daemon_config": {
    "command": "npx",
    "args": ["-y", "@anthropic-ai/computer-use"],
    "env": {"DISPLAY": ":0"},
    "health_check_interval_sec": 30,
    "auto_restart": true
  }
}
```

## 4. 消息协议

### 请求格式 (mailbox.send → computer-use-peer)

```json
{
  "message_type": "mcp_tool_call",
  "message_id": "msg-uuid-001",
  "tool_name": "screenshot",
  "tool_params": {"monitor": 0, "format": "png"},
  "requester_id": "worker-01",
  "timeout_sec": 120,
  "trace_id": "trace-xxx"
}
```

### 响应格式 (mailbox.poll ← computer-use-peer)

```json
{
  "message_type": "mcp_tool_result",
  "message_id": "msg-uuid-001",
  "status": "success|error",
  "result": {
    "image_path": "/tmp/screenshot_001.png",
    "image_data": "base64...",
    "dimensions": {"width": 1920, "height": 1080}
  },
  "error": null,
  "duration_ms": 850,
  "responder_id": "computer-use-peer"
}
```

## 5. 完整任务调度流程

```
Worker-01 需要截图
    ↓
peer.list 查找 computer-use-peer
    ↓
┌─────────────────────────────────┐
│ computer-use-peer 状态检查       │
├─────────────┬─────────┬─────────┤
│   离线      │   忙    │   空闲  │
│             │         │         │
│ daemon.     │ mailbox.│ mailbox.│
│ ensure_peer │ send →  │ send →  │
│ → 唤醒      │ 队列等待│ 立即处理│
└─────────────┴─────────┴─────────┘
                    ↓
         computer-use-peer 执行 MCP 工具调用
                    ↓
         mailbox.send 返回结果给 Worker-01
                    ↓
         Worker-01 处理截图结果
```

## 6. 实现步骤

### 步骤 1: 创建 computer-use-peer 代理模块

```rust
// fin/peers/computer_use_peer/src/main.rs
use fin_framework::peer::{Peer, Mailbox, PeerDescriptor};
use fin_framework::mcp::McpClient;

struct ComputerUsePeer {
    mcp_client: McpClient,
    mailbox: Mailbox,
    descriptor: PeerDescriptor,
}

impl Peer for ComputerUsePeer {
    async fn start(&mut self) {
        self.mcp_client.connect("computer-use").await;
        self.mailbox.register_peer(&self.descriptor).await;
        self.poll_loop().await;
    }
    
    async fn handle_message(&mut self, msg: MailboxMessage) {
        match msg.message_type.as_str() {
            "mcp_tool_call" => {
                let result = self.mcp_client.call_tool(
                    &msg.tool_name, &msg.tool_params
                ).await;
                self.mailbox.reply(msg.message_id, result).await;
            }
            _ => log::warn!("Unknown message type: {}", msg.message_type),
        }
    }
}
```

### 步骤 2: 在 fin 工具链中添加 MCP 调用工具

```rust
// fin/tools/src/mcp_call.rs
#[tool(name = "mcp_call")]
async fn mcp_call(
    peer_id: String,
    tool_name: String,
    tool_params: JsonValue,
    timeout_sec: Option<u32>,
) -> ToolResult {
    let peer_status = peer.describe(&peer_id).await?;
    if peer_status.presence == "offline" {
        daemon.ensure_peer(&peer_id).await?;
    }
    let msg_id = mailbox.send(&peer_id, McpToolCallRequest {
        tool_name, tool_params,
        timeout_sec: timeout_sec.unwrap_or(120),
    }).await?;
    let result = mailbox.wait_for_response(&msg_id, timeout_sec).await?;
    Ok(result)
}
```

### 步骤 3: 配置 peer 拓扑

```toml
# fin/config/peers.toml
[[peers]]
peer_id = "computer-use-peer"
peer_kind = "mcp_proxy"
enabled = true

[peers.daemon]
command = "npx"
args = ["-y", "@anthropic-ai/computer-use"]
env = { DISPLAY = ":0" }
health_check_interval_sec = 30
auto_restart = true

[peers.capabilities]
tools = ["screenshot", "click", "type", "scroll"]
max_concurrent_sessions = 4
```

## 7. 优势总结

| 特性 | 说明 |
|------|------|
| 拓扑感知 | peer.list 实时知道 computer-use-peer 状态 |
| 生命周期管理 | daemon.ensure_peer 自动启动/恢复 MCP server |
| 异步解耦 | mailbox 机制支持异步调用，不阻塞 worker |
| 容量控制 | peer descriptor 声明 max_concurrent_sessions |
| 可观测性 | 每次调用通过 trace_id 追踪完整链路 |
| 容错恢复 | Daemon 自动重启失败的 MCP server 进程 |

## 8. 关键 Codex 源码参考

| 文件 | 用途 | 移植价值 |
|------|------|---------|
| `rmcp-client/src/rmcp_client.rs` | MCP 协议客户端核心 | ★★★★★ 直接复用 |
| `codex-mcp/src/mcp_connection_manager.rs` | 连接管理与工具路由 | ★★★★☆ 参考架构 |
| `codex-mcp/src/mcp/mod.rs` | 配置与快照收集 | ★★★☆☆ 参考设计 |
| `codex-mcp/src/mcp_tool_names.rs` | 工具名处理 | ★★☆☆☆ 逻辑简单 |
| `codex-mcp/src/mcp/auth.rs` | OAuth 认证 | ★★★★☆ 如需认证则必需 |

## 9. 后续扩展

- 支持多个 MCP server peer（如 `browser-use-peer`, `file-system-peer`）
- 增加消息优先级队列（高优先级截图请求优先处理）
- 支持 MCP 工具结果缓存（相同参数的截图可复用）
