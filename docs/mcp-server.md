# Applet MCP Server

An MCP (Model Context Protocol) server that lets an AI agent (Claude, etc.) drive the Applet
runtime directly: create/manage declarative Apps (`AppDefinition`), read/write their data and
files, and back up/restore them. This is TODO step 4 ("MCP 接口层").

It is a separate binary (`src-tauri/src/bin/mcp_server.rs`) from the Tauri desktop app, spoken to
over **stdio** and started on demand by the MCP client — the same model Claude Desktop/Claude Code
use for local MCP servers. See AGENTS.md's "MCP Interface Layer" section for why (in short:
Runtime Core's Rust functions don't depend on the Tauri runtime, so a second on-demand process can
call them directly without going through the GUI app; and it avoids two processes both polling the
same automation scheduler).

## Building

Two build steps, in order:

```sh
# 1. Bundle the App Schema validator (Zod, TypeScript) into a self-contained
#    script the server spawns via `node`. Re-run this whenever
#    src/app-schema/{validate,schema,types}.ts change.
pnpm build:mcp-validator

# 2. Build the server binary.
cd src-tauri && cargo build --release --bin mcp_server
```

This produces `src-tauri/target/release/mcp_server` and `dist-cli/validate-app.mjs` (at the repo
root). Both are required at runtime — the server refuses `install_app`/`update_app` calls with a
clear error if it can't find the validator bundle or `node` on `PATH`.

## Configuring an MCP client

Example for a client that reads a JSON config with `command`/`args`/`env` (adjust paths to where
you built the binary):

```json
{
  "mcpServers": {
    "applet": {
      "command": "/path/to/applet/src-tauri/target/release/mcp_server",
      "env": {
        "APPLET_VALIDATOR_PATH": "/path/to/applet/dist-cli/validate-app.mjs"
      }
    }
  }
}
```

### Data directory

The server operates on the same app data directory as the desktop app (`registry.sqlite`, `apps/`,
`shared/`, `backups/`), so installing/editing an App from the AI agent and from the desktop UI stay
in sync. Resolution order:

1. `APPLET_DATA_DIR` env var, or `--data-dir <path>` CLI argument.
2. Otherwise `<OS data dir>/com.applet.runtime` (matches Tauri's `app_data_dir()` for this app's
   `identifier` in `tauri.conf.json`) — e.g. `~/Library/Application Support/com.applet.runtime` on
   macOS.

Point both the desktop app and the MCP server at the same directory if you override it.

## Tool reference

Tool names mirror the equivalent Tauri commands in `src-tauri/src/commands.rs` where one exists,
so the two surfaces stay easy to cross-reference.

### App lifecycle

| Tool | Args | Notes |
|---|---|---|
| `install_app` | `definition` (AppDefinition JSON) | Validates first; returns `{success:false, errors}` without side effects if invalid. |
| `list_apps` | — | All installed apps. |
| `get_app` | `appId` | |
| `update_app` | `appId`, `definition`, `force?`, `confirm?` | Additive schema changes apply automatically. Anything that drops/reinterprets data needs `force: true` **and** `confirm: true`. |
| `start_app` / `stop_app` | `appId` | Toggles the status the desktop app's scheduler checks. |
| `uninstall_app` | `appId`, `purgeData?`, `confirm?` | `purgeData: true` deletes the app's database and files permanently; needs `confirm: true` too. |

### Data CRUD (entity-scoped, not arbitrary SQL)

| Tool | Args |
|---|---|
| `list_records` | `appId`, `entityId` |
| `create_record` | `appId`, `entityId`, `values` (object) |
| `update_record` | `appId`, `entityId`, `recordId`, `values` (object) |
| `delete_record` | `appId`, `entityId`, `recordId` |

Unknown field ids in `values` are rejected; entity/field ids are whitelisted against the app's
`dataModel` before touching SQL (see `src-tauri/src/runtime/appdb.rs`).

### File storage

| Tool | Args | Notes |
|---|---|---|
| `write_app_file` | `appId`, `path`, `contentsBase64` | Per-app private storage. |
| `read_app_file` | `appId`, `path` | Returns `{ path, contentsBase64 }`. |
| `delete_app_file` | `appId`, `path` | |
| `list_app_files` | `appId` | |
| `write_shared_file` / `read_shared_file` / `list_shared_files` | `path`, `contentsBase64` | Storage shared by every installed app (no per-app grant model yet — see AGENTS.md). |

Binary content is base64-encoded since MCP tool payloads are JSON. `path` is checked against path
traversal (`..`, absolute paths) by `src-tauri/src/runtime/storage.rs`.

### Backup / restore

| Tool | Args | Notes |
|---|---|---|
| `backup_app` | `appId` | Returns the archive path. |
| `backup_all_apps` | — | |
| `list_backups` | `appId` | |
| `restore_app` | `archivePath`, `overwrite?`, `confirm?` | `overwrite: true` replaces an already-installed app's current data; needs `confirm: true` too. |

### Scheduler (query only)

| Tool | Args | Notes |
|---|---|---|
| `list_automations` | `appId` | Reads the `automations` (cron triggers) already on the app's definition. There's no separate add/remove — edit them via `update_app`. This does **not** run or schedule anything; only the desktop app's own scheduler does that while it's running. |

## Security model

- **Structural, not ACL-based.** This mirrors the desktop app's own trust model: a local process a
  trusted AI client spawns, not a multi-tenant service. There's no auth on the stdio transport
  itself.
- **No arbitrary SQL / arbitrary file paths.** `appdb.rs` whitelists entity/field ids against the
  app's own schema before generating SQL; `storage.rs` rejects any path that would escape its root.
  The MCP layer inherits both for free by calling the same functions the desktop app's Tauri
  commands use.
- **Validation before trust.** `install_app`/`update_app` run the candidate `AppDefinition` through
  the same Zod-based `validateAppDefinition` the React frontend uses (via a Node subprocess, see
  `src-tauri/src/mcp/validator.rs`) before any state changes. Rust never re-implements the App
  Schema's business rules — see AGENTS.md.
- **Confirmation gate on destructive operations.** `uninstall_app(purgeData)`, `update_app(force)`,
  and `restore_app(overwrite)` each additionally require `confirm: true` to actually run; without
  it, the tool call fails with a message describing exactly what would have been lost and does not
  touch any state. This is a structural second argument the caller must pass, not just a warning in
  the tool description.
- **Known limitation:** there is no interactive elicitation (asking the human via the MCP protocol
  mid-call) — `confirm` is the whole mechanism for now. `rmcp`'s `elicitation` feature could add a
  richer flow later if needed.
