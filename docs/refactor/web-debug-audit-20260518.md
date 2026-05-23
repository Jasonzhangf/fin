# Web Debug 与 Daemon 耦合审计 — 2026-05-18

## 审计背景
Fin 当前由 debug-server 同时承载业务与调试职责。本文档记录 As-Is 拓扑、耦合点与风险，作为后续解耦重构真源。

## 1. 入口与端口现状

### 1.1 当前入口
| 入口 | 地址 | 职责 |
|---|---|---|
| HTTP/WS | `fin-cli web-debug ... 4040` | 混合：业务+debug |

> 关键事实：当前只有一个公开入口，business/debug 未分层。

### 1.2 主要路由
| 路由 | 归属文件 | 角色 |
|---|---|---|
| `/ws` | `mobile_ws.rs` | 业务（移动会话主链） |
| `/api/chat/send` | `chat_api.rs` | 业务 |
| `/api/*`（projection/context/events） | `routes.rs` | 观测/调试 |
| `/api/watch` | `event_stream.rs` | 调试 SSE |
| `/updates/latest.json`、`/updates/*.apk` | `routes.rs`（本轮新增） | 升级分发 |

## 2. 数据真源
| 数据 | 写入口 | 读入口 |
|---|---|---|
| runtime sessions/turn/tool | daemon runtime | business/debug 共享 |
| `android-client/update-dist` | build publish 脚本 | 客户端升级检查/下载 |

## 3. 当前耦合点
| 编号 | 耦合点 | 风险 |
|---|---|---|
| C1 | business/debug 共用同一 listener 与路由层 | 高 |
| C2 | 更新分发路由直接加在 debug-server routes | 中 |
| C3 | 历史上存在独立 `python http.server:8080` 方案 | 中 |

## 4. 审计结论
1. Web debug 与业务主链目前未清晰分层。
2. 更新分发需要成为“业务平面能力”，而非 debug 附属能力。
3. 需进行路由分层重构，保证 debug 只是观测增强，不绑架业务架构。
