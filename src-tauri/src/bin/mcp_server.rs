//! Stand-alone MCP server binary: exposes the Runtime Core over stdio for an
//! AI agent to spawn on demand (see AGENTS.md "MCP Interface Layer" for why
//! this is a separate on-demand process rather than a daemon bundled into
//! the Tauri GUI app). Run directly, or configure it as a local MCP server in
//! an MCP client — see `docs/mcp-server.md`.
//!
//! Locates the same app data directory the desktop app uses
//! (`registry.sqlite`, `apps/`, `shared/`, `backups/`) so both processes
//! operate on identical state:
//! 1. `APPLET_DATA_DIR` env var / `--data-dir <path>` CLI flag, if set.
//! 2. Otherwise `<OS data dir>/com.applet.runtime`, mirroring how Tauri
//!    resolves `app_data_dir()` from `tauri.conf.json`'s `identifier` — kept
//!    as a literal constant here since this binary intentionally does not
//!    depend on the `tauri` crate (it would pull in the whole webview stack
//!    for a headless stdio process).

use std::path::PathBuf;

use applet_lib::mcp::AppletMcpServer;
use rmcp::ServiceExt;

/// Must match `identifier` in `src-tauri/tauri.conf.json`.
const APP_IDENTIFIER: &str = "com.applet.runtime";

fn resolve_base_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("APPLET_DATA_DIR") {
        return PathBuf::from(dir);
    }
    let args: Vec<String> = std::env::args().collect();
    if let Some(idx) = args.iter().position(|a| a == "--data-dir") {
        if let Some(dir) = args.get(idx + 1) {
            return PathBuf::from(dir);
        }
    }
    dirs::data_dir()
        .expect("resolve OS data directory")
        .join(APP_IDENTIFIER)
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let base_dir = resolve_base_dir();
    let server = AppletMcpServer::new(base_dir).map_err(anyhow::Error::msg)?;
    let service = server.serve(rmcp::transport::stdio()).await?;
    service.waiting().await?;
    Ok(())
}
