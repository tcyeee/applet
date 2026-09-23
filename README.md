# Applet

**AI-Native Desktop Runtime** — AI creates the software, Applet makes it actually run.

Applet is a cross-platform desktop runtime that lets AI agents (like Claude) create,
modify, and run local applications through an MCP interface, using **declarative
App definitions** instead of generating a whole standalone codebase. Users just
describe what they need in natural language ("build me an expense tracker" →
"add monthly stats" → "back up my data"), and the runtime handles UI rendering,
database management, persistence, backups, and updates — no need to understand
development, deployment, or databases.

See the project concept in [IDEA.md](./IDEA.md) and the development task list in
[TODO.md](./TODO.md).

## Core Capabilities

- **App Schema** — A JSON declarative description of data models, pages
  (list/form/detail/chart), interaction logic, and scheduled tasks, with a
  versioned schema validator.
- **Runtime Core** — App lifecycle management, a local App Registry, a
  dedicated SQLite database per app, safe data migrations, file storage,
  backup/restore, and scheduled task execution.
- **Declarative UI Runtime** — Dynamically renders pages from the App Schema
  with a unified component library and theme, so the AI doesn't need to worry
  about styling details.
- **MCP Interface Layer** — Exposes a full toolset to AI agents (install/
  update/uninstall apps, data CRUD, file storage, backup/restore, scheduled
  tasks); see [docs/mcp-server.md](./docs/mcp-server.md) for details.

## Tech Stack

Tauri 2 + React + TypeScript + Vite + SQLite (`rusqlite`) + MCP (`rmcp`).

## Development

```bash
pnpm install
pnpm tauri dev
```

## Build

```bash
pnpm tauri build
```

See [docs/packaging.md](./docs/packaging.md) for the packaging and release process.

## License

This project is open source under the [MIT License](./LICENSE). Issues and PRs are welcome.
