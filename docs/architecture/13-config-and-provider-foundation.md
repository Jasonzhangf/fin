# 13 Config and Provider Foundation

本文档定义 `fin` 的两个 M1 基础模块：

1. **Config Module**
2. **AI Provider Module**

它们是 M1 第一阶段必须先稳定的底座。没有这两个模块，后续 runtime / debug / task bootstrap 都会返工。

## 1. 配置模块总原则

### 1.1 配置文件统一支持注释

`fin` 的配置文件统一采用 **支持注释的格式**。

当前默认选型：

- `user.toml`
- `system.toml`

选择 TOML 的原因：

- 原生支持注释
- 语义稳定，适合配置文件
- 比 YAML 更不容易因格式灵活而出错
- 比 JSON/JSONC 更适合作为长期配置真源

### 1.2 双层配置模型

`fin` 采用双层配置：

#### `user.toml`

用户唯一手写配置入口。

特点：

- 格式固定
- 暴露面尽量小
- 用户只填必须由用户决定的信息

#### `system.toml`

系统配置单文件。

特点：

- 不作为普通用户配置入口
- 可以由框架自动生成模板 / 标准化结果
- 可配置，但不面向普通用户频繁手改
- 是系统模块配置的集中位置

### 1.3 不做 merge

`user.toml` 与 `system.toml` **不做 merge**。

只允许：

```text
user.toml
-> ConfigMapper / Normalizer
-> system.toml / runtime config view
```

规则：

- 用户配置项与系统配置项尽量互斥
- 一个配置语义只在一层出现
- 避免双源覆盖与 merge 引发的歧义

## 2. Config Module 分层

### `UserConfig`

用户层配置对象。

职责：

- 读取 `user.toml`
- 校验用户输入是否合法
- 只表示用户需要决定的最小配置

### `SystemConfig`

系统层配置对象。

职责：

- 表示系统各模块的统一配置
- 为 runtime / routing / digest / debug / provider 提供统一读取入口

### `ConfigMapper`

唯一允许的用户配置到系统配置转换层。

职责：

- 从 `UserConfig` 生成标准化 `SystemConfig`
- 填充默认值
- 做必要规范化与互斥校验

注意：

- `ConfigMapper` 不是 merge engine
- 它是 mapping / normalization engine

## 3. 用户配置最小暴露面

M1 阶段，`user.toml` 只暴露最小必要项，例如：

- 默认 provider
- provider 协议类型
- provider `base_url`
- `api_key` 或 `api_key_env`
- 默认 model

当前项目启动默认值冻结为：

- `default_provider = "ali-coding-plan"`
- `protocol = "anthropic-wire"`
- `base_url = "https://coding.dashscope.aliyuncs.com/apps/anthropic"`
- `model = "qwen3.6-plus"`
- 优先写 `api_key_env = "ALI_CODINGPLAN_KEY"`，避免把真实 key 落进 `user.toml`

用户配置当前不应暴露：

- digest / rebuild 细节
- routing 阈值
- heartbeat 参数
- projection/debug 参数
- task/topic policy 细节

这些属于 `system.toml` 的系统层配置。

## 4. 系统配置单文件原则

系统配置统一进入一个文件：

- `system.toml`

目的：

- 避免多个零散配置文件分散
- 避免修改时误改、漏改
- 便于 debug、导出、模板生成、版本迁移

M1 阶段可纳入 `system.toml` 的配置域包括：

- runtime
- routing
- digest / rebuild
- health / heartbeat
- projection / debug
- provider resolved config

## 5. AI Provider 模块总原则

Provider 模块必须是独立基础设施层，不能污染 runtime / task / topic 语义。

目标：

- 多协议支持
- 统一 provider 描述
- 统一注册与调用
- 与 task/session/topic 解耦

## 6. Provider 模块边界

### Provider 模块负责

- provider 类型定义
- provider descriptor / normalized config
- provider registry
- protocol adapter
- provider client 统一调用入口

### Provider 模块不负责

- task / topic routing
- session lifecycle
- dispatch / progress
- digest / rebuild 语义

## 7. 多协议支持策略

`fin` 在项目一开始就要求 **架构上支持多协议**。

M1 推荐策略：

- 独立出多协议 provider 模块
- 先完成基础抽象与模块边界
- 后续协议接入按模块迭代扩展

当前默认协议方向：

- `openai-compatible`
- `anthropic-wire`
- 其他协议后续扩展

## 8. 可参考来源

`finger` 中已有的 provider 体系可作为参考来源，但 `fin` 中必须重新保持清晰边界。

可参考思想包括：

- provider types
- provider factory
- provider registry
- protocol adapters
- 用户配置到 provider 标准化配置的加载逻辑

但不应直接把 `finger` 的项目语义耦合带入 `fin`。

## 9. M1 中 Config + Provider 的位置

在 M1 的实现顺序中，这两个模块必须优先于：

- inference kernel
- recording/context
- debug server
- tentative session formalization

理由：

- provider 依赖 config
- runtime 依赖 provider
- debug 与 task bootstrap 依赖统一配置与模型接入

## 10. 当前非目标

以下属于后续模块阶段再展开：

- 具体字段命名最终版
- 配置迁移策略细节
- provider 协议高级能力差异
- 各 provider 的完整 capability matrix
- UI 配置编辑器