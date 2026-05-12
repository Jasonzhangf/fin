# Codex Computer Use 移植到 Fin 方案

## 1. Codex Computer Use 实现分析

### 1.1 架构定位

Codex 的 Computer Use **不是内置功能**，而是通过 MCP 协议集成的外部服务：

```
Codex (Rust)
    ↓ MCP 协议调用
computer-use MCP Server (外部)
    ↓ 实际操作
屏幕截图 / 鼠标点击 / 键盘输入
```

### 1.2 核心源码路径

```
~/code/codex/codex-rs/
├── rmcp-client/src/rmcp_client.rs          # MCP 客户端实现（核心）
├── codex-mcp/src/mcp_connection_manager.rs # 连接管理器
├── codex-mcp/src/mcp/mod.rs               # 配置与 server 发现
├── codex-mcp/src/mcp/auth.rs              # OAuth 认证
└── codex-mcp/src/mcp_tool_names.rs        # 工具名限定与去重
```

### 1.3 Computer Use 在 Codex 中的配置

```rust
// codex-mcp/src/mcp/mod.rs
pub const COMPUTER_USE_MCP_SERVER_NAME: &str = "computer-use";

// 通过配置发现 MCP server
pub fn effective_mcp_servers(config, auth) -> HashMap<String, McpServerConfig> {
    let servers = configured_mcp_servers(config);
    with_codex_apps_mcp(servers, auth, config)  // 添加 computer-use server
}
```

### 1.4 MCP 客户端关键能力 (`rmcp_client.rs`)

| 能力 | 实现方式 |
|------|---------|
| **传输协议** | `StreamableHttp` (SSE) 或 `Stdio` |
| **OAuth 认证** | Token 管理 + 自动刷新 |
| **超时控制** | 工具调用 120s，启动 30s |
| **响应类型** | SSE 事件流 / JSON 直接响应 / Accepted 异步确认 |
| **工具名处理** | `mcp__server__tool` 格式，SHA1 哈希去重 |

### 1.5 Computer Use 工具列表

通过 MCP 协议暴露的典型工具：
- `screenshot` - 屏幕截图
- `click` - 鼠标点击
- `type` - 键盘输入
- `scroll` - 滚动
- `emitImage` - 图像输出（配合 js_repl）

---

## 2. 移植到 Fin 的方案

### 2.1 方案选择：Fin Peer MCP 代理模式

将 `computer-use` MCP server 作为 fin 框架中的独立 peer 节点：

```
fin peer 拓扑:
┌─────────────────┐       ┌──────────────────────┐
│  System (调度器) │       │ computer-use-peer    │
│  peer.list      │◄─────►│ (MCP Server 代理)     │
│  daemon.ensure  │       │  ├─ screenshot       │
│  agent.assign   │       │  ├─ click            │
└────────┬────────┘       │  ├─ type             │
         │                │  └─ scroll           │
         │                └──────────────────────┘
┌────────▼────────┐
│  worker-01/02   │
│  mailbox.poll   │
└─────────────────┘
```

### 2.2 实现步骤

#### 步骤 1: 创建 computer-use-peer 代理模块

```
fin/peers/computer-use-peer/
├── Cargo.toml
├── src/main.rs              # Peer 入口
├── src/mcp_client.rs        # 封装 rmcp-client
└── src/handler.rs           # Mailbox 消息处理
```

**核心逻辑**:
```rust
// 1. 启动时通过 daemon 配置启动 computer-use MCP server
// 2. 连接 MCP server (Stdio 或 StreamableHttp)
// 3. 注册 peer descriptor 到 fin 拓扑
// 4. 监听 mailbox，转发 MCP 工具调用
```

#### 步骤 2: Peer 描述符配置

```toml
# fin/config/peers.toml
[[peers]]
peer_id = "computer-use-peer"
peer_kind = "mcp_proxy"
enabled = true

[peers.daemon]
command = "npx"  # 或实际 computer-use server 路径
args = ["-y", "@anthropic-ai/computer-use"]
env = { DISPLAY = ":0" }
health_check_interval_sec = 30
auto_restart = true

[peers.capabilities]
tools = ["screenshot", "click", "type", "scroll"]
max_concurrent_sessions = 4
transport = "stdio"  # 或 "streamable_http"
```

#### 步骤 3: Mailbox 消息协议

**请求** (worker → computer-use-peer):
```json
{
  "message_type": "mcp_tool_call",
  "message_id": "msg-uuid",
  "tool_name": "screenshot",
  "tool_params": {"monitor": 0},
  "requester_id": "worker-01",
  "timeout_sec": 120
}
```

**响应** (computer-use-peer → worker):
```json
{
  "message_type": "mcp_tool_result",
  "message_id": "msg-uuid",
  "status": "success",
  "result": {"image_path": "/tmp/screenshot.png"},
  "duration_ms": 850
}
```

#### 步骤 4: Worker 调用工具

Worker 通过标准 fin 工具链调用：
1. `peer.list` 检查 computer-use-peer 状态
2. 如果离线，调用 `daemon.ensure_peer` 唤醒
3. `mailbox.send` 发送工具调用请求
4. `mailbox.poll` 等待响应（或 `wait.remind` 异步等待）

### 2.3 移植价值评估

| Codex 组件 | 移植方式 | 优先级 |
|-----------|---------|--------|
| `rmcp-client` crate | 直接作为 fin 依赖 | ★★★★★ |
| `McpConnectionManager` | 参考架构，在 peer 中实现 | ★★★★☆ |
| OAuth 认证 | 按需移植 | ★★★☆☆ |
| 工具名去重逻辑 | 简化实现 | ★★☆☆☆ |

---

## 3. 执行计划

| 阶段 | 任务 | 输出 |
|------|------|------|
| 1 | 创建 computer-use-peer 模块骨架 | 代码结构 |
| 2 | 集成 rmcp-client 依赖 | Cargo.toml 更新 |
| 3 | 实现 MCP 连接与工具调用 | mcp_client.rs |
| 4 | 实现 Mailbox 消息处理 | handler.rs |
| 5 | 配置 peer 拓扑 | peers.toml |
| 6 | 测试截图工具调用 | 端到端测试 |
| 7 | 文档与示例 | README + 使用指南 |

---

## 4. 风险与依赖

- **依赖**: 需要 `computer-use` MCP server 可用（本地或远程）
- **风险**: 图形环境依赖（DISPLAY/:0），macOS 需要辅助功能权限
- **缓解**: 提供 fallback 模式，无图形环境时优雅降级
