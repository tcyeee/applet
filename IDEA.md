### **项目构思：AI-Native Desktop Runtime**

随着 AI Agent 的能力提升，AI 已经可以快速生成完整的 App，但普通用户仍然很难解决 **部署、打包、安装、数据库、数据持久化、更新和备份** 等问题。

因此，希望开发一个**跨平台桌面 Runtime**，专门作为 AI Agent 创建应用后的运行环境。

核心理念：

**AI 负责创造软件，Runtime 负责让软件真正运行起来。**

Runtime 本身不预设具体业务功能，而是提供：

- 跨平台桌面运行环境
- 内置 SQLite 数据库
- 文件存储
- App 生命周期管理
- 数据迁移与持久化
- 备份与恢复
- 权限管理
- Scheduler
- MCP 接口

AI Agent 通过 MCP 操作 Runtime，可以创建和修改应用：

```text
用户需求
   ↓
AI Agent
   ↓ MCP
Desktop Runtime
   ├── App
   ├── Database
   ├── UI
   ├── Storage
   └── Automation
   ↓
用户直接使用
```

应用最好采用**声明式定义**，而不是让 Agent 每次生成一个完整的独立项目。Runtime 负责渲染 UI、管理数据和应用生命周期。

例如用户可以直接告诉 AI：

“帮我做一个个人记账工具。”

Agent 创建数据库、页面和功能，并将 App 安装到 Runtime。之后用户还可以继续要求：

“增加月度统计。”

“增加预算功能。”

“帮我备份数据。”

整个过程不需要用户理解开发、部署和数据库。

**最终目标：让 AI Agent 从“代码生成器”进一步变成真正的软件创造者，而 Runtime 成为 AI-generated applications 的基础运行环境。**

MVP 可先聚焦：

**Tauri + SQLite + MCP + App Registry + 声明式 UI Runtime + 数据持久化。**