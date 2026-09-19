//! Runtime Core: App lifecycle, registry, per-app SQLite storage, data
//! migration, file storage, backup/restore, and the automation scheduler.
//!
//! See `AGENTS.md` for the App Schema contract this module consumes.

pub mod appdb;
pub mod backup;
pub mod paths;
pub mod registry;
pub mod scheduler;
pub mod storage;

use std::path::PathBuf;
use std::sync::Mutex;

use rusqlite::Connection;

use crate::runtime::scheduler::Scheduler;

/// Shared Tauri-managed state for the runtime.
pub struct RuntimeState {
    pub base_dir: PathBuf,
    pub registry_conn: Mutex<Connection>,
    pub scheduler: Mutex<Option<Scheduler>>,
}

impl RuntimeState {
    pub fn init(base_dir: PathBuf) -> Result<Self, String> {
        std::fs::create_dir_all(&base_dir).map_err(|e| e.to_string())?;
        let conn =
            Connection::open(paths::registry_db_path(&base_dir)).map_err(|e| e.to_string())?;
        registry::init_schema(&conn).map_err(|e| e.to_string())?;
        Ok(Self {
            base_dir,
            registry_conn: Mutex::new(conn),
            scheduler: Mutex::new(None),
        })
    }
}
