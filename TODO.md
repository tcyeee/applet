# TODO：AI-Native Desktop Runtime

根据 [IDEA.md](./IDEA.md) 拆解的开发任务清单。MVP 方向：**Tauri + SQLite + MCP + App Registry + 声明式 UI Runtime + 数据持久化**。

## 0. 项目初始化

- [x] 技术选型确认：Tauri 2 + React + TypeScript（渲染声明式 UI 用什么，如 React/Svelte/纯 Web Components）
- [x] 初始化 Tauri 项目骨架（Rust 后端 + 前端）
- [x] 确定 monorepo / 目录结构：暂不拆分 monorepo，先用单一 Tauri 项目（`src-tauri/` + `src/`），后续模块变多再拆
- [x] 基础 CI（lint、build、test）：GitHub Actions，前端跑 eslint/vitest/vite build，后端跑 cargo fmt/clippy/test

## 1. 声明式 App 定义（App Schema）

- [ ] 设计 App 的声明式描述格式（JSON/YAML/DSL），至少覆盖：
  - [ ] 数据模型定义（表结构、字段类型、关系）
  - [ ] 页面/视图定义（列表、表单、详情、统计图表等常见组件）
  - [ ] 业务逻辑/交互定义（增删改查、简单计算、条件展示）
  - [ ] 自动化/定时任务定义
- [ ] 编写 Schema 校验器（版本化，便于后续升级不破坏已有 App）
- [ ] 编写 2-3 个手写示例 App 定义（如记账工具）用于驱动开发

## 2. Runtime Core

- [ ] App 生命周期管理：安装、启动、停止、卸载、更新
- [ ] App Registry：本地已安装 App 的元数据管理（版本、依赖、状态）
- [ ] 内置 SQLite 集成：每个 App 独立 database 文件/命名空间
- [ ] 数据迁移机制：App 定义变更后如何安全迁移已有数据（新增字段、改类型等）
- [ ] 文件存储模块：App 私有存储目录、跨 App 共享存储的权限控制
- [ ] 权限管理：App 之间的数据/文件隔离，用户可见的权限授权 UI
- [ ] 备份与恢复：全量/单 App 备份，导出为可迁移的归档文件，一键恢复
- [ ] Scheduler：支持 App 内定义的定时任务（如每日汇总、提醒）

## 3. 声明式 UI Runtime（渲染层）

- [ ] 根据 App Schema 动态渲染页面（列表、表单、详情、图表等基础组件库）
- [ ] 组件与数据绑定机制（读取/写入 SQLite，触发校验）
- [ ] 导航/路由：多页面 App 内的页面跳转
- [ ] 主题/样式的默认规范，保证不同 AI 生成的 App UI 风格统一
- [ ] 错误态、空态、加载态的默认处理，降低 AI 生成时的心智负担

## 4. MCP 接口层

- [ ] 设计 MCP tool 集合，覆盖：
  - [ ] 创建/更新/删除 App（写入声明式定义）
  - [ ] 查询已安装 App 列表与详情
  - [ ] 数据库 CRUD 操作（面向 App 数据，而非任意 SQL，防止越权）
  - [ ] 文件存储读写
  - [ ] 触发备份/恢复
  - [ ] Scheduler 任务的增删查
- [ ] 权限与安全边界：MCP 侧的操作范围限制、危险操作确认机制
- [ ] 编写 MCP server 的对接文档，方便 Agent（如 Claude）调用

## 5. 端到端打通（MVP 验收）

- [ ] 场景一：通过 AI 对话创建一个"个人记账工具"（含数据表、录入表单、列表页）
- [ ] 场景二：追加需求"增加月度统计"，验证声明式变更 + 数据迁移不丢数据
- [ ] 场景三：追加需求"增加预算功能"，验证多次迭代的可维护性
- [ ] 场景四："帮我备份数据"，验证备份恢复闭环
- [ ] 场景五：kill 掉 Runtime 重新打开，验证数据持久化与 App 状态恢复

## 6. 打包与分发

- [ ] Tauri 跨平台打包（macOS / Windows / Linux）
- [ ] 应用自更新机制（Runtime 本身的版本升级，区别于 App 内数据迁移）
- [ ] 首次安装引导（初始化本地数据库、配置 MCP 连接）

## 7. 待决策 / 需要进一步澄清的问题

- [ ] Runtime 与 AI Agent 的连接方式：本地 MCP server 常驻，还是按需启动？
- [ ] 多个 App 之间是否允许数据互通（例如记账 App 和日历 App 共享数据）？
- [ ] 声明式 UI 的表达能力边界：遇到 Schema 无法表达的复杂交互时如何降级（是否允许嵌入自定义代码片段）？
- [ ] 是否需要支持多用户/多设备同步，还是先聚焦单机单用户？
