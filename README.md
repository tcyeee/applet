# Applet

**AI-Native Desktop Runtime** — AI 负责创造软件，Applet 负责让软件真正运行起来。

Applet 是一个跨平台桌面 Runtime，让 AI Agent（如 Claude）通过 MCP 接口，用**声明式
App 定义**（而非生成一整套独立代码）来创建、修改和运行本地应用。用户只需要用自然语言
描述需求（"帮我做一个记账工具" → "增加月度统计" → "帮我备份数据"），Runtime 负责渲染
UI、管理数据库、持久化、备份与更新，用户不需要理解开发、部署或数据库。

详见项目构思 [IDEA.md](./IDEA.md) 与开发任务清单 [TODO.md](./TODO.md)。

## 核心能力

- **App Schema** — JSON 声明式描述数据模型、页面（列表/表单/详情/图表）、交互逻辑与
  定时任务，带版本化 Schema 校验器。
- **Runtime Core** — App 生命周期管理、本地 App Registry、每 App 独立 SQLite、安全的
  数据迁移、文件存储、备份/恢复、定时任务调度。
- **声明式 UI Runtime** — 根据 App Schema 动态渲染页面，统一的组件库与主题，无需
  AI 关心样式细节。
- **MCP 接口层** — 向 AI Agent 暴露一整套工具（安装/更新/卸载 App、数据 CRUD、文件
  存储、备份恢复、定时任务），详见 [docs/mcp-server.md](./docs/mcp-server.md)。

## 技术栈

Tauri 2 + React + TypeScript + Vite + SQLite（`rusqlite`）+ MCP（`rmcp`）。

## 开发

```bash
pnpm install
pnpm tauri dev
```

## 构建

```bash
pnpm tauri build
```

打包与发布流程见 [docs/packaging.md](./docs/packaging.md)。

## 开源协议

本项目基于 [MIT License](./LICENSE) 开源，欢迎 Issue 与 PR。
