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
### 2.0 唯一链路图（冻结）
```text
Android Client / WebUI / QQ Bot
            │
            │  WS (same contract, same event stream)
            ▼
         daemon (:4040/ws)
            │
            │  runtime/session projection (single source of truth)
            ▼
   ~/.fin/runtime + session durable artifacts
```

约束：
1. 所有 channel 都是同一 daemon 的适配层，不是多真源。
2. channel 不做业务语义推断；只消费 daemon 结构化事件。
3. 历史渲染来自 session 持久化，WS 仅用于增量实时事件。

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

## 12. Session 真源与默认绑定（冻结）
### 12.1 默认绑定规则
1. WebUI / Android / QQ BOT 只是 channel，默认启动会话统一绑定 `system agent`。
2. 客户端启动后先尝试恢复本地 `last_session_id`；若不存在或失效，回退到 `session-system-entry`。
3. channel adapter 不拥有会话业务语义；只做接入、订阅、渲染。

### 12.2 Session 与 Ledger 关系（冻结语义）
1. Agent 只感知 session，不感知 ledger。
2. 底层采用 `track = session_id`：
   - 逻辑上单一 ledger truth（append-only event chain）
   - 物理上按 session track 分轨写入，互不串写
3. UI 渲染：
   - 历史来自 session 持久化/投影
   - WS 仅提供增量事件
4. 删除/清理 session 不得破坏已沉淀知识（见 14 节）。

### 12.3 多 session 恢复与切换
1. 启动后自动连接 daemon，拉取 session 列表并绑定默认会话。
2. session 列表按 `updated_at desc` 排序。
3. 用户可手动切换 `session_id`；切换后：
   - 更新本地 `last_session_id`
   - 只订阅/渲染当前绑定 session 的增量
4. 每个 session 展示：
   - `session_id`
   - `updated_at`
   - `title`（自动或手工）
   - `preview_100`（最近内容 100 字）

## 13. Session CRUD 命令与 UI 编辑
### 13.1 命令面（CLI/Channel）
- 已有：
  - `/new`：创建并切换新 session
  - `/resume <session_id>`：恢复并切换到指定 session
- 规划新增（冻结 contract，后续实现）：
  - `/sessions`：列出可见 session（含 title/updated_at/preview）
  - `/session rename <session_id> <title>`：手工改名
  - `/session delete <session_id> [--force]`：删除 session（默认先做精华提取）
  - `/session archive <session_id>`：归档，不在默认列表显示
  - `/clear`：仅清理当前前端显示缓冲，不删除 ledger/session truth

### 13.2 UI 面（Android/Web）
1. Session 列表页支持：
   - 切换
   - 重命名（编辑 title）
   - 删除（带“先提取精华”确认）
   - 归档/取消归档
2. 操作反馈必须结构化显示（成功/失败/原因/证据路径）。

## 14. Knowledge Base 目录与“先提取再删除”
### 14.1 目录结构（project 级）
```text
~/.fin/projects/<project_id>/knowledge-base/
  entries/
    kb-<id>.json
  indexes/
    by_session.json
    by_topic.json
```

### 14.2 精华提取来源
模型 control block 中已存在可用于沉淀的字段（当前已见）：
- `note_candidate`
- `digest_candidate`
- `reason`
- 以及 closure/tool/error 相关结构化证据

后续可扩展 `learning` 字段，但不阻塞当前流程。

### 14.3 删除前流程（强约束）
`session delete` 默认流程：
1. 扫描该 session 最新 N 轮 control block / digest / tool receipt
2. 自动生成 knowledge entries（自动提取）
3. 允许用户补充/编辑（手工提取）
4. 写入 knowledge-base 并建立 `session_id -> kb_entry_ids` 索引
5. 仅在 2~4 成功后，才允许删除 session（除 `--force`）

### 14.4 验收要求
1. 删除 session 后，knowledge-base 中可检索到对应精华条目。
2. 任一 channel（Web/Android/QQ）删除会话时流程一致。
3. 无精华落盘证据，不允许宣称“删除完成”。
