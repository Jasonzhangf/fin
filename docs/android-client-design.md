# Android 客户端设计（fin MVP，先文档后编码）

## 0. 执行顺序（强约束）
1. **先修改文档**（本文件 + 验证矩阵）
2. 文档评审通过后再编码
3. 每阶段编码前先补对应章节的“验收标准”

> 本次按 Jason 要求：先文档，再实现。

---

## 1. 目标与边界
构建 Android 客户端 MVP，满足：
- session 管理
- 输入管理
- 推理 turn 渲染
- worker/project/runtime 状态可观测
- 本地 WS 建连过程可见
- 以 runtime truth 为唯一事实源

### 1.1 与 WebUI 的关系（更新）
- Android 客户端**优先复用 WebUI 语义与协议**（同一套 runtime truth / turn 语义 / 状态约束）。
- 若仓库内不存在可直接复用的 WebUI 实现文件（当前 `web/` 仅 README），则采用：
  - Android 壳 + Bridge + 可替换前端容器；
  - 接口与 contract 与目标 WebUI 对齐；
  - 后续接入真实 WebUI bundle 时不改 runtime contract。

---

## 2. 连接层设计
### 2.1 状态机
`idle -> scanned -> resolving -> connecting -> handshaking -> subscribed -> healthy`

异常态：
- `auth_failed`
- `endpoint_unreachable`
- `stale_token`
- `protocol_mismatch`

### 2.2 二维码 payload（contract）
```json
{
  "v": 1,
  "endpoint": "wss://192.168.1.10:4040/ws",
  "device_id": "mbp-fin-local",
  "token": "short_lived_jwt",
  "exp": 1715750400,
  "project": "fin",
  "scopes": ["session.read", "session.write", "runtime.read"]
}
```

### 2.3 多 profile 持久化
至少两套：`local` / `dev`。

---

## 3. Session 管理
### 3.1 页面
- 任务列表页
- 会话详情页（Conversation / Timeline / Runtime / Connection）

### 3.2 能力
- 列表、过滤、切换、恢复
- 展示字段：`session_id/task_id/topic/phase/updated_at`
- daemon 重启后恢复同一 session，禁止串台

---

## 4. 输入管理
状态机：
`draft -> queued -> consumed -> rendered -> delivered`

去重键：
`source + client_message_id + timestamp_bucket`

约束：重连/重启后不得重复补发。

---

## 5. Turn 渲染
Turn 卡片：
1. user input
2. assistant response
3. control feedback 摘要
4. tool execution 摘要
5. closure-stop source

模式：
- Compact（默认）
- Debug
- Full Trace（后置）

约束：禁止前端伪造业务语义，必须来自 WS/runtime 数据。

---

## 6. 状态可观测
### 6.1 Runtime 页
- worker：online/offline/busy/idle/heartbeat
- project：supervision action/wake queue/pickup state
- daemon：lifecycle/recovery/stale lease

### 6.2 Connection 页
- 握手阶段
- 订阅明细
- 最近错误与恢复动作

---

## 7. 反刷屏硬门禁
1. 相同 snapshot 不重复发送 activity
2. ready/idle 且无新进展时禁止周期重复卡片
3. binding mismatch 必须清理 delivery binding，禁止旧 target 投递

---

## 8. 实施阶段计划（文档先行）
### Phase A（文档完成，当前阶段）
- [x] 设计文档修订
- [x] WebUI 复用策略与缺失场景说明
- [x] 验证矩阵定义

### Phase B（骨架编码）
- Android 壳
- Bridge
- profile 持久化
- 最小页面骨架

### Phase C（功能编码）
- WS 建连全流程
- session/input/turn/runtime 渲染
- 反刷屏约束落地

### Phase D（验证收口）
- 连接、恢复、渲染、一致性、反刷屏回归
- 证据产出（日志 + 截图 + 步骤）

---

## 9. 最小验证矩阵（先定义，后执行）
1. 连接：状态机 happy path + 异常态 4 类
2. session 恢复：daemon 重启后同 session id
3. turn 渲染：卡片字段与 runtime truth 一致
4. 状态一致性：worker/project/daemon 与 runtime 投影一致
5. 反刷屏：相同 snapshot 不重复发送；idle 无新进展不刷；binding mismatch 清理成功

---

## 10. 交付要求
每阶段必须回报：
- 改动文件列表
- 验证命令
- 结果
- 剩余风险
- 下一步

无证据不宣称完成。


## 11. WebUI 复用实施（webui_debug -> webui_core）
### 11.1 目标
在不改 runtime truth contract 的前提下，复用现有 webui_debug 的事实消费链，抽离非 debug 的用户交互核心（webui_core）。

### 11.2 拆分边界
- `webui_core`（用户核心）
  - connection 状态机与握手订阅
  - session 列表/过滤/切换/恢复
  - 输入队列与去重
  - turn 渲染（compact/debug）
  - runtime 状态（worker/project/daemon）
- `webui_debug`（调试壳）
  - raw event 瀑布
  - trace 深度诊断面板
  - 调试专用注入工具

### 11.3 单真源约束
1. webui_core 与 webui_debug 必须共享同一套 store/transport/render 内核。
2. Android 仅承载与桥接，不重复实现业务语义。
3. runtime 语义只来自 WS 事件，不允许 UI 侧推断补齐。

### 11.4 Android 复用接入点
- WebView 默认入口：`webui_core` 页面。
- Native Bridge 提供：扫码、profile 持久化、设备信息。
- 连接参数通过 bridge/query 注入，不新增第二套协议。

### 11.5 验收增补
- 同一条 WS 事件在 webui_core 与 webui_debug 的关键字段渲染一致。
- Android 端切换 profile 后连接态与 webui_debug 观测一致。
