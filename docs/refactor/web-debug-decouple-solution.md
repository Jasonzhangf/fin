# Web Debug 与 Daemon 解耦重构方案（v1）

## 目标
让 **business plane** 与 **debug plane** 平行：
- business plane：会话/推理/升级分发（主链）
- debug plane：观测与诊断（增强）
- 共享同一 runtime 真源，不复制业务语义

## 设计原则
1. debug 不承载业务唯一入口
2. 更新服务归 business plane
3. 无 fallback 双真源
4. 先分层再收口清理

## 分期计划

### Phase 1：路由分类与 owner 明确
- 抽象路由 owner：`business` / `debug`
- 将 `/updates/*` 标记为 business
- 验收：路由表与实现一致，owner 可审计

### Phase 2：入口分层
- 保持同端口可兼容，但内部 dispatcher 分 business/debug 两套 handler
- business handler 不依赖 debug 专用结构
- 验收：关闭 debug 观测能力不影响业务链

### Phase 3：升级链路归位
- `latest.json` 与 APK 文件只由 business handler 分发
- 客户端默认 manifest 指向 business 地址（4040 /updates/latest.json）
- 验收：检查更新/下载/安装闭环通过

### Phase 4：清理旧路径
- 删除独立 8080 脚本服务依赖
- 删除历史 debug 专用升级路径
- 验收：代码库无重复升级实现

## 验收矩阵
1. WS 会话主链路不回退
2. `/updates/latest.json` 返回有效 manifest
3. `/updates/*.apk` 可下载
4. debug 观测 API 正常
5. 关闭 debug 观测功能后，business 仍正常
