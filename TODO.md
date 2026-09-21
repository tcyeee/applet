# TODO：AI-Native Desktop Runtime

根据 [IDEA.md](./IDEA.md) 拆解的开发任务清单。MVP 方向：**Tauri + SQLite + MCP + App Registry + 声明式 UI Runtime + 数据持久化**。

## 0. 项目初始化

- [x] 技术选型确认：Tauri 2 + React + TypeScript（渲染声明式 UI 用什么，如 React/Svelte/纯 Web Components）
- [x] 初始化 Tauri 项目骨架（Rust 后端 + 前端）
- [x] 确定 monorepo / 目录结构：暂不拆分 monorepo，先用单一 Tauri 项目（`src-tauri/` + `src/`），后续模块变多再拆
- [x] 基础 CI（lint、build、test）：GitHub Actions，前端跑 eslint/vitest/vite build，后端跑 cargo fmt/clippy/test

## 1. 声明式 App 定义（App Schema）

- [x] 设计 App 的声明式描述格式（JSON/YAML/DSL），至少覆盖：
  - [x] 数据模型定义（表结构、字段类型、关系）
  - [x] 页面/视图定义（列表、表单、详情、统计图表等常见组件）
  - [x] 业务逻辑/交互定义（增删改查、简单计算、条件展示）
  - [x] 自动化/定时任务定义
- [x] 编写 Schema 校验器（版本化，便于后续升级不破坏已有 App）
- [x] 编写 2-3 个手写示例 App 定义（如记账工具）用于驱动开发

> 实现见 `src/app-schema/`（`types.ts` + `schema.ts` 为 TS 类型/Zod 校验规则，`validate.ts` 为统一校验入口，`examples/` 下为 `bookkeeping.json` / `habit-tracker.json` 两个手写示例）。规则与约定见 [AGENTS.md](./AGENTS.md)。

## 2. Runtime Core

- [x] App 生命周期管理：安装、启动、停止、卸载、更新
- [x] App Registry：本地已安装 App 的元数据管理（版本、依赖、状态）
- [x] 内置 SQLite 集成：每个 App 独立 database 文件/命名空间
- [x] 数据迁移机制：App 定义变更后如何安全迁移已有数据（新增字段、改类型等）
- [ ] 文件存储模块：App 私有存储目录、跨 App 共享存储的权限控制
- [ ] 权限管理：App 之间的数据/文件隔离，用户可见的权限授权 UI
- [x] 备份与恢复：全量/单 App 备份，导出为可迁移的归档文件，一键恢复
- [x] Scheduler：支持 App 内定义的定时任务（如每日汇总、提醒）

> 实现见 `src-tauri/src/runtime/`（`registry.rs` App Registry、`appdb.rs` 每
> App 独立 SQLite + schema 迁移、`storage.rs` 文件存储、`backup.rs` 备份/恢复、
> `scheduler.rs` cron 触发器扫描），对外通过 `src-tauri/src/commands.rs` 的
> Tauri command 暴露。`app_schema.rs` 是 App Schema `dataModel`/`automations`
> 的 Rust 镜像，仅用于生成 SQL 和读取 cron 表达式，**不是**第二个校验器——
> 校验仍然只能通过 TS 的 `validateAppDefinition`（见 AGENTS.md）。
>
> 未完成/已知缺口（文件存储与权限管理两项打勾未全部完成）：
> - App 私有目录与 `shared/` 目录已实现且互相隔离，但共享目录当前对所有已安装
>   App 开放，还没有按 App 授权访问的权限模型——这需要 App Schema 新增
>   `permissions` 字段，目前 schema 里还没有这个概念。
> - "用户可见的权限授权 UI" 属于声明式 UI Runtime（步骤 3）的工作，尚未开始。
> - App Registry 的"依赖"字段未建模：App Schema 本身目前没有 App 间依赖的
>   概念，等该概念出现后再补充。
> - Scheduler 只负责"检测 cron 触发并派发事件"（`runtime://automation`），
>   真正执行 `runAction` / `summarize` / `notify` 需要步骤 4 的 action 执行引擎。
> - 数据迁移默认拒绝破坏性变更（删字段/删实体/改字段类型），需要显式传
>   `force: true` 才会尝试重建表并尽量保留数据（类型转换用 `CAST`）；`force`
>   下删除的字段/实体数据仍会丢失，这是预期行为而非 bug。

## 3. 声明式 UI Runtime（渲染层）

- [x] 根据 App Schema 动态渲染页面（列表、表单、详情、图表等基础组件库）
- [x] 组件与数据绑定机制（读取/写入 SQLite，触发校验）
- [x] 导航/路由：多页面 App 内的页面跳转
- [x] 主题/样式的默认规范，保证不同 AI 生成的 App UI 风格统一
- [x] 错误态、空态、加载态的默认处理，降低 AI 生成时的心智负担

> 实现见 `src/ui-runtime/`：`AppShell` 左侧导航 + 自研内存态视图栈
> （`router.ts`，不依赖浏览器 URL/react-router）驱动 `ListView`/`FormView`/
> `DetailView`/`ChartView` 四种视图组件；`useEntityRecords` 封装新增的
> Tauri `list_records`/`create_record`/`update_record`/`delete_record`
> command 做数据绑定；`fieldSchema.ts` 按 `dataModel` 字段类型动态生成 Zod
> schema 供 `react-hook-form` 校验；`computeExpression.ts` 是 `Action`
> `type: "compute"` 的安全四则运算求值器，在表单里对依赖字段联动实时计算
> `targetField`；`aggregate.ts` 供 `ChartView` 按 `chart.groupBy`/`aggregate`
> 做客户端聚合（`recharts` 渲染）。UI 组件库用 Tailwind v4 + 手写的
> shadcn/ui 组件（`src/components/ui/`，`components.json` 声明其
> 约定）——同一套组件是所有 AI 生成 App 唯一能渲染出的视觉语言，风格统一由
> 渲染层强制保证，App Schema 本身不携带任何样式信息。`EmptyState`/
> `ErrorState`/`LoadingState` 是三种视图共用的默认态。
>
> 前置补充：Rust 侧原本只有建表/迁表，没有读写数据行的能力，本步骤在
> `src-tauri/src/runtime/appdb.rs` 补上了 `list_records`/`insert_record`/
> `update_record`/`delete_record`（`get_record` 供内部查找用），通过
> `src-tauri/src/commands.rs` 暴露的四个 Tauri command 调用；字段级类型转换
> 镜像 `sql_type()` 的规则，未知字段名拒绝，权当纵深防御。
>
> 已知缺口：
> - many-cardinality 引用字段的 link 表读写未实现（两个示例 App 都未使用）。
> - `reference: "one"` 字段的表单 Select 用目标实体的 `id` 作为选项文案，
>   还没有"取哪个字段当展示名"的约定。
> - Detail/Edit/Chart 都是拉取该实体全部记录后在前端按 id 过滤或聚合，没有
>   单条查询或服务端聚合的 Tauri command——数据量大的 App 会是性能瓶颈。
> - `automations` 里 `runAction`/`notify` 的执行、以及 App 间导航之外更复杂
>   的交互，仍然是步骤 4（MCP/action 执行引擎）的工作。
> - 本机验证受限于沙箱环境没有 Accessibility 权限，只跑通了
>   `pnpm tauri dev` 启动后的截图确认（App 列表页渲染正常），记账示例的
>   新增/编辑/删除/图表完整点击流程需要人工过一遍。

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
